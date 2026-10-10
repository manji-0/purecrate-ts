//! A package's copy of the runtime keeps what the package uses. The runtime
//! marks its parts: `// #region <uses>` .. `// #endregion`, and a trailing
//! `// #needs <uses>` on one line, each kept when any listed use is found in
//! the package's other files. A use is `bits.<ty>`, `methods.<ty>` (or
//! `minmax.<ty>`), or `parse.<ty>` for an integer type's operators, methods,
//! or `str::parse`; `op.<op>` for `Int.<ty>.add` and the other operators,
//! `m.<method>` for `Int.<ty>.checkedAdd` and the other methods; `str.<member>`,
//! `slice.<member>`, `ord.<member>`, `iter.<member>`, `char.is` or `char`,
//! `uuid`, `json`, `parseJson`, or `parseIntError`; or an identifier read in
//! code (`I32`, `Result`, `Uuid`, a helper such as `panic`); `int.<ty>` for
//! `Int.<ty>`, read or reached by a brand the index exports. `trim_closed`
//! keeps what the kept runtime reads in turn, until nothing more is read.

use std::collections::BTreeSet;

use purecrate_ir::IntMethod;

/// The parts an index exports to callers, kept whole: `Char` with all its
/// methods, `Uuid`, `parseJson`, and `Int.<ty>` for each brand it exports.
pub fn exported(index: &str) -> BTreeSet<String> {
    let names: BTreeSet<&str> = index
        .lines()
        .filter(|l| (l.starts_with("export {") || l.starts_with("export type {")) && l.ends_with("from \"purecrate\";"))
        .flat_map(|l| l[l.find('{').map_or(0, |b| b + 1)..l.find('}').unwrap_or(l.len())].split(','))
        .map(|n| n.trim().trim_start_matches("type ").trim())
        .collect();
    let mut out = BTreeSet::new();
    if names.contains("Char") {
        out.extend(["char".to_string(), "char.is".to_string()]);
    }
    if names.contains("Uuid") {
        out.insert("uuid".to_string());
    }
    if names.contains("parseJson") {
        out.insert("parseJson".to_string());
    }
    // A brand the index exports: callers make its values with `Int.<ty>.of`.
    for ty in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "usize", "f32", "f64"] {
        if brand(ty).is_some_and(|b| names.contains(b)) {
            out.insert(format!("int.{ty}"));
        }
    }
    out
}

/// The runtime parts `sources` read: what they name after `Int.<ty>.`,
/// `Str.`, `Char.`, `Uuid.`, `Json.`, and `parseJson` and `ParseIntError`.
pub fn uses<'a>(sources: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for source in sources {
        for (at, _) in source.match_indices("Int.") {
            if !starts_word(source, at) {
                continue;
            }
            let rest = &source[at + 4..];
            let ty = ident(rest);
            let op = rest[ty.len()..].strip_prefix('.').map(ident).unwrap_or("");
            if brand(ty).is_some() {
                out.insert(format!("int.{ty}"));
            }
            if matches!(op, "add" | "sub" | "mul" | "div" | "rem" | "neg") {
                out.insert(format!("op.{op}"));
            } else if matches!(op, "and" | "or" | "xor" | "not" | "shl" | "shr") {
                out.insert(format!("bits.{ty}"));
            } else if op == "parse" {
                out.insert(format!("parse.{ty}"));
            } else if IntMethod::ALL.iter().any(|m| m.ts_name() == op) {
                let factory = if matches!(op, "min" | "max") { "minmax" } else { "methods" };
                out.insert(format!("{factory}.{ty}"));
                out.insert(format!("m.{op}"));
            }
        }
        for (prefix, name) in [
            ("Str.", "str"),
            ("Slice.", "slice"),
            ("Ord.", "ord"),
            ("Iter.", "iter"),
            ("Char.", "char"),
            ("Uuid.", "uuid"),
            ("Json.", "json"),
        ] {
            for (at, _) in source.match_indices(prefix) {
                if !starts_word(source, at) {
                    continue;
                }
                let member = ident(&source[at + prefix.len()..]);
                out.insert(match name {
                    "str" | "slice" | "ord" | "iter" => format!("{name}.{member}"),
                    "char" if member == "is" => "char.is".to_string(),
                    other => other.to_string(),
                });
            }
        }
        for (word, used) in [("parseJson", "parseJson"), ("ParseIntError", "parseIntError")] {
            if source.match_indices(word).any(|(at, _)| starts_word(source, at)) {
                out.insert(used.to_string());
            }
        }
        out.extend(code_words(source));
    }
    out
}

