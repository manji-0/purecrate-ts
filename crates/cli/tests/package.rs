//! A generated package is usable from `node_modules` the way npm delivers it:
//! built to `dist` by its own `build` script, packed, and installed next to
//! the runtime (`purecrate`) and a schema adapter as peer dependencies. The
//! consumer runs it under plain node and type-checks it with `nodenext` and
//! `bundler` resolution, reading the packages' declarations
//! (`skipLibCheck: false`). Each TypeScript major builds and checks it.
//! Offline: every tarball is packed from this repository.

#[allow(dead_code, unused_macros)]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::{assemble_versioned, disk_path};
use purecrate_syntax::parse_source;

const SOURCE: &str = "
pub struct Yen(i64);
pub enum Event { Add(i32), Reset }
pub struct State { pub n: i32, pub total: Yen }
pub fn step(s: State, e: Event) -> State {
    match e {
        Event::Add(k) => State { n: s.n + k, total: s.total },
        Event::Reset => State { n: 0i32, total: s.total },
    }
}
";

const MAIN_JS: &str = r#"import { step, Event, Int } from "shop";
import { State as StateWire } from "shop/wire";
import { parseJson } from "purecrate";
const s = StateWire.parse(parseJson('{"n":1,"total":9007199254740993}'));
const t = step(s, Event.Add(Int.i32.of(2)));
console.log(`${t.n} ${t.total}`);
"#;

const MAIN_TS: &str = r#"import { step, Event, Int, type State, type Yen } from "shop";
import { State as StateWire } from "shop/wire";
import { parseJson } from "purecrate";
const s: State = StateWire.parse(parseJson('{"n":1,"total":9007199254740993}'));
const t: State = step(s, Event.Add(Int.i32.of(2)));
const total: Yen = t.total;
console.log(`${t.n} ${total}`);
"#;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(cmd: &mut Command, what: &str) -> String {
    let output = cmd.output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        output.status.success(),
        "{what} failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for entry in fs::read_dir(from).expect("read_dir") {
        let path = entry.expect("entry").path();
        let name = path.file_name().expect("name");
        if name == "node_modules" || name == "dist" || name == "package-lock.json" {
            continue;
        }
        if path.is_dir() {
            copy(&path, &to.join(name));
        } else {
            fs::copy(&path, to.join(name)).expect("copy");
        }
    }
}

fn link(dir: &Path, name: &str, target: &Path) {
    let modules = dir.join("node_modules").join(name);
    fs::create_dir_all(modules.parent().expect("parent")).expect("mkdir node_modules");
    std::os::unix::fs::symlink(target, &modules).unwrap_or_else(|e| panic!("link {name}: {e}"));
}

fn tsc(major: &str, dir: &Path, project: &str) {
    run(
        Command::new("npx")
            .args(["-y", "-p", &format!("typescript@{major}"), "tsc", "-p", project])
            .current_dir(dir),
        &format!("tsc {major} -p {project} in {}", dir.display()),
    );
}

/// `npm pack` without the `prepack` build: `dist` is already built.
fn pack(dir: &Path, into: &Path) -> PathBuf {
    let name = run(
        Command::new("npm")
            .args(["pack", "--ignore-scripts", "--silent", "--pack-destination"])
            .arg(into)
            .arg(dir),
        &format!("npm pack {}", dir.display()),
    );
    into.join(name.trim())
}

#[test]
fn packed_package_installs_runs_and_type_checks() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let krate = parse_source("shop", SOURCE).expect("parse");
    let typed = accept(&krate).expect("accept");
    let root = std::env::temp_dir().join(format!("purecrate-package-{}", std::process::id()));
    for major in support::TS_MAJORS {
        let dir = root.join(format!("ts{major}"));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clear");
        }
        let tarballs = dir.join("tarballs");
        fs::create_dir_all(&tarballs).expect("mkdir");

        let runtime = dir.join("purecrate");
        copy(&repo().join("packages/boundary"), &runtime);
        tsc(major, &runtime, "tsconfig.build.json");

        let zod = repo().join("packages/boundary-zod/node_modules/zod");
        let adapter = dir.join("purecrate-zod");
        copy(&repo().join("packages/boundary-zod"), &adapter);
        link(&adapter, "purecrate", &runtime);
        link(&adapter, "zod", &zod);
        tsc(major, &adapter, "tsconfig.build.json");

        let generated = dir.join("shop");
        for file in assemble_versioned(&typed, Some(WireSchema::Zod), "1.2.3").files {
            let path = generated.join(disk_path(&file.stem));
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, file.source).expect("write");
        }
        link(&generated, "purecrate", &runtime);
        link(&generated, "purecrate-zod", &adapter);
        link(&generated, "zod", &zod);
        tsc(major, &generated, "tsconfig.build.json");

        let packed: Vec<PathBuf> = [&runtime, &adapter, &zod, &generated]
            .into_iter()
            .map(|d| pack(d, &tarballs))
            .collect();
        assert!(packed[3].ends_with("shop-1.2.3.tgz"), "{}", packed[3].display());

        let consumer = dir.join("consumer");
        fs::create_dir_all(&consumer).expect("mkdir consumer");
        fs::write(consumer.join("package.json"), r#"{ "name": "consumer", "private": true, "type": "module" }"#)
            .expect("write consumer manifest");
        run(
            Command::new("npm")
                .args(["install", "--offline", "--ignore-scripts", "--no-audit", "--no-fund", "--silent"])
                .args(&packed)
                .current_dir(&consumer),
            "npm install",
        );
        fs::write(consumer.join("main.mjs"), MAIN_JS).expect("write main.mjs");
        let out = run(Command::new("node").arg("main.mjs").current_dir(&consumer), "node main.mjs");
        assert_eq!(out.trim(), "3 9007199254740993");

        fs::write(consumer.join("main.ts"), MAIN_TS).expect("write main.ts");
        for (module, resolution) in [("nodenext", "nodenext"), ("preserve", "bundler")] {
            fs::write(
                consumer.join("tsconfig.json"),
                format!(
                    r#"{{ "compilerOptions": {{ "strict": true, "target": "ES2022", "module": "{module}", "moduleResolution": "{resolution}", "noEmit": true, "skipLibCheck": false }}, "files": ["main.ts"] }}"#
                ),
            )
            .expect("write consumer tsconfig");
            tsc(major, &consumer, ".");
        }
    }
    fs::remove_dir_all(&root).ok();
}
