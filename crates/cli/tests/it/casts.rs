//! Every `as` the generator prints is one of the kinds design/03 §1.1 lists,
//! each sound for a reason outside TS: a literal rustc has range-checked, a
//! `.length`, a lossless widening, a `for` counter below its bound, a folded
//! discriminant, a float, the crate's own constructor, or a union given back
//! its declared type. A brand (a newtype or closed struct of the crate) is
//! cast to only in its constructor. Read over the output of every example
//! and every test fixture the subset accepts, without a schema and with each
//! schema library where the crate derives serde.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use purecrate_check::accept;
use purecrate_emit_ts::{has_wire, WireSchema};
use purecrate_pack::assemble_with;
use purecrate_syntax::parse_source;

const NUMERIC: &[&str] = &["I8", "I16", "I32", "I64", "U8", "U16", "U32", "U64", "Usize", "F32", "F64"];

/// The generated sources of `source`, runtime and adapters left out (they
/// are hand-written and checked on their own).
fn generated(name: &str, source: &str) -> Vec<(String, String)> {
    let Ok(krate) = parse_source(name, source) else { return Vec::new() };
    let Ok(typed) = accept(&krate) else { return Vec::new() };
    let mut schemas = vec![None];
    if has_wire(&typed) {
        schemas.extend([Some(WireSchema::Zod), Some(WireSchema::Valibot), Some(WireSchema::Arktype)]);
    }
    let mut out = Vec::new();
    for schema in schemas {
        for f in assemble_with(&typed, schema).files {
            let hand_written = f.stem == "purecrate-runtime" || f.stem.starts_with("purecrate-") && f.stem != "purecrate-wire";
            if !hand_written && !f.stem.contains('.') {
                out.push((format!("{name}/{schema:?}/{}", f.stem), f.source));
            }
        }
    }
    out
}

/// The text before `at` back to the `(` that opens the group ending there,
/// or the identifier, path, or literal ending there.
fn operand(line: &str, at: usize) -> &str {
    let bytes = line.as_bytes();
    let end = at;
    let mut i = end;
    let mut depth = 0i32;
    while i > 0 {
        let c = bytes[i - 1];
        match c {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' if depth > 0 => depth -= 1,
            b'(' | b'[' | b'{' => break,
            b' ' | b',' | b'=' | b':' | b'?' | b'!' if depth == 0 => break,
            _ => {}
        }
        i -= 1;
        if depth == 0 && matches!(c, b'(' | b'[') {
            // A call or index: keep the callee before it.
            continue;
        }
    }
    &line[i..end]
}

fn is_number(s: &str) -> bool {
    let s = s.trim_start_matches('-').trim_end_matches('n');
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-' | '_'))
}

/// Why `<operand> as <target>` on `line` is sound, or `None`.
fn kind(line: &str, value: &str, target: &str, brands: &BTreeSet<String>) -> Option<&'static str> {
    let target_name = target.split([' ', ')', ';', ',', '[', '<']).next().unwrap_or("");
    let value = value.trim_start_matches('(');
    if target.starts_with("const") {
        return Some("readonly literal");
    }
    if line.trim_start().starts_with("import ") || (value.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && target_name.contains('$')) {
        return Some("import alias");
    }
    if target.starts_with("never") && value.starts_with("ctx.error(") {
        return Some("arktype's error value");
    }
    if NUMERIC.contains(&target_name) && is_number(value) {
        return Some("literal rustc range-checked");
    }
    if target_name == "Char" && value.starts_with('"') {
        return Some("char literal");
    }
    if target.starts_with("Iterable<Char>") {
        return Some("a string's code points are chars");
    }
    if target_name == "Usize" && value.trim_end_matches(')').ends_with(".length") {
        return Some("length");
    }
    // `x as number as U32`: both halves.
    if target.starts_with("number as ") || (value == "number" && NUMERIC.contains(&target_name)) || (NUMERIC.contains(&target_name) && value.starts_with("globalThis.BigInt(")) {
        return Some("lossless widening");
    }
    // The increment stays on the `for` line, or on the header's last line
    // once the end expression wraps (`i < $e; i = (i + 1) as Usize) {`).
    let for_header = line.contains("for (let ") || (line.contains("; ") && line.trim_end().ends_with(") {"));
    if NUMERIC.contains(&target_name) && (value.ends_with(" + 1)") || value.ends_with(" + 1n)")) && for_header {
        return Some("for counter below its bound");
    }
    if target.starts_with("Record<string, ") || (NUMERIC.contains(&target_name) && value.ends_with(".kind]")) {
        return Some("folded discriminant");
    }
    if target_name == "F64" || (target_name == "F32" && value.starts_with("globalThis.Math.fround(")) {
        return Some("float");
    }
    if (value == "value" || value == "fields") && (line.contains("$of = (") || line.trim_start().starts_with("of: (value")) {
        return Some("the crate's constructor");
    }
    if target.split(';').next().unwrap_or("").contains(" | null") && !is_place_text(value) {
        return Some("an `Option` given back its declared type");
    }
    if !brands.contains(target_name) && target_name.starts_with(|c: char| c.is_ascii_uppercase()) && !NUMERIC.contains(&target_name) && target_name != "Char" && target_name != "Uuid" && value.trim_start().starts_with('{') {
        return Some("union given back its declared type");
    }
    None
}

