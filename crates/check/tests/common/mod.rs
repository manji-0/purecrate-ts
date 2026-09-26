use purecrate_check::{check, Diagnostic};
use purecrate_syntax::parse_source;

pub fn diagnostics(source: &str) -> Vec<Diagnostic> {
    check(&parse_source("c", source).expect("parse"))
}

pub fn messages(source: &str) -> Vec<String> {
    diagnostics(source).into_iter().map(|d| d.message).collect()
}

pub fn assert_clean(source: &str) {
    let found = messages(source);
    assert!(found.is_empty(), "unexpected diagnostics: {found:#?}");
}

pub fn assert_rejects(source: &str, needle: &str) {
    let found = messages(source);
    assert!(
        found.iter().any(|m| m.contains(needle)),
        "expected a diagnostic containing {needle:?}, got {found:#?}"
    );
}
