//! Every generated domain line is at most 100 columns (design/03 §1), a wide
//! East Asian character counting two, as oxfmt counts it. Lines oxfmt does
//! not break either are left as they are: an `import` of one name, and a
//! template literal, whose text a break would change.
//! Comment lines are left as they are. Runtime and adapter copies are
//! hand-written and checked on their own.

use std::fs;
use std::path::Path;

use purecrate_check::accept;
use purecrate_emit_ts::{has_wire, WireSchema};
use purecrate_pack::assemble_with;
use purecrate_syntax::parse_source;

const WIDTH: usize = 100;

fn generated(name: &str, source: &str) -> Vec<(String, String)> {
    let Ok(krate) = parse_source(name, source) else {
        return Vec::new();
    };
    let Ok(typed) = accept(&krate) else {
        return Vec::new();
    };
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

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with('*')
}

/// What oxfmt keeps on one line however long.
fn unbreakable(line: &str) -> bool {
    let lone_import = line.starts_with("import ")
        && line.split_once('{').and_then(|(_, r)| r.split_once('}')).is_some_and(|(names, _)| !names.contains(','));
    lone_import || line.contains('`')
}

#[test]
fn generated_lines_fit_the_width() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut inputs: Vec<(String, String)> = Vec::new();
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
    let mut over = Vec::new();
    let mut files = 0;
    for (name, src) in &inputs {
        for (file, text) in generated(name, src) {
            files += 1;
            for (i, line) in text.lines().enumerate() {
                let cols = purecrate_emit_ts::columns(line);
                if !is_comment(line) && !unbreakable(line) && cols > WIDTH {
                    over.push(format!("{file}:{}:{cols}: {line}", i + 1));
                }
            }
        }
    }
    assert!(files > 200, "read {files} files");
    assert!(over.is_empty(), "{} generated lines over {WIDTH} characters:\n{}", over.len(), over.join("\n"));
}
