//! Plain names for what the compiler made. While the code is built, a name
//! `check` or the printer makes starts with `$` (`$matched_2`,
//! `$majorResult`), and `rename` numbers a shadowed local `x$1`: Rust
//! identifiers have no `$`, so none of these meets a source name. Last, each
//! becomes a plain name: the `$` and a trailing `_<n>` dropped (`matched`),
//! a shadow counted from two (`x$1` → `x2`), and a number added where the
//! name is taken. Names other files import (`Version$of`, `Yen$raw`) have
//! no leading `$` and no `$<n>` end, and stay.
//!
//! A name is taken where the made one appears: in the innermost `{ .. }`
//! holding every use of it, which holds its declaration's scope. Any other
//! identifier read or declared there is taken, so nothing is captured or
//! hidden; so is a name given to a made one whose block overlaps.

use std::collections::BTreeMap;

/// `src` with every compiler-made name in its code replaced.
pub(crate) fn plain_names(src: &str) -> String {
    let spans: Vec<(usize, usize)> = crate::imports::ident_spans(src)
        .into_iter()
        .filter(|(_, _, after_dot)| !after_dot)
        .map(|(start, end, _)| (start, end))
        .collect();
    let blocks = crate::imports::brace_spans(src);
    // Each made name, in order of first use, with its first and last use.
    let mut uses: Vec<(&str, usize, usize)> = Vec::new();
    for &(start, end) in &spans {
        let word = &src[start..end];
        if made(word).is_none() {
            continue;
        }
        match uses.iter_mut().find(|(w, _, _)| *w == word) {
            Some(u) => u.2 = end,
            None => uses.push((word, start, end)),
        }
    }
    if uses.is_empty() {
        return src.to_string();
    }
    let mut given: Vec<((usize, usize), String)> = Vec::new();
    let mut renamed: BTreeMap<&str, String> = BTreeMap::new();
    for (word, first, last) in uses {
        let region = blocks
            .iter()
            .filter(|(open, close)| *open <= first && last <= *close)
            .min_by_key(|(open, close)| close - open)
            .copied()
            .unwrap_or((0, src.len()));
        let taken = |name: &str| {
            RESERVED.contains(&name)
                || spans.iter().any(|&(start, end)| region.0 <= start && end <= region.1 && &src[start..end] == name)
                || given.iter().any(|((open, close), n)| n == name && *open < region.1 && region.0 < *close)
        };
        let (base, from) = made(word).expect("a made name");
        let name = (from..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base}{n}") })
            .find(|n| !taken(n))
            .expect("a free name");
        given.push((region, name.clone()));
        renamed.insert(word, name);
    }
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for &(start, end) in &spans {
        if let Some(name) = renamed.get(&src[start..end]) {
            out.push_str(&src[at..start]);
            out.push_str(name);
            at = end;
        }
    }
    out.push_str(&src[at..]);
    out
}

/// The wire file's own names, plain: a domain type and its companion,
/// imported beside the schema of the same name (`Yen$` → `DomainYen`), an
/// arktype shape (`Yen$wire` → `yenWire`), and a variant's arm
/// (`Method$arm$Card` → `methodCardArm`). They are top-level, so each is
/// told apart from every other identifier in the file.
pub(crate) fn wire_name_map(src: &str) -> BTreeMap<String, String> {
    let spans: Vec<(usize, usize)> = crate::imports::ident_spans(src)
        .into_iter()
        .filter(|(_, _, after_dot)| !after_dot)
        .map(|(start, end, _)| (start, end))
        .collect();
    let lower = purecrate_ir::lower_first;
    let wire_base = |word: &str| -> Option<String> {
        let (ty, rest) = word.split_once('$')?;
        if !ty.starts_with(|c: char| c.is_ascii_uppercase()) {
            return None;
        }
        match rest {
            "" => Some(format!("Domain{ty}")),
            "wire" => Some(format!("{}Wire", lower(ty))),
            _ => rest.strip_prefix("arm$").map(|v| format!("{}{v}Arm", lower(ty))),
        }
    };
    let mut taken: std::collections::BTreeSet<String> = RESERVED.iter().map(|w| w.to_string()).collect();
    for &(start, end) in &spans {
        let word = &src[start..end];
        if wire_base(word).is_none() {
            taken.insert(word.to_string());
        }
    }
    let mut renamed: BTreeMap<String, String> = BTreeMap::new();
    for &(start, end) in &spans {
        let word = &src[start..end];
        if renamed.contains_key(word) {
            continue;
        }
        let Some(base) = wire_base(word) else { continue };
        let name = (1..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base}{n}") })
            .find(|n| !taken.contains(n))
            .expect("a free name");
        taken.insert(name.clone());
        renamed.insert(word.to_string(), name);
    }
    renamed
}

