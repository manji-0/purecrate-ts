//! The serde attributes the wire module models (design/04 §3.6), against
//! the real derive: `fixtures/wire_attrs.rs` is compiled here with
//! `serde_derive`, and generated with each schema library. For every JSON
//! text of a corpus, `fromJson.T` then `toJson.T` must give what
//! `serde_json::from_str::<T>` then `to_string` gives, or both refuse.
//!
//! The corpus is serde_json's own text for sample values, texts written
//! for the edges (Rust names where the JSON names differ, keys twice, the
//! sequence form), and mechanical mutations of each: every key left out,
//! the first key twice, an object as the array of its values, and every
//! JSON name of the type swapped in for a string.

use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::assemble_with;
use purecrate_syntax::parse_source;
use serde_json::Value;

use crate::support;

#[allow(dead_code)]
mod attrs {
    include!("../fixtures/wire_attrs.rs");
}

const SOURCE: &str = include_str!("../fixtures/wire_attrs.rs");

/// `ok <json>` or `err`, as the TS driver prints.
fn rust<T: serde::de::DeserializeOwned + serde::Serialize>(text: &str) -> String {
    match serde_json::from_str::<T>(text) {
        Ok(v) => format!("ok {}", serde_json::to_string(&v).expect("serialize")),
        Err(_) => "err".into(),
    }
}

fn oracle(ty: &str) -> fn(&str) -> String {
    match ty {
        "Account" => rust::<attrs::Account>,
        "Limits" => rust::<attrs::Limits>,
        "Status" => rust::<attrs::Status>,
        "Event" => rust::<attrs::Event>,
        "Envelope" => rust::<attrs::Envelope>,
        other => panic!("no oracle for {other}"),
    }
}

/// Every string the corpus swaps in for another: the JSON names and the
/// Rust names they replace.
const NAMES: &[&str] = &[
    "userId",
    "user_id",
    "displayName",
    "e-mail",
    "email",
    "createdAtMs",
    "MAX-ITEMS",
    "max_items",
    "PER-PAGE",
    "ok?",
    "ok",
    "active",
    "Active",
    "on_hold",
    "OnHold",
    "gone!",
    "Deleted",
    "opened",
    "Opened",
    "renamed",
    "OldName",
    "old_name",
    "NewName",
    "moved",
    "closed",
    "pinged",
    "Pinged",
    "at-ms",
    "at_ms",
];

