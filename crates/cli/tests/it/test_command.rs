//! `purecrate-ts test` (design/01 §8.4): a user's crate called in Rust and
//! in the generated TypeScript on drawn arguments. It passes on an example,
//! reaches private fields and nested modules, builds a closed type through
//! its constructor, and fails on a stated non-equivalence: `usize` past
//! 2^53 - 1, where Rust goes on and the TS `number` throws (design/01 §3).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use crate::support;

fn test(krate: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_purecrate-ts"))
        .arg("test")
        .arg(krate)
        .args(flags)
        .output()
        .expect("run purecrate-ts")
}

fn skip() -> bool {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        eprintln!("PURECRATE_SKIP_NODE set: skipping purecrate-ts test");
        return true;
    }
    false
}

#[test]
fn an_example_agrees() {
    if skip() {
        return;
    }
    let out = test(&support::repo().join("examples/ulid"), &["--cases", "30"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    // `Ulid` is closed: every method on it needs one made by `parse`, `new`,
    // or `from_bytes` first.
    for name in ["Ulid.parse", "Ulid.encode", "Ulid.toBytes", "next", "compare"] {
        assert!(stdout.contains(&format!("ok        {name} (30 cases)")), "{name}:\n{stdout}");
    }
}

#[test]
fn a_disagreement_fails_and_names_the_call() {
    if skip() {
        return;
    }
    let dir = support::scratch("test-command");
    let src = dir.join("src");
    fs::create_dir_all(src.join("shapes")).expect("mkdir");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"shapes-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[profile.release]\noverflow-checks = true\n",
    )
    .expect("write");
    fs::write(
        src.join("lib.rs"),
        "mod shapes;\n\npub use shapes::{area, Shape};\n\npub fn double(n: usize) -> usize {\n    n * 2\n}\n",
    )
    .expect("write");
    // A closed type with a private field, in a private module two levels
    // down, inside an enum the function under test takes.
    fs::write(
        src.join("shapes.rs"),
        "mod inner;\n\npub use inner::Side;\n\npub enum Shape {\n    Square(Side),\n    Rect { w: i64, h: i64 },\n    Dot,\n}\n\n\
         pub fn area(s: Shape) -> i64 {\n    match s {\n        Shape::Square(side) => side.len() * side.len(),\n        Shape::Rect { w, h } => w * h,\n        Shape::Dot => 0,\n    }\n}\n",
    )
    .expect("write");
    fs::write(
        src.join("shapes/inner.rs"),
        "pub struct Side {\n    len: i64,\n}\n\nimpl Side {\n    pub fn new(len: i64) -> Option<Side> {\n        if len >= 0 {\n            Some(Side { len })\n        } else {\n            None\n        }\n    }\n\n\
         pub fn len(&self) -> i64 {\n        self.len\n    }\n}\n",
    )
    .expect("write");
    let out = test(&dir, &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("DIFFERS   double"), "{stderr}");
    assert!(stderr.contains("ts:   panic(attempt to multiply with overflow)"), "{stderr}");
    for name in ["area", "Side.new", "Side.len"] {
        assert!(stderr.contains(&format!("ok        {name} (100 cases)")), "{name}:\n{stderr}");
    }
    // The same seed draws the same calls.
    let again = test(&dir, &[]);
    assert_eq!(String::from_utf8_lossy(&again.stderr), stderr);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_crate_that_uses_uuid_is_refused() {
    let out = test(&support::repo().join("examples/raft"), &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("uses `uuid`"), "{stderr}");
}
