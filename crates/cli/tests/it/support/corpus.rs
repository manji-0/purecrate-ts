//! Every input the subset is read over: each example and each test fixture,
//! and the files generated from them.

use std::fs;

use purecrate_check::accept;
use purecrate_emit_ts::{has_wire, WireSchema};
use purecrate_pack::assemble_with;
use purecrate_syntax::parse_source;

/// Each example's `src/lib.rs` and each fixture, by name.
pub fn inputs() -> Vec<(String, String)> {
    let root = super::repo();
    let mut inputs = Vec::new();
    for dir in fs::read_dir(root.join("examples")).expect("examples") {
        let lib = dir.expect("entry").path().join("src/lib.rs");
        if let Ok(src) = fs::read_to_string(&lib) {
            inputs.push((
                lib.parent().unwrap().parent().unwrap().file_name().unwrap().to_string_lossy().into_owned(),
                src,
            ));
        }
    }
    for f in fs::read_dir(root.join("crates/cli/tests/fixtures")).expect("fixtures") {
        let path = f.expect("entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            inputs.push((name, fs::read_to_string(&path).expect("read")));
        }
    }
    inputs
}

/// The generated sources of `source`, without a schema and with each schema
/// library where the crate derives serde, as `<name>/<schema>/<stem>`; the
/// runtime and adapters left out (they are hand-written and checked on their
/// own), and nothing where the subset rejects `source`.
pub fn generated(name: &str, source: &str) -> Vec<(String, String)> {
    let Ok(krate) = parse_source(name, source) else { return Vec::new() };
    let Ok(typed) = accept(&krate) else { return Vec::new() };
    let mut schemas = vec![None];
    if has_wire(&typed) {
        schemas.extend([Some(WireSchema::Zod), Some(WireSchema::Valibot), Some(WireSchema::Arktype)]);
    }
    let mut out = Vec::new();
    for schema in schemas {
        for f in assemble_with(&typed, schema).files {
            let hand_written =
                f.stem == "purecrate-runtime" || f.stem.starts_with("purecrate-") && f.stem != "purecrate-wire";
            if !hand_written && !f.stem.contains('.') {
                out.push((format!("{name}/{schema:?}/{}", f.stem), f.source));
            }
        }
    }
    out
}
