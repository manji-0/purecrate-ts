//! `--schema zod` writes a wire schema whose parse result is the domain value.

use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::{assemble_with, disk_path};
use purecrate_syntax::parse_source;

const SOURCE: &str = r#"
pub struct State { pub n: i32 }
pub enum Event { Inc, Dec, Add(i32) }
pub fn step(state: State, event: Event) -> State {
    match event {
        Event::Inc => State { n: state.n + 1 },
        Event::Dec => State { n: state.n - 1 },
        Event::Add(k) => State { n: state.n + k },
    }
}
"#;

#[test]
fn zod_wire_parses_serde_json_into_the_domain_value() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let krate = parse_source("wire", SOURCE).expect("parse");
    let typed = accept(&krate).expect("accept");
    let dir = std::env::temp_dir().join(format!("purecrate-wire-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).ok();
    }
    for file in assemble_with(&typed, Some(WireSchema::Zod)).files {
        let path = dir.join(disk_path(&file.stem));
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, file.source).expect("write");
    }
    link(&dir, "purecrate", "boundary");
    link(&dir, "purecrate-zod", "boundary-zod");
    link(&dir, "zod", "boundary-zod/node_modules/zod");
    fs::write(
        dir.join("driver.ts"),
        r#"import { Event, State } from "./src/purecrate-wire.ts";
const state = State.parse({ n: 1 });
const inc = Event.parse("Inc");
const add = Event.parse({ Add: 4 });
if (state.n !== 1) throw new Error("state");
if (inc.kind !== "Inc") throw new Error("inc");
if (add.kind !== "Add" || add.content[0] !== 4) throw new Error("add " + JSON.stringify(add));
console.log("ok");
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}

fn link(dir: &std::path::Path, name: &str, rel: &str) {
    let target = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages").join(rel);
    let modules = dir.join("node_modules").join(name);
    fs::create_dir_all(modules.parent().expect("node_modules")).expect("mkdir");
    let _ = fs::remove_file(&modules);
    std::os::unix::fs::symlink(&target, &modules).unwrap_or_else(|e| panic!("link {name}: {e} ({target:?})"));
}