fn seeds() -> Vec<(&'static str, String)> {
    let account = r#"{"userId":7,"displayName":"Ann","e-mail":"a@b","quote\"and\\slash\tand tab":true,"createdAtMs":18446744073709551615}"#;
    let limits = r#"{"MAX-ITEMS":10,"PER-PAGE":255,"ok?":false}"#;
    let mut out: Vec<(&str, String)> = vec![
        ("Account", account.into()),
        ("Account", r#"{"userId":7,"displayName":"Ann","quote\"and\\slash\tand tab":false,"createdAtMs":0}"#.into()),
        (
            "Account",
            r#"{"user_id":7,"displayName":"Ann","e-mail":null,"quote\"and\\slash\tand tab":true,"createdAtMs":1}"#
                .into(),
        ),
        ("Account", r#"[7,"Ann",null,true,5]"#.into()),
        ("Limits", limits.into()),
        ("Limits", r#"{"MAX-ITEMS":10,"PER-PAGE":256,"ok?":false}"#.into()),
        ("Limits", r#"{"MAX-ITEMS":1,"PER-PAGE":2,"ok":true}"#.into()),
        ("Status", r#""active""#.into()),
        ("Status", r#""on_hold""#.into()),
        ("Status", r#""gone!""#.into()),
        ("Status", r#"{"on_hold":null}"#.into()),
        ("Status", r#""Deleted""#.into()),
        ("Event", r#"{"opened":{"at-ms":5}}"#.into()),
        ("Event", r#"{"renamed":{"OldName":"a","NewName":"b"}}"#.into()),
        ("Event", r#"{"renamed":["a","b"]}"#.into()),
        ("Event", r#"{"moved":[1,-2]}"#.into()),
        ("Event", r#"{"closed":"gone!"}"#.into()),
        ("Event", r#""pinged""#.into()),
        ("Event", r#"{"pinged":null}"#.into()),
        ("Event", r#"{"Opened":{"at_ms":5}}"#.into()),
    ];
    out.push((
        "Envelope",
        format!(r#"{{"account":{account},"limits":{limits},"events":[{{"opened":{{"at-ms":1}}}},"pinged",{{"closed":"active"}}]}}"#),
    ));
    out
}

/// The text, and its mutations.
fn mutations(text: &str) -> Vec<String> {
    let mut out = vec![text.to_string()];
    let Ok(value) = serde_json::from_str::<Value>(text) else { return out };
    match &value {
        Value::Object(map) => {
            for key in map.keys() {
                let mut less = map.clone();
                less.remove(key);
                out.push(Value::Object(less).to_string());
                for name in NAMES {
                    let mut swapped = serde_json::Map::new();
                    for (k, v) in map {
                        swapped.insert(if k == key { name.to_string() } else { k.clone() }, v.clone());
                    }
                    out.push(Value::Object(swapped).to_string());
                }
            }
            out.push(Value::Array(map.values().cloned().collect()).to_string());
            if let Some((k, v)) = map.iter().next() {
                let twice = format!("{{{}:{v},{}", Value::String(k.clone()), &text[1..]);
                out.push(twice);
            }
        }
        Value::String(_) => out.extend(NAMES.iter().map(|n| Value::String(n.to_string()).to_string())),
        _ => {}
    }
    out
}

fn corpus() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (ty, text) in seeds() {
        for m in mutations(&text) {
            if !out.contains(&(ty, m.clone())) {
                out.push((ty, m));
            }
        }
    }
    out
}

fn js(s: &str) -> String {
    serde_json::to_string(s).expect("a JS string literal")
}

fn run(schema: WireSchema, cases: &[(&str, String)]) -> Option<Vec<String>> {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return None;
    }
    let lib = schema.runtime_dep();
    let dir = support::scratch(&format!("wire-attrs-{lib}"));
    let krate = parse_source("wire_attrs", SOURCE).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("rejected: {d:#?}"));
    support::write_package(&dir, &assemble_with(&typed, Some(schema)));
    let package = format!("boundary-{lib}");
    support::link(&dir, lib, &format!("{package}/node_modules/{lib}"));
    if schema == WireSchema::Arktype {
        support::link(&dir, "@ark", &format!("{package}/node_modules/@ark"));
    }
    support::typecheck(&dir);
    let rows: Vec<String> = cases.iter().map(|(ty, text)| format!("  [{}, {}],", js(ty), js(text))).collect();
    let script = format!(
        "import {{ fromJson, toJson }} from \"./src/purecrate-wire.ts\";\nconst cases = [\n{}\n];\nconst out = [];\n\
         for (const [t, text] of cases) {{\n  try {{\n    out.push(\"ok \" + toJson[t](fromJson[t](text)));\n  }} catch {{\n    out.push(\"err\");\n  }}\n}}\n\
         console.log(out.join(\"\\n\"));\n",
        rows.join("\n")
    );
    fs::write(dir.join("driver.ts"), script).expect("write driver");
    let output = Command::new("node")
        .arg(format!("--conditions={}", support::SOURCE_CONDITION))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    assert!(output.status.success(), "node in {}:\n{}", dir.display(), String::from_utf8_lossy(&output.stderr));
    let _ = fs::remove_dir_all(&dir);
    Some(String::from_utf8(output.stdout).expect("utf8").lines().map(str::to_string).collect())
}

#[test]
fn renamed_fields_and_variants_read_and_write_as_serde_does() {
    let cases = corpus();
    let expected: Vec<String> = cases.iter().map(|(ty, text)| oracle(ty)(text)).collect();
    assert!(expected.iter().filter(|e| e.starts_with("ok")).count() > 20, "the corpus reads too little");
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let Some(got) = run(schema, &cases) else { return };
        assert_eq!(got.len(), cases.len());
        let wrong: Vec<String> = cases
            .iter()
            .zip(expected.iter().zip(&got))
            .filter(|(_, (e, g))| e != g)
            .map(|((ty, text), (e, g))| format!("{ty} {text}\n  serde: {e}\n  {}:  {g}", schema.runtime_dep()))
            .collect();
        assert!(wrong.is_empty(), "{} of {} differ:\n{}", wrong.len(), cases.len(), wrong.join("\n"));
    }
}
