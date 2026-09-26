//! Drives the `purecrate-ts` binary: diagnostics, and what happens to `--out`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("purecrate-build-{tag}-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    fs::create_dir_all(&dir).expect("mkdir scratch");
    dir
}

fn build(src: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_purecrate-ts"))
        .args(["build"])
        .arg(src)
        .args(["--name", "fixture", "--out"])
        .arg(out)
        .output()
        .expect("run purecrate-ts")
}

fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .expect("read scratch")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("purecrate-"))
        .collect()
}

#[test]
fn rejected_crate_reports_locations_and_leaves_out_alone() {
    let dir = scratch("rejected");
    let out = dir.join("pkg");
    fs::create_dir_all(&out).expect("mkdir out");
    fs::write(out.join("keep.txt"), "previous build").expect("write sentinel");

    let src = fixture("rejected.rs");
    let result = build(&src, &out);
    let stderr = String::from_utf8_lossy(&result.stderr);

    assert_eq!(result.status.code(), Some(1), "{stderr}");
    let path = src.display();
    assert!(
        stderr.contains(&format!("{path}:10:8: `Step` and `step` would both be emitted as `step.ts`")),
        "{stderr}"
    );
    assert!(stderr.contains(&format!("  note: see {path}:6:12")), "{stderr}");
    assert!(
        stderr.contains(&format!("{path}:10:8: match on `Event` is missing `Event::Dec`")),
        "{stderr}"
    );
    assert!(stderr.contains("2 error(s); nothing written"), "{stderr}");

    assert_eq!(fs::read_to_string(out.join("keep.txt")).unwrap(), "previous build");
    assert!(!out.join("src").exists());
    assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn parse_errors_also_leave_out_alone() {
    let dir = scratch("parse");
    let src = dir.join("bad.rs");
    fs::write(&src, "pub struct S { pub n: i32 }\npub fn f(s: S) -> i32 {\n    let r = &s;\n    0\n}\n")
        .expect("write source");
    let out = dir.join("pkg");

    let result = build(&src, &out);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains(&format!("{}:3:13: unsupported expression `&s`", src.display())),
        "{stderr}"
    );
    assert!(!out.exists());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn successful_build_replaces_out_and_prunes_unreachable_items() {
    let dir = scratch("ok");
    let out = dir.join("pkg");
    fs::create_dir_all(&out).expect("mkdir out");
    fs::write(out.join("stale.txt"), "old").expect("write stale");

    let src = dir.join("lib.rs");
    let counter = fs::read_to_string(repo().join("examples/counter/src/lib.rs")).expect("read counter");
    fs::write(&src, format!("{counter}\nfn unused(s: State) -> State {{ s }}\n")).expect("write source");

    let result = build(&src, &out);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(!out.join("stale.txt").exists());
    assert!(out.join("src/step.ts").exists());
    assert!(!out.join("src/unused.ts").exists());
    assert!(leftovers(&dir).is_empty(), "{:?}", leftovers(&dir));
    fs::remove_dir_all(&dir).ok();
}
