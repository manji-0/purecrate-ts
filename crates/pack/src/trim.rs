//! A package's copy of the runtime keeps what the package uses. The runtime
//! marks its parts: `// #region <uses>` .. `// #endregion`, and a trailing
//! `// #needs <uses>` on one line, each kept when any listed use is found in
//! the package's other files. A use is `bits.<ty>` or `methods.<ty>` for an
//! integer type's operators or methods, `str.<member>`, `char.is` or `char`,
//! `uuid`, `json`, or `parseJson`.

use std::collections::BTreeSet;

use purecrate_ir::IntMethod;

/// Parts every package keeps: its index exports them to callers.
pub const EXPORTED: [&str; 4] = ["char", "char.is", "uuid", "parseJson"];

/// The runtime parts `sources` read: what they name after `Int.<ty>.`,
/// `Str.`, `Char.`, `Uuid.`, `Json.`, and `parseJson`.
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
            if matches!(op, "and" | "or" | "xor" | "not" | "shl" | "shr") {
                out.insert(format!("bits.{ty}"));
            } else if IntMethod::ALL.iter().any(|m| m.ts_name() == op) {
                out.insert(format!("methods.{ty}"));
            }
        }
        for (prefix, name) in [("Str.", "str"), ("Char.", "char"), ("Uuid.", "uuid"), ("Json.", "json")] {
            for (at, _) in source.match_indices(prefix) {
                if !starts_word(source, at) {
                    continue;
                }
                let member = ident(&source[at + prefix.len()..]);
                out.insert(match name {
                    "str" => format!("str.{member}"),
                    "char" if member == "is" => "char.is".to_string(),
                    other => other.to_string(),
                });
            }
        }
        if source.match_indices("parseJson").any(|(at, _)| starts_word(source, at)) {
            out.insert("parseJson".to_string());
        }
    }
    out
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
        match line.split_once(" // #needs ") {
            Some((code, list)) if wanted(list) => out.push_str(code),
            Some(_) => continue,
            None => out.push_str(line),
        }
        out.push('\n');
    }
    assert!(kept.is_empty(), "every `// #region` is closed");
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
        assert_eq!(
            found,
            set(&["bits.u8", "char", "char.is", "json", "methods.i64", "parseJson", "str.slice", "uuid"])
        );
    }

    #[test]
    fn every_use_keeps_the_whole_runtime_less_its_markers() {
        let all = set(&[
            "bits.i8", "bits.i16", "bits.i32", "bits.u8", "bits.u16", "bits.u32", "bits.i64", "bits.u64",
            "methods.i8", "methods.i16", "methods.i32", "methods.u8", "methods.u16", "methods.u32",
            "methods.usize", "methods.i64", "methods.u64", "str.bytes", "str.len", "str.slice",
            "str.stripPrefix", "str.stripSuffix", "str.cmp", "char", "char.is", "uuid", "json", "parseJson",
        ]);
        let full = trim(RUNTIME, &all);
        assert!(!full.contains("#region") && !full.contains("#endregion") && !full.contains("#needs"));
        let bare: String = RUNTIME
            .lines()
            .filter(|l| !l.trim_start().starts_with("// #region") && l.trim_start() != "// #endregion")
            .map(|l| l.split_once(" // #needs ").map_or(l, |(code, _)| code).to_string() + "\n")
            .collect();
        assert_eq!(full, bare);
    }

    #[test]
    fn no_use_keeps_the_types_and_the_base_operators() {
        let none = trim(RUNTIME, &BTreeSet::new());
        assert!(none.contains("export type Usize") && none.contains("export const Int = {") && none.contains("...small<I8>"));
        for gone in ["methods(", "bits32<", "Grapheme", "parseStr", "ryu", "INTEGER_LITERAL", "digitValue", "/** `str::"] {
            assert!(!none.contains(gone), "{gone} is kept");
        }
    }
}
