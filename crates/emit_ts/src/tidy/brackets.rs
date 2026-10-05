//! Breaking a long line inside a bracket: one item per line, a call's
//! arguments, an object type's fields.

use super::*;

/// A call whose last argument is an arrow with an expression body, the
/// other arguments plain: the arrow's head stays on the call's line and its
/// body goes one indent in (`Iter.all(xs, (b: U8): boolean =>` then the
/// body), where both fit; else every argument opens, as oxfmt does. oxfmt
/// keeps the head only for a body it can expand (a call, a `?:`, an `as`);
/// a body such as `a || b` opens every argument.
/// The top-level arguments of the call whose `(` is at `open` in `s`, and
/// the position of its `)`.
pub(super) fn call_args(s: &str, open: usize) -> Option<(Vec<&str>, usize)> {
    let d = depths(s);
    let base = d[open]?;
    let shut = match s.as_bytes()[open] {
        b'(' => b')',
        b'[' => b']',
        _ => b'}',
    };
    let close = (open + 1..s.len()).find(|&j| s.as_bytes()[j] == shut && d[j] == Some(base))?;
    let mut items = Vec::new();
    let mut start = open + 1;
    for j in open + 1..close {
        if s.as_bytes()[j] == b',' && d[j] == Some(base + 1) {
            items.push(s[start..j].trim());
            start = j + 1;
        }
    }
    let last = s[start..close].trim();
    if !last.is_empty() {
        items.push(last);
    }
    Some((items, close))
}

pub(super) fn wrap_bracket(line: &str, width: usize, require_excess: bool, out: &mut String) -> bool {
    let excess = (line.len() - width, require_excess);
    let excess = excess.1.then_some(excess.0);
    // A literal body's bracket, when the line up to it fits; else any.
    let body = split_point(line, excess, arrow_body(line, width)).filter(|p| p.0 < width);
    let Some((open, close, items, _sep)) = body.or_else(|| split_point(line, excess, 0)) else {
        return false;
    };
    let trimmed = line.trim_start();
    // A `?:` branch's contents sit one indent past its `?` / `:`.
    let branch = if trimmed.starts_with("? ") || trimmed.starts_with(": ") { "  " } else { "" };
    let pad = format!("{}{branch}", &line[..line.len() - trimmed.len()]);
    let pad = pad.as_str();
    if !require_excess {
        let first_fits = open < width;
        if !first_fits || items.len() < 2 {
            return false;
        }
    }
    wrap_line(&line[..=open], width, out);
    let generic = line.as_bytes().get(open) == Some(&b'<');
    let n = items.len();
    for (i, item) in items.into_iter().enumerate() {
        let comma = if generic && i + 1 == n { "" } else { "," };
        wrap_item(&format!("{pad}  {item}{comma}"), width, out);
    }
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
    true
}

/// A long `{ … }` or `( … )` with no top-level comma (a one-field object
/// or a single argument) still opens, so the contents can wrap.
pub(super) fn wrap_fat_group(line: &str, width: usize, out: &mut String) -> bool {
    let d = depths(line);
    let bytes = line.as_bytes();
    let mut best: Option<(usize, usize)> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if !matches!(b, b'(' | b'{') || d[i].is_none() {
            continue;
        }
        let level = d[i].expect("checked");
        let closer = if b == b'(' { b')' } else { b'}' };
        let Some(close) = (i + 1..bytes.len()).find(|&j| d[j] == Some(level) && bytes[j] == closer) else {
            continue;
        };
        if close - i <= width / 2 || i + 1 > width {
            continue;
        }
        if best.is_none_or(|(o, _)| i < o) {
            best = Some((i, close));
        }
    }
    let Some((open, close)) = best else {
        return false;
    };
    let inner = line[open + 1..close].trim();
    if inner.is_empty() {
        return false;
    }
    let trimmed = line.trim_start();
    let pad = &line[..line.len() - trimmed.len()];
    wrap_line(&line[..=open], width, out);
    let call = is_call(line[..open].trim_end());
    let trail = if line.as_bytes()[open] == b'{' || call { "," } else { "" };
    wrap_item(&format!("{pad}  {inner}{trail}"), width, out);
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
    true
}

