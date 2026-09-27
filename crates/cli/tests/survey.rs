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
    for accepted in ["twice", "area", "unit", "clamp01", "Shape"] {
        assert!(item(&json, accepted).contains("\"status\":\"accepted\""), "{}", item(&json, accepted));
    }
    let rejected = [
        ("abs_of", "\"code\":\"expr/method-call\",\"detail\":\"abs\""),
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
        text.contains("public functions: 9 — accepted 4 (44%), rejected 3, blocked by a dependency 2"),
        "{text}"
    );
    assert!(text.contains("public types: 3 — accepted 1 (33%), rejected 2, blocked by a dependency 0"), "{text}");
}
