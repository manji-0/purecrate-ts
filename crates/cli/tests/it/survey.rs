//! `survey` over a small multi-file crate: modules are followed and each
//! public item gets its own verdict.

use std::path::PathBuf;
use std::process::Command;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/survey")
}

fn survey(extra: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_purecrate-ts"))
        .arg("survey")
        .arg(fixture())
        .args(extra)
        .output()
        .expect("run purecrate-ts");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("utf-8")
}

/// The JSON object for the item named `name`.
fn item<'a>(json: &'a str, name: &str) -> &'a str {
    let key = format!("{{\"name\":\"{name}\",");
    let start = json.find(&key).unwrap_or_else(|| panic!("no item {name} in {json}"));
    let rest = &json[start..];
    let mut depth = 0;
    for (i, c) in rest.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &rest[..=i];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated item {name}")
}

#[test]
fn every_module_file_is_read_once_and_tests_are_skipped() {
    let json = survey(&["--json"]);
    for file in ["src/lib.rs", "src/shapes.rs", "src/util/mod.rs", "src/util/deeper.rs"] {
        assert_eq!(json.matches(&format!("{file}\"")).count(), 1, "{file} in {json}");
    }
    assert!(json.contains("\"missing_modules\":[]"), "{json}");
    assert!(!json.contains("\"name\":\"t\""), "{json}");
}

#[test]
fn each_public_item_gets_a_verdict() {
    let json = survey(&["--json"]);
    for accepted in ["twice", "area", "unit", "clamp01", "Shape", "sides_of", "diagonal_of", "Percent", "PercentError"] {
        assert!(item(&json, accepted).contains("\"status\":\"accepted\""), "{}", item(&json, accepted));
    }
    let rejected = [
        ("sign_of", "\"code\":\"expr/method-call\",\"detail\":\"signum\""),
        ("describe", "\"code\":\"item/ref-receiver\""),
        ("area_of", "\"code\":\"type/qualified-path\",\"detail\":\"shapes::Shape\""),
        ("Id", "\"code\":\"item/tuple-struct\""),
    ];
    for (name, cause) in rejected {
        let it = item(&json, name);
        assert!(it.contains("\"status\":\"rejected\"") && it.contains(cause), "{it}");
    }
    let blocked = [
        ("lookup", "\"blocked_by\":\"Table\"", "\"detail\":\"HashMap\""),
        ("next", "\"blocked_by\":\"Id\"", "\"code\":\"item/tuple-struct\""),
    ];
    for (name, by, cause) in blocked {
        let it = item(&json, name);
        assert!(it.contains("\"status\":\"blocked\"") && it.contains(by) && it.contains(cause), "{it}");
    }
    assert!(item(&json, "describe").contains("\"owner\":\"Shape\""));
    assert!(json.contains("\"not_judged\":{\"trait impl\":1}"), "{json}");
}

#[test]
fn human_summary_counts_functions_and_types() {
    let text = survey(&[]);
    assert!(text.starts_with("crate survey-demo (4 file(s))"), "{text}");
    assert!(
        text.contains("public functions: 14 — accepted 8 (57%), rejected 4, blocked by a dependency 2"),
        "{text}"
    );
    assert!(text.contains("public types: 5 — accepted 3 (60%), rejected 2, blocked by a dependency 0"), "{text}");
}

/// Without `--all-causes` a function reports the first cause the parser
/// meets; with it, every cause, the type check's included.
#[test]
fn all_causes_lists_every_cause_of_a_function() {
    let first = survey(&["--json"]);
    let one = item(&first, "many");
    assert!(one.contains("\"code\":\"expr/macro\""), "{one}");
    for later in ["expr/loop", "expr/method-call", "expr/cast"] {
        assert!(!one.contains(&format!("\"code\":\"{later}\"")), "{one}");
    }
    let all = survey(&["--json", "--all-causes"]);
    let every = item(&all, "many");
    for code in ["expr/macro", "expr/loop", "expr/method-call", "expr/cast"] {
        assert!(every.contains(&format!("\"code\":\"{code}\"")), "{code} missing in {every}");
    }
    // Items with one cause, or none, read the same either way.
    for name in ["sign_of", "twice", "lookup"] {
        assert_eq!(item(&first, name), item(&all, name), "{name}");
    }
}

/// std's `Ordering` is added as the parser adds it, so what uses it is
/// judged, and it is not counted among the crate's own types.
#[test]
fn std_ordering_is_there_for_what_uses_it() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ordering.rs");
    let out = Command::new(env!("CARGO_BIN_EXE_purecrate-ts")).arg("survey").arg(&path).output().expect("run purecrate-ts");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8(out.stdout).expect("utf-8");
    assert!(text.contains("public functions: 33 — accepted 33 (100%)"), "{text}");
    assert!(text.contains("public types: 2 — accepted 2 (100%)"), "{text}");
}
