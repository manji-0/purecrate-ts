//! Reading printed TS: parenthesis depth outside literals, operator
//! precedence, and display width.

/// The byte ranges of `s` outside string and template literals, with the
/// parenthesis depth at each byte (`None` inside a literal).
pub(super) fn depths(s: &str) -> Vec<Option<usize>> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                out.push(None);
                if b == b'\\' && i + 1 < bytes.len() {
                    out.push(None);
                    i += 2;
                    continue;
                }
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' | b'`' => {
                    quote = Some(b);
                    out.push(None);
                }
                // A `/* .. */` comment reads as a literal.
                b'/' if bytes.get(i + 1) == Some(&b'*') => {
                    let end = s[i + 2..].find("*/").map_or(bytes.len(), |e| i + 2 + e + 2);
                    out.extend(std::iter::repeat_n(None, end - i));
                    i = end;
                    continue;
                }
                // So does a `//` comment, to the end of its line.
                b'/' if bytes.get(i + 1) == Some(&b'/') => {
                    let end = s[i..].find('\n').map_or(bytes.len(), |e| i + e);
                    out.extend(std::iter::repeat_n(None, end - i));
                    i = end;
                    continue;
                }
                b'(' | b'[' | b'{' => {
                    out.push(Some(depth));
                    depth += 1;
                }
                b')' | b']' | b'}' => {
                    depth = depth.saturating_sub(1);
                    out.push(Some(depth));
                }
                // Type arguments, `Result<A, B>`: the printer writes a
                // comparison with spaces around `<` and `>`, and `=>` is not
                // a bracket.
                b'<' if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || matches!(bytes[i - 1], b'_' | b'$')) => {
                    out.push(Some(depth));
                    depth += 1;
                }
                b'>' if i > 0 && !matches!(bytes[i - 1], b'=' | b' ') && depth > 0 => {
                    depth -= 1;
                    out.push(Some(depth));
                }
                _ => out.push(Some(depth)),
            },
        }
        i += 1;
    }
    out
}

/// `s` without the parentheses around all of it, as often as they wrap it.
/// A template literal's `${..}` counts as inside the literal, so it is left
/// alone; that only keeps a pair of parentheses that could have gone.
pub(crate) fn strip_outer(mut s: &str) -> &str {
    loop {
        let t = s.trim();
        if !(t.starts_with('(') && t.ends_with(')')) {
            return t;
        }
        let d = depths(t);
        // The first `(` closes at the last byte exactly when no byte in
        // between is back at depth 0.
        let closes_at_end = d[1..d.len() - 1].iter().all(|x| x.is_none_or(|x| x >= 1));
        if !closes_at_end {
            return t;
        }
        s = &t[1..t.len() - 1];
    }
}

/// JS/TS operator precedence; higher binds tighter. Used to parenthesize
/// nested operators only when the child would parse otherwise.
pub(crate) const PREC_ATOMIC: u8 = 20;

pub(crate) const PREC_UNARY: u8 = 15;

pub(crate) const PREC_MUL: u8 = 14;

pub(crate) const PREC_ADD: u8 = 13;

pub(crate) const PREC_REL: u8 = 11;

pub(crate) const PREC_EQ: u8 = 10;

pub(crate) const PREC_AND: u8 = 6;

pub(crate) const PREC_OR: u8 = 5;

/// `??` binds looser than `||` but may not stand beside `||` or `&&`
/// without parentheses: JS rejects `a ?? b || c` (`needs_paren`).
pub(crate) const PREC_COALESCE: u8 = 4;

pub(crate) const PREC_TERNARY: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Assoc {
    Left,
    Right,
}

/// Whether a child of `child` precedence needs parentheses under a parent
/// of `parent` precedence. `&&` / `||` are associative, and an `&&` under
/// `||` is parenthesized all the same; `?:` is right-associative;
/// the rest are left-associative (`a - (b - c)` keeps its grouping).
pub(crate) fn needs_paren(child: u8, parent: u8, assoc: Assoc, side: Side) -> bool {
    // `a && b || c` reads as `(a && b) || c`, as oxfmt writes it.
    if child == PREC_AND && parent == PREC_OR {
        return true;
    }
    // `??` beside `||` or `&&` is a syntax error without them.
    let logical = |p| p == PREC_AND || p == PREC_OR;
    if child == PREC_COALESCE && logical(parent) || logical(child) && parent == PREC_COALESCE {
        return true;
    }
    if child > parent || child == PREC_ATOMIC {
        return false;
    }
    if child < parent {
        return true;
    }
    if child == PREC_AND || child == PREC_OR {
        return false;
    }
    match (assoc, side) {
        (Assoc::Left, Side::Right) | (Assoc::Right, Side::Left) => true,
        (Assoc::Left, Side::Left) | (Assoc::Right, Side::Right) => false,
    }
}

/// Lowest-precedence operator at depth 0 of `s`. A fully parenthesized or
/// atomic expression is `PREC_ATOMIC`.
pub(crate) fn top_prec(s: &str) -> u8 {
    let s = s.trim();
    let d = depths(s);
    let mut min = PREC_ATOMIC;
    let mut i = 0;
    while i < s.len() {
        if d.get(i) != Some(&Some(0)) {
            i += 1;
            continue;
        }
        let rest = &s[i..];
        let hit = [
            (" !== ", PREC_EQ),
            (" === ", PREC_EQ),
            (" ?? ", PREC_COALESCE),
            (" || ", PREC_OR),
            (" && ", PREC_AND),
            (" <= ", PREC_REL),
            (" >= ", PREC_REL),
            (" < ", PREC_REL),
            (" > ", PREC_REL),
            (" + ", PREC_ADD),
            (" - ", PREC_ADD),
            (" * ", PREC_MUL),
            (" / ", PREC_MUL),
            (" % ", PREC_MUL),
            (" ? ", PREC_TERNARY),
        ]
        .into_iter()
        .find(|(op, _)| rest.starts_with(op));
        if let Some((op, prec)) = hit {
            min = min.min(prec);
            i += op.len();
        } else {
            i += 1;
        }
    }
    if min == PREC_ATOMIC && s.starts_with('!') {
        PREC_UNARY
    } else {
        min
    }
}

/// `T` where `s` is one operand cast to it (`512 as Usize`, `b.length as
/// Usize`, `x as number as U8`): the type the declaration of `s` has.
pub(crate) fn cast_type(s: &str) -> Option<&str> {
    let s = strip_outer(s);
    let d = depths(s);
    let at = (0..s.len()).rev().find(|&i| d[i] == Some(0) && s[i..].starts_with(" as "))?;
    (top_prec(&s[..at]) == PREC_ATOMIC).then(|| s[at + 4..].trim())
}

/// The columns `s` takes, as oxfmt counts them: a wide East Asian character
/// takes two (`金` is two, `a` one).
pub(crate) fn cols(s: &str) -> usize {
    s.chars().map(|c| if wide(c) { 2 } else { 1 }).sum()
}

/// East Asian Wide and Fullwidth: Hangul Jamo, CJK and its punctuation,
/// Hiragana, Katakana, Hangul syllables, compatibility ideographs, vertical
/// and fullwidth forms, and the supplementary ideographic planes.
pub(super) fn wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD)
}