/// The outermost call or parameter list on the line holding one item that
/// oxfmt does not hug (an object, array, or arrow is hugged): it opens, the
/// item one indent in with a trailing comma, so `unsafeMakeYen(` then
/// `Int.i64.add(..),` and `(` then `fields: Readonly<{ .. }>,`.
pub(super) fn wrap_sole_item(line: &str, width: usize, out: &mut String) -> bool {
    let d = depths(line);
    let bytes = line.as_bytes();
    let Some(open) = (0..bytes.len()).find(|&i| bytes[i] == b'(' && d[i] == Some(0)) else { return false };
    let Some(close) = (open + 1..bytes.len()).find(|&j| bytes[j] == b')' && d[j] == Some(0)) else { return false };
    let before = line[..open].trim_end();
    let call = is_call(before);
    let after = &line[close + 1..];
    // Parameters open only where the arrow's head does not fit; else its
    // body breaks (`wrap_arrow_first`, `wrap_bracket`).
    // A block body (`=> {`) never moves to the next line: its head is the
    // whole line.
    let block_body = line.trim_end().ends_with(" => {") && cols(line) > width;
    let params = before.ends_with('=')
        && (after.starts_with(": ") || after.starts_with(" =>"))
        && (arrow_split(line, width).is_none() || block_body);
    if !(call || params) || open + 1 > width {
        return false;
    }
    let item = line[open + 1..close].trim();
    let commas = (open + 1..close).any(|j| bytes[j] == b',' && d[j] == Some(1));
    let arrow = (open + 1..close).any(|j| d[j] == Some(1) && line[j..].starts_with(" => "));
    if item.is_empty() || commas || arrow || item.starts_with(['{', '[', '`']) {
        return false;
    }
    let pad = &line[..line.len() - line.trim_start().len()];
    wrap_line(&line[..=open], width, out);
    wrap_item(&format!("{pad}  {item},"), width, out);
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
    true
}

/// Whether a `(` after `before` opens a call's arguments, where a trailing
/// comma is allowed, rather than a group (`return (`, `!== (`) or a
/// statement's head (`if (`), where it is a syntax error, or with two items
/// the comma operator.
pub(super) fn is_call(before: &str) -> bool {
    if before.ends_with([')', ']']) {
        return true;
    }
    let word_start =
        before.rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '$'))).map_or(0, |i| i + 1);
    let word = &before[word_start..];
    const KEYWORDS: &[&str] = &[
        "if",
        "for",
        "while",
        "switch",
        "return",
        "throw",
        "typeof",
        "void",
        "delete",
        "case",
        "in",
        "of",
        "instanceof",
        "await",
        "yield",
        "else",
        "do",
        "catch",
    ];
    !word.is_empty() && !word.starts_with(|c: char| c.is_ascii_digit()) && !KEYWORDS.contains(&word)
}

/// `{ kind: "X"; a: T; b: U }` in a type, split on `;`.
pub(super) fn wrap_type_fields(line: &str, width: usize, out: &mut String) -> bool {
    wrap_bracket_sep(line, width, b';', out)
}

fn wrap_bracket_sep(line: &str, width: usize, sep: u8, out: &mut String) -> bool {
    let d = depths(line);
    let bytes = line.as_bytes();
    let mut best: Option<(usize, usize, usize)> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'{' || d[i].is_none() {
            continue;
        }
        let level = d[i].expect("checked");
        let Some(close) = (i + 1..bytes.len()).find(|&j| d[j] == Some(level) && bytes[j] == b'}') else {
            continue;
        };
        let top = |c: u8| (i + 1..close).any(|j| bytes[j] == c && d[j] == Some(level + 1));
        if top(sep) && !top(b',') && best.is_none_or(|(_, _, l)| level < l) {
            best = Some((i, close, level));
        }
    }
    let Some((open, close, level)) = best else {
        return false;
    };
    let trimmed = line.trim_start();
    let pad = &line[..line.len() - trimmed.len()];
    // A union member's contents sit past its `| `, its close under the `R`.
    let (item_pad, close_pad) = match trimmed.starts_with("| ") {
        true => (format!("{pad}    "), format!("{pad}  ")),
        false => (format!("{pad}  "), pad.to_string()),
    };
    let mut items = Vec::new();
    let mut start = open + 1;
    for j in open + 1..close {
        if bytes[j] == sep && d[j] == Some(level + 1) {
            items.push(line[start..j].trim().to_string());
            start = j + 1;
        }
    }
    let last = line[start..close].trim();
    if !last.is_empty() {
        items.push(last.to_string());
    }
    if items.len() < 2 || open + 1 > width {
        return false;
    }
    let ch = sep as char;
    wrap_line(&line[..=open], width, out);
    for item in items {
        wrap_line(&format!("{item_pad}{item}{ch}"), width, out);
    }
    wrap_line(&format!("{close_pad}{}", &line[close..]), width, out);
    true
}