/// `src` with each identifier `map` names replaced; property names (after
/// a `.`), strings, and comments are left as they are.
pub(crate) fn rename_idents(src: &str, map: &BTreeMap<String, String>) -> String {
    if map.is_empty() {
        return src.to_string();
    }
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for (start, end, after_dot) in crate::imports::ident_spans(src) {
        if after_dot {
            continue;
        }
        if let Some(name) = map.get(&src[start..end]) {
            out.push_str(&src[at..start]);
            out.push_str(name);
            at = end;
        }
    }
    out.push_str(&src[at..]);
    out
}

/// The plain base of a compiler-made name, and the first number to try
/// (`n + 1` for a shadowed source name `x$n`, else `1` for the bare base).
/// `None` for any other identifier.
fn made(word: &str) -> Option<(String, usize)> {
    let (head, shadow) = match word.rsplit_once('$') {
        Some((head, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
            (head, Some(n.parse::<usize>().ok()?))
        }
        _ => (word, None),
    };
    let leading = head.starts_with('$');
    if !leading && shadow.is_none() {
        return None;
    }
    let mut base = head.trim_start_matches('$');
    // `_<n>` keeps temporaries of different depths apart; the renaming
    // here keeps distinct names apart on its own.
    if let Some((b, n)) = base.rsplit_once('_') {
        if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) {
            base = b;
        }
    }
    let base = camel(base);
    let base = if base.is_empty() { "value".to_string() } else { base };
    // `rename` numbers a made name it already holds (`$verified$1`) even
    // when the first never prints; only a source name's shadow counts on.
    let from = match shadow {
        Some(n) if !leading => n + 1,
        _ => 1,
    };
    Some((base, from))
}

/// `$` and `_` dropped, the letter after each upper-cased: `t$1` → `t1`,
/// `major_or` → `majorOr`.
fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in s.chars() {
        if c == '$' || c == '_' {
            upper = !out.is_empty();
            continue;
        }
        if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Words a binding may not be named, in strict-mode TS.
const RESERVED: &[&str] = &[
    "arguments",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn made_names_become_plain_and_apart() {
        let src = "const $matched_1 = f(x$1);\nif ($matched_1.kind) { const $matched_2 = x; }\nconst matched = 1;\n";
        assert_eq!(
            plain_names(src),
            "const matched2 = f(x2);\nif (matched2.kind) { const matched = x; }\nconst matched = 1;\n"
        );
    }

    #[test]
    fn a_name_is_taken_only_where_the_made_one_is_used() {
        let src = "const f = () => { const $v = g(); return $v; };\nconst h = () => { const v = 1; return v; };\n";
        assert_eq!(
            plain_names(src),
            "const f = () => { const v = g(); return v; };\nconst h = () => { const v = 1; return v; };\n"
        );
        // An outer name read inside the block is not hidden.
        let src = "const f = (v) => { const $v = g(); return $v + v; };\n";
        assert_eq!(plain_names(src), "const f = (v) => { const v2 = g(); return v2 + v; };\n");
    }

    #[test]
    fn a_for_head_is_its_loop_alone() {
        let src = "const f = () => {\n  for (let i = 0, $end_1 = a; i < $end_1; i++) { g(); }\n  for (let i = 0, $end_2 = b; i < $end_2; i++) { g(); }\n};\n";
        assert_eq!(
            plain_names(src),
            "const f = () => {\n  for (let i = 0, end = a; i < end; i++) { g(); }\n  for (let i = 0, end = b; i < end; i++) { g(); }\n};\n"
        );
    }

    #[test]
    fn strings_properties_and_shared_names_stay() {
        let src = "const $result = Version$of({ a: \"$x\" });\nreturn `${$result.value} $y`;\n";
        assert_eq!(plain_names(src), "const result = Version$of({ a: \"$x\" });\nreturn `${result.value} $y`;\n");
    }

    #[test]
    fn a_shadow_counts_from_two_past_what_is_taken() {
        assert_eq!(
            plain_names("const pre = 1; const pre$1 = pre; const pre2 = 3;"),
            "const pre = 1; const pre3 = pre; const pre2 = 3;"
        );
        assert_eq!(made("$major_or"), Some(("majorOr".into(), 1)));
        assert_eq!(made("$t$1"), Some(("t".into(), 1)));
        assert_eq!(made("pre$1"), Some(("pre".into(), 2)));
        assert_eq!(made("Yen$raw"), None);
        assert_eq!(made("A$"), None);
    }
}
