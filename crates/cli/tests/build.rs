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

fn check(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_purecrate-ts"))
        .arg("check")
        .args(args)
        .output()
        .expect("run purecrate-ts")
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for entry in fs::read_dir(from).expect("read_dir") {
        let path = entry.expect("entry").path();
        let dest = to.join(path.file_name().expect("name"));
        if path.is_dir() {
            copy_tree(&path, &dest);
        } else {
            fs::copy(&path, &dest).expect("copy");
        }
    }
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
fn check_without_out_only_runs_the_subset_checks() {
    let ok = check(&[repo().join("examples/counter").as_os_str()]);
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));

    let bad = check(&[fixture("rejected.rs").as_os_str()]);
    let stderr = String::from_utf8_lossy(&bad.stderr);
    assert_eq!(bad.status.code(), Some(1), "{stderr}");
    assert!(stderr.ends_with("2 error(s)\n"), "{stderr}");
}

#[test]
fn check_with_out_passes_on_the_committed_golden() {
    let crate_dir = repo().join("examples/counter");
    let golden = repo().join("examples/counter-ts");
    let result = check(&[crate_dir.as_os_str(), "--out".as_ref(), golden.as_os_str()]);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

#[test]
fn check_with_out_lists_every_drifted_file() {
    let dir = scratch("drift");
    let out = dir.join("pkg");
    copy_tree(&repo().join("examples/counter-ts"), &out);
    fs::write(out.join("src/step.ts"), "// edited by hand\n").expect("edit");
    fs::remove_file(out.join("src/event.ts")).expect("remove");
    fs::write(out.join("src/notes.ts"), "").expect("extra");

    let crate_dir = repo().join("examples/counter");
    let result = check(&[crate_dir.as_os_str(), "--out".as_ref(), out.as_os_str()]);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert_eq!(result.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("3 file(s) out of date"), "{stderr}");
    let listed: Vec<&str> = stderr.lines().filter(|l| l.starts_with("  ")).collect();
    assert_eq!(
        listed,
        ["  missing  src/event.ts", "  extra    src/notes.ts", "  differs  src/step.ts"],
        "{stderr}"
    );
    assert!(stderr.contains("run: purecrate-ts build"), "{stderr}");
    assert_eq!(fs::read_to_string(out.join("src/step.ts")).unwrap(), "// edited by hand\n");

    let missing_dir = check(&[crate_dir.as_os_str(), "--out".as_ref(), dir.join("nope").as_os_str()]);
    assert!(String::from_utf8_lossy(&missing_dir.stderr).contains("out of date"));
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