/// A long `(…)` group whose contents can wrap: open it so a nested `?:` or
/// `&&` is not stuck behind an outer pair.
pub(super) fn wrap_outer_parens(line: &str, width: usize, out: &mut String) -> bool {
    let pad_len = line.len() - line.trim_start().len();
    let pad = &line[..pad_len];
    let mut rest = &line[pad_len..];
    let mut prefix = pad.to_string();
    for p in ["return ", "? ", ": "] {
        if let Some(r) = rest.strip_prefix(p) {
            prefix.push_str(p);
            rest = r;
            break;
        }
    }
    let (expr, trailer) = match rest.strip_suffix(';') {
        Some(e) => (e, ";"),
        None => (rest, ""),
    };
    let inner = strip_outer(expr);
    if inner.len() + 2 >= expr.len() || !expr.trim_start().starts_with('(') {
        return false;
    }
    wrap_line(&format!("{prefix}("), width, out);
    wrap_line(&format!("{pad}  {inner}"), width, out);
    wrap_line(&format!("{pad}){trailer}"), width, out);
    true
}

/// A `let a = x, b = y` (a `for` init) split on commas at depth 0.
pub(super) fn wrap_top_commas(line: &str, width: usize, out: &mut String) -> bool {
    let Some(parts) = split_at_op(line.trim_start(), ", ", 0) else {
        return false;
    };
    if parts.len() < 2 {
        return false;
    }
    let pad_len = line.len() - line.trim_start().len();
    let pad = &line[..pad_len];
    for part in parts {
        wrap_line(&format!("{pad}{part}"), width, out);
    }
    true
}

/// The brackets to open on `line` and the items between them: the first,
/// outermost pair closed on the line with a comma at its top level, and no
/// `;` there (a `for` header), optionally required to hold more than the
/// `excess` past the width so a small `Slice.at(b, i)` is not opened when
/// the overflow is elsewhere.
fn split_point(line: &str, excess: Option<usize>, from: usize) -> Option<(usize, usize, Vec<String>, char)> {
    let d = depths(line);
    let bytes = line.as_bytes();
    let mut best: Option<(usize, usize, usize)> = None;
    for (i, &b) in bytes.iter().enumerate().skip(from) {
        let generic =
            b == b'<' && i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || matches!(bytes[i - 1], b'_' | b'$'));
        if !(matches!(b, b'(' | b'[' | b'{') || generic) || d[i].is_none() {
            continue;
        }
        let level = d[i].expect("checked");
        let closer = match b {
            b'(' => b')',
            b'[' => b']',
            b'{' => b'}',
            _ => b'>',
        };
        let Some(close) = (i + 1..bytes.len()).find(|&j| d[j] == Some(level) && bytes[j] == closer) else {
            continue;
        };
        let top = |c: u8| (i + 1..close).any(|j| bytes[j] == c && d[j] == Some(level + 1));
        let long_enough = excess.is_none_or(|e| close - i > e);
        // An arrow's object body opens with one field too (`=> ({` then
        // `kind: "A",`), as oxfmt prints it.
        let body_object = from > 0 && b == b'{' && (i == from || i == from + 1) && close > i + 2;
        if (top(b',') || body_object) && !top(b';') && long_enough && best.is_none_or(|(_, _, l)| level < l) {
            best = Some((i, close, level));
        }
    }
    let (open, close, level) = best?;
    let mut items = Vec::new();
    let mut start = open + 1;
    for j in open + 1..close {
        if bytes[j] == b',' && d[j] == Some(level + 1) {
            items.push(line[start..j].trim().to_string());
            start = j + 1;
        }
    }
    let last = line[start..close].trim();
    if !last.is_empty() {
        items.push(last.to_string());
    }
    Some((open, close, items, ','))
}
