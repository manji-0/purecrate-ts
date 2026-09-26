//! v0 acceptance: the generated counter package returns the same `State` as the
//! Rust source for every `Event`. Needs `node` (>= 22.18, native type stripping)
//! on PATH; set `PURECRATE_SKIP_NODE=1` to skip explicitly.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::parse_source;

#[allow(dead_code)]
mod counter {
    include!("../../../examples/counter/src/lib.rs");
}

const SOURCE: &str = include_str!("../../../examples/counter/src/lib.rs");

const EVENTS: [&str; 3] = ["Inc", "Dec", "Reset"];

fn rust_step(n: i32, event: &str) -> i32 {
    let event = match event {
        "Inc" => counter::Event::Inc,
        "Dec" => counter::Event::Dec,
        "Reset" => counter::Event::Reset,
        other => panic!("unknown event {other}"),
    };
    counter::step(counter::State { n }, event).n
}

const DRIVER: &str = r#"import { Event, State, step } from "./src/index.ts";
const lines = process.argv[2].split("\n").filter((l) => l.length > 0);
const out = lines.map((line) => {
  const [n, kind] = line.split(" ");
  return step(State.of(Number(n)), Event[kind]()).n;
});
console.log(out.join("\n"));
"#;

fn write_package(dir: &Path) {
    let krate = parse_source("counter", SOURCE).expect("parse counter");
    for file in assemble(&krate).files {
        let path = dir.join(disk_path(&file.stem));
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, file.source).expect("write");
    }
    fs::write(dir.join("driver.ts"), DRIVER).expect("write driver");
}

fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("purecrate-counter-eq-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    dir
}

#[test]
fn generated_counter_matches_rust_step() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        eprintln!("PURECRATE_SKIP_NODE set: skipping TS equivalence");
        return;
    }

    let states: Vec<i32> = (-3..=3).chain([-1000, 1000]).collect();
    let cases: Vec<(i32, &str)> = states
        .iter()
        .flat_map(|&n| EVENTS.iter().map(move |&e| (n, e)))
        .collect();
    let expected: Vec<i32> = cases.iter().map(|&(n, e)| rust_step(n, e)).collect();

    let dir = scratch_dir();
    write_package(&dir);
    let input: String = cases.iter().map(|(n, e)| format!("{n} {e}\n")).collect();
    let output = Command::new("node")
        .arg("driver.ts")
        .arg(&input)
        .current_dir(&dir)
        .output()
        .expect("run node (set PURECRATE_SKIP_NODE=1 to skip)");
    assert!(
        output.status.success(),
        "node failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Vec<i32> = String::from_utf8(output.stdout)
        .expect("utf8")
        .lines()
        .map(|l| l.parse().expect("number"))
        .collect();
    fs::remove_dir_all(&dir).ok();

    assert_eq!(actual.len(), cases.len());
    for ((case, want), got) in cases.iter().zip(&expected).zip(&actual) {
        assert_eq!(want, got, "step({case:?})");
    }
}
