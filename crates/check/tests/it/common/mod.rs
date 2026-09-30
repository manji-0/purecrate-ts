// Each test file uses its own share of these helpers.
#![allow(dead_code)]

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use purecrate_check::{check, Diagnostic};
use purecrate_syntax::parse_source;

pub fn diagnostics(source: &str) -> Vec<Diagnostic> {
    check(&parse_source("c", source).expect("parse"))
}

pub fn messages(source: &str) -> Vec<String> {
    diagnostics(source).into_iter().map(|d| d.message).collect()
}

/// Accepted by the subset checks, and a library rustc compiles: the CLI
/// runs both, so a clean case that rustc rejects would never be accepted.
pub fn assert_clean(source: &str) {
    let found = messages(source);
    assert!(found.is_empty(), "unexpected diagnostics: {found:#?}");
    assert_compiles(source);
}

pub fn assert_rejects(source: &str, needle: &str) {
    let found = messages(source);
    assert!(
        found.iter().any(|m| m.contains(needle)),
        "expected a diagnostic containing {needle:?}, got {found:#?}"
    );
}

fn assert_compiles(source: &str) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "purecrate-check-rustc-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let src = dir.join("lib.rs");
    std::fs::write(&src, source).expect("write");
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition", "2021", "--crate-type", "lib", "--emit=metadata", "--cap-lints", "allow"])
        .arg("--out-dir")
        .arg(&dir)
        .arg(&src)
        .output()
        .expect("run rustc");
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        output.status.success(),
        "rustc rejects a case the subset accepts:\n{source}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