/// The brand of an integer or float namespace of `Int` (`i32` → `I32`).
fn brand(ty: &str) -> Option<&'static str> {
    Some(match ty {
        "i8" => "I8",
        "i16" => "I16",
        "i32" => "I32",
        "i64" => "I64",
        "u8" => "U8",
        "u16" => "U16",
        "u32" => "U32",
        "u64" => "U64",
        "usize" => "Usize",
        "f32" => "F32",
        "f64" => "F64",
        _ => return None,
    })
}

/// Identifiers in the code of a TS source: outside comments, string
/// literals, and the text of template literals (their `${..}` is code).
fn code_words(src: &str) -> BTreeSet<String> {
    let b = src.as_bytes();
    let mut out = BTreeSet::new();
    // Open template literals, each with the brace depth of its `${..}`.
    let mut templates: Vec<usize> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    let template_text = |mut i: usize, templates: &mut Vec<usize>, depth: &mut usize| -> usize {
        while i < b.len() {
            match b[i] {
                b'\\' => i += 2,
                b'`' => {
                    *depth = templates.pop().unwrap_or(0);
                    return i + 1;
                }
                b'$' if b.get(i + 1) == Some(&b'{') => {
                    *depth = 0;
                    return i + 2;
                }
                _ => i += 1,
            }
        }
        i
    };
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() && b[i] != q {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b'`' => {
                templates.push(depth);
                depth = 0;
                i = template_text(i + 1, &mut templates, &mut depth);
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' if depth == 0 && !templates.is_empty() => i = template_text(i + 1, &mut templates, &mut depth),
            b'}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$') {
                    i += 1;
                }
                // A property (`x.add`) names no binding; a spread (`...x`) does.
                let property = start > 0 && b[start - 1] == b'.' && !(start > 2 && &b[start - 3..start] == b"...");
                if !property {
                    out.insert(src[start..i].to_string());
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// `trim`, then what the kept code reads keeps more, until it reads
/// nothing new: a helper the kept code calls (`panic`), a brand it names.
pub fn trim_closed(source: &str, uses: &BTreeSet<String>) -> String {
    let mut uses = uses.clone();
    loop {
        let kept = trim(source, &uses);
        let more = self::uses([kept.as_str()]);
        if more.is_subset(&uses) {
            return kept;
        }
        uses.extend(more);
    }
}

/// `source` without the parts no use in `uses` asks for, and without the
/// markers.
pub fn trim(source: &str, uses: &BTreeSet<String>) -> String {
    let wanted = |list: &str| list.split_whitespace().any(|u| uses.contains(u));
    let mut kept: Vec<bool> = Vec::new();
    let mut out = String::new();
    for line in source.lines() {
        let bare = line.trim_start();
        if let Some(list) = bare.strip_prefix("// #region ") {
            kept.push(wanted(list));
            continue;
        }
        if bare == "// #endregion" {
            kept.pop().expect("a `// #endregion` closes a `// #region`");
            continue;
        }
        if kept.contains(&false) {
            continue;
        }
        let code = match line.split_once(" // #needs ") {
            Some((code, list)) if wanted(list) => code,
            Some(_) => continue,
            None => line,
        };
        // A part left out leaves the blank lines around it; keep one.
        if code.is_empty() && (out.is_empty() || out.ends_with("\n\n")) {
            continue;
        }
        out.push_str(code);
        out.push('\n');
    }
    assert!(kept.is_empty(), "every `// #region` is closed");
    drop_empty_namespaces(&out)
}

/// `export const Iter = { } as const` (and its JSDoc) when every member was
/// trimmed: a namespace with no members is not a runtime API.
fn drop_empty_namespaces(src: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut skip = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.starts_with("export const ")
            && t.ends_with(" = {")
            && i + 1 < lines.len()
            && lines[i + 1].trim() == "} as const;"
        {
            let mut start = i;
            while start > 0 && lines[start - 1].trim().is_empty() {
                start -= 1;
            }
            if start > 0 && lines[start - 1].trim().ends_with("*/") {
                let mut j = start - 1;
                while j > 0 && !lines[j].trim_start().starts_with("/*") {
                    j -= 1;
                }
                if lines[j].trim_start().starts_with("/*") {
                    start = j;
                    while start > 0 && lines[start - 1].trim().is_empty() {
                        start -= 1;
                    }
                }
            }
            skip[start..=i + 1].fill(true);
            i += 2;
            continue;
        }
        i += 1;
    }
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if skip[i] {
            continue;
        }
        if line.is_empty() && (out.is_empty() || out.ends_with("\n\n")) {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn starts_word(source: &str, at: usize) -> bool {
    source[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$' || c == '.'))
}

fn ident(s: &str) -> &str {
    let end = s.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(s.len());
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNTIME: &str = include_str!("../../../packages/boundary/src/index.ts");

    fn set(xs: &[&str]) -> BTreeSet<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn uses_are_read_from_names() {
        let found = uses([
            "Int.i32.add(a, b); Int.u8.shl(x, n); Int.i64.checkedMul(a, b); globalThis.BigInt.asIntN(64, x);",
            "Str.slice(s, a); Char.is(v); Char.code(c); Uuid.parseStr(s); Json.f64(x); parseJson(t); myparseJson()",
        ]);
        let marked: BTreeSet<String> =
            found.iter().filter(|u| u.contains('.') || *u == "char" || *u == "json" || *u == "uuid").cloned().collect();
        assert_eq!(
            marked,
            set(&[
                "bits.u8",
                "char",
                "char.is",
                "int.i32",
                "int.i64",
                "int.u8",
                "json",
                "m.checkedMul",
                "methods.i64",
                "op.add",
                "str.slice",
                "uuid",
            ])
        );
        // Every name read in code is a use; a property or a longer name is not.
        assert!(found.contains("parseJson") && found.contains("Int") && found.contains("myparseJson"));
        assert!(!found.contains("add") && !found.contains("asIntN"));
    }

    #[test]
    fn code_words_skip_comments_strings_and_template_text() {
        let words = code_words("// I8\n/* U8 */ const a = \"I16\" + `I32 ${b(I64)} U16`; c.d;");
        assert_eq!(words, set(&["I64", "a", "b", "c", "const"]));
    }

    #[test]
    fn what_the_kept_code_reads_is_kept_in_turn() {
        // `Int.i32.add` keeps `small`, which keeps `panic` and the `I32` brand.
        let out = trim_closed(RUNTIME, &set(&["int.i32", "op.add"]));
        assert!(out.contains("const small =") && out.contains("const panic =") && out.contains("export type I32"));
        assert!(out.contains("add: (a: T, b: T)") && !out.contains("sub: (a: T, b: T)"));
        assert!(
            !out.contains("const big =") && !out.contains("export type I64") && !out.contains("export type Result")
        );
    }

    #[test]
    fn every_use_keeps_the_whole_runtime_less_its_markers() {
        let all = set(&[
            "bits.i8",
            "bits.i16",
            "bits.i32",
            "bits.u8",
            "bits.u16",
            "bits.u32",
            "bits.i64",
            "bits.u64",
            "methods.i8",
            "methods.i16",
            "methods.i32",
            "methods.u8",
            "methods.u16",
            "methods.u32",
            "methods.usize",
            "methods.i64",
            "methods.u64",
            "str.bytes",
            "str.len",
            "str.slice",
            "str.stripPrefix",
            "str.stripSuffix",
            "str.splitOnce",
            "str.eqIgnoreAsciiCase",
            "str.wellFormed",
            "str.cmp",
            "slice.at",
            "slice.range",
            "slice.insert",
            "slice.remove",
            "slice.set",
            "ord.cmp",
            "ord.cmpStr",
            "ord.cmpList",
            "ord.then",
            "iter.all",
            "iter.any",
            "iter.position",
            "iter.count",
            "iter.sum",
            "iter.map",
            "iter.filter",
            "iter.tryCollect",
            "char",
            "char.is",
            "uuid",
            "json",
            "parseJson",
            "parseIntError",
            "parse.i8",
            "parse.i16",
            "parse.i32",
            "parse.u8",
            "parse.u16",
            "parse.u32",
            "parse.usize",
            "parse.i64",
            "parse.u64",
            "Result",
            "Char",
            "Uuid",
            "UuidError",
            "panic",
            "small",
            "big",
            "op.add",
            "op.sub",
            "op.mul",
            "op.div",
            "op.rem",
            "op.neg",
            "I8",
            "I16",
            "I32",
            "I64",
            "U8",
            "U16",
            "U32",
            "U64",
            "Usize",
            "F32",
            "F64",
            "int.i8",
            "int.i16",
            "int.i32",
            "int.i64",
            "int.u8",
            "int.u16",
            "int.u32",
            "int.u64",
            "int.usize",
            "int.f32",
            "int.f64",
        ]);
        let mut all = all;
        all.extend(IntMethod::ALL.iter().map(|m| format!("m.{}", m.ts_name())));
        all.extend(
            ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "usize"].iter().map(|t| format!("minmax.{t}")),
        );
        let full = trim(RUNTIME, &all);
        assert!(!full.contains("#region") && !full.contains("#endregion") && !full.contains("#needs"));
        let mut bare: String = RUNTIME
            .lines()
            .filter(|l| !l.trim_start().starts_with("// #region") && l.trim_start() != "// #endregion")
            .map(|l| l.split_once(" // #needs ").map_or(l, |(code, _)| code).to_string() + "\n")
            .collect();
        while bare.contains("\n\n\n") {
            bare = bare.replace("\n\n\n", "\n\n");
        }
        assert_eq!(full, bare);
    }

    #[test]
    fn a_part_left_out_leaves_no_run_of_blank_lines() {
        for uses in [BTreeSet::new(), set(&["uuid"]), set(&["json", "char"])] {
            let out = trim(RUNTIME, &uses);
            assert!(!out.contains("\n\n\n") && !out.starts_with('\n'), "{uses:?}");
        }
    }

    #[test]
    fn what_the_index_exports_is_kept() {
        let index = "export { Result, assertNever, Int, Char, Uuid, type UuidError } from \"purecrate\";\n\
                     export type { I8, I16 } from \"purecrate\";\n\
                     export { parseJson } from \"purecrate\";\n\
                     export { Event } from \"./event.ts\";\n";
        assert_eq!(exported(index), set(&["char", "char.is", "int.i16", "int.i8", "parseJson", "uuid"]));
        assert!(exported("export { Result, assertNever, Int } from \"purecrate\";\n").is_empty());
    }

    #[test]
    fn no_use_keeps_panic_and_assert_never_only() {
        let none = trim_closed(RUNTIME, &BTreeSet::new());
        assert!(none.contains("export class Panic") && none.contains("export const assertNever"));
        for gone in [
            "export type Usize",
            "export const Int",
            "small<",
            "export type Result",
            "export type Char",
            "export type Uuid",
            "methods(",
            "bits32<",
            "Grapheme",
            "parseStr",
            "parser(",
            "ParseIntError",
            "ryu",
            "INTEGER_LITERAL",
            "digitValue",
            "/** `str::",
            "xs[i] as T",
            "xs.slice(",
        ] {
            assert!(!none.contains(gone), "{gone} is kept");
        }
        for empty in [
            "export const Str",
            "export const Ord",
            "export const Iter",
            "export const Slice",
            "export const Char =",
            "export const Int =",
        ] {
            assert!(!none.contains(empty), "{empty} stays empty:\n{none}");
        }
    }
}