fn is_place_text(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '.'))
}

#[test]
fn every_cast_is_of_a_sound_kind() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut inputs: Vec<(String, String)> = Vec::new();
    for dir in fs::read_dir(root.join("examples")).expect("examples") {
        let lib = dir.expect("entry").path().join("src/lib.rs");
        if let Ok(src) = fs::read_to_string(&lib) {
            inputs.push((lib.parent().unwrap().parent().unwrap().file_name().unwrap().to_string_lossy().into_owned(), src));
        }
    }
    for f in fs::read_dir(root.join("crates/cli/tests/fixtures")).expect("fixtures") {
        let path = f.expect("entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            inputs.push((name, fs::read_to_string(&path).expect("read")));
        }
    }
    let mut unexplained = Vec::new();
    let mut files = 0;
    for (name, src) in &inputs {
        let package = generated(name, src);
        // The crate's brands, in any of its files:
        // `export type X = .. & { readonly "<crate>.X": true }`.
        let brands: BTreeSet<String> = package
            .iter()
            .flat_map(|(_, text)| text.lines())
            .filter(|l| l.starts_with("export type ") && l.contains(": true }"))
            .filter_map(|l| l["export type ".len()..].split(' ').next().map(str::to_string))
            .collect();
        for (file, text) in &package {
            files += 1;
            for line in text.lines() {
                let t = line.trim_start();
                if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
                    continue;
                }
                let mut from = 0;
                while let Some(at) = line[from..].find(" as ").map(|i| from + i) {
                    let value = operand(line, at);
                    let target = &line[at + 4..];
                    if kind(line, value, target, &brands).is_none() {
                        unexplained.push(format!("{file}: `{value} as {}` in\n    {}", target.split([';', ',']).next().unwrap_or(""), line.trim()));
                    }
                    from = at + 4;
                }
            }
        }
    }
    assert!(files > 200, "read {files} files");
    assert!(unexplained.is_empty(), "casts of no known kind:\n{}", unexplained.join("\n"));
}

/// The kinds are narrow enough to catch a forged brand or an unchecked
/// number.
#[test]
fn a_cast_of_no_sound_kind_is_caught() {
    let brands: BTreeSet<String> = ["Yen".to_string()].into();
    let unexplained = |line: &str| {
        let at = line.find(" as ").expect("a cast");
        kind(line, operand(line, at), &line[at + 4..], &brands).is_none()
    };
    // A brand outside its constructor.
    assert!(unexplained("  const y = amount as Yen;"));
    // A computed number into an integer brand.
    assert!(unexplained("  const n = (a + b) as I32;"));
    assert!(unexplained("  const n = f(x) as U8;"));
    // A string into a `Char`.
    assert!(unexplained("  const c = s as Char;"));
    // What the generator does print.
    assert!(!unexplained("export const Yen$of = (value: I64): Yen => value as Yen;"));
    assert!(!unexplained("  return Int.i32.add(n, (1 as I32));"));
    assert!(!unexplained("  const s = { kind: \"A\" } as State;"));
    // A place whose type is already the target: the printer must not emit this.
    assert!(unexplained("  const s = state as State;"));
    assert!(unexplained("  const m = method as PaymentMethod | null;"));
}
