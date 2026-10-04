//! Small rewrites of printed TS that keep its meaning: the parentheses
//! around a whole condition, `return` value, or initializer, `!(a === b)` as
//! `a !== b`, parentheses by operator precedence, and long lines broken at
//! a comma-separated bracket, an `if (cond) return`, a long `&&` / `||` /
//! `?:`, or an arrow body. Each reads the text with strings, templates, and
//! comments skipped, so a bracket, comma, or operator inside one is never
//! taken for code.

/// The byte ranges of `s` outside string and template literals, with the
/// parenthesis depth at each byte (`None` inside a literal).
fn depths(s: &str) -> Vec<Option<usize>> {
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

/// `src` with each line longer than `width` broken: a comma-separated
/// bracket, a semicolon-separated type literal, `if (cond) return …`, a
/// long `&&` / `||` / `?:`, or an arrow body. Comment lines stay as they are.
pub(crate) fn wrap(src: &str, width: usize) -> String {
    let mut out = String::with_capacity(src.len());
    for line in src.lines() {
        wrap_line(line, width, &mut out);
    }
    out
}

fn emit_raw(line: &str, out: &mut String) {
    out.push_str(line);
    out.push('\n');
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("//") || t.starts_with("/*") || t.starts_with('*')
}

/// The columns `s` takes, as oxfmt counts them: a wide East Asian character
/// takes two (`金` is two, `a` one).
pub(crate) fn cols(s: &str) -> usize {
    s.chars().map(|c| if wide(c) { 2 } else { 1 }).sum()
}

/// East Asian Wide and Fullwidth: Hangul Jamo, CJK and its punctuation,
/// Hiragana, Katakana, Hangul syllables, compatibility ideographs, vertical
/// and fullwidth forms, and the supplementary ideographic planes.
fn wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD)
}

/// `import { a as b } from "m";` with one name, which oxfmt keeps on its
/// line however long.
fn lone_import(line: &str) -> bool {
    line.starts_with("import ")
        && line
            .split_once('{')
            .and_then(|(_, rest)| rest.split_once('}'))
            .is_some_and(|(names, _)| !names.contains(','))
}

fn wrap_line(line: &str, width: usize, out: &mut String) {
    if !is_comment(line) && wrap_composition(line, width, out) {
        return;
    }
    if cols(line) <= width || is_comment(line) || lone_import(line) {
        emit_raw(line, out);
        return;
    }
    if wrap_if_return(line, width, out) {
        return;
    }
    if wrap_if_open(line, width, out) {
        return;
    }
    if wrap_for(line, width, out) {
        return;
    }
    if wrap_condition(line, width, out) {
        return;
    }
    if wrap_arrow_first(line, width, out) {
        return;
    }
    if wrap_assign_ternary(line, width, out) {
        return;
    }
    // A top-level `?:` splits before a bracket in one of its branches opens.
    if wrap_ternary(line, width, out) {
        return;
    }
    if wrap_after_assign(line, width, out) {
        return;
    }
    if wrap_last_arrow(line, width, out) {
        return;
    }
    if wrap_sole_item(line, width, out) {
        return;
    }
    if wrap_bracket(line, width, true, out) {
        return;
    }
    if wrap_bracket(line, width, false, out) {
        return;
    }
    if wrap_type_fields(line, width, out) {
        return;
    }
    if wrap_fat_group(line, width, out) {
        return;
    }
    if wrap_arrow(line, width, out) {
        return;
    }
    // A top-level `?:` binds looser than the `&&` / `||` in its operands, so
    // it splits first.
    if wrap_ternary(line, width, out) {
        return;
    }
    if wrap_logical(line, width, out) {
        return;
    }
    if wrap_outer_parens(line, width, out) {
        return;
    }
    if wrap_top_commas(line, width, out) {
        return;
    }
    emit_raw(line, out);
}

/// A call whose last argument is an arrow with an expression body, the
/// other arguments plain: the arrow's head stays on the call's line and its
/// body goes one indent in (`Iter.all(xs, (b: U8): boolean =>` then the
/// body), where both fit; else every argument opens, as oxfmt does. oxfmt
/// keeps the head only for a body it can expand (a call, a `?:`, an `as`);
/// a body such as `a || b` opens every argument.
/// The top-level arguments of the call whose `(` is at `open` in `s`, and
/// the position of its `)`.
fn call_args(s: &str, open: usize) -> Option<(Vec<&str>, usize)> {
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

/// Whether `arg` is a call one of whose arguments is an arrow.
fn calls_with_arrow(arg: &str) -> bool {
    let bytes = arg.as_bytes();
    let callee = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'.');
    match arg.find('(') {
        Some(open) if open > 0 && arg.ends_with(')') && bytes[..open].iter().all(|&b| callee(b)) => {
            call_args(arg, open).is_some_and(|(items, close)| {
                close == arg.len() - 1
                    && items.iter().any(|a| {
                        let d = depths(a);
                        a.match_indices(" => ").any(|(i, _)| d[i] == Some(0))
                    })
            })
        }
        _ => false,
    }
}

/// A call of two or more arguments, one of them a call taking an arrow
/// (`Iter.sum(Iter.map(xs, (x) => ..), add, 0)`), opens every argument,
/// each on its line, however short: oxfmt reads it as function composition.
/// As an arrow's body, it goes to the next line first.
fn wrap_composition(line: &str, width: usize, out: &mut String) -> bool {
    let d = depths(line);
    let bytes = line.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$');
    let pad = &line[..line.len() - line.trim_start().len()];
    // The outermost such call.
    let found = (1..line.len())
        .filter(|&i| bytes[i] == b'(' && ident(bytes[i - 1]) && d[i].is_some())
        .filter_map(|i| call_args(line, i).map(|(items, close)| (i, items, close)))
        .filter(|(_, items, _)| items.len() >= 2 && items.iter().any(|a| calls_with_arrow(a)))
        .min_by_key(|(i, _, _)| (d[*i], *i));
    let Some((open, items, close)) = found else { return false };
    // Inside other brackets, those open first: a break inside breaks them.
    if d[open] != Some(0) {
        let Some(outer) = (0..open).rev().find(|&j| matches!(bytes[j], b'(' | b'[' | b'{') && d[j] == Some(0)) else {
            return false;
        };
        let Some((outer_items, outer_close)) = call_args(line, outer) else { return false };
        let start = (0..outer).rev().find(|&j| !(ident(bytes[j]) || bytes[j] == b'.')).map_or(0, |j| j + 1);
        if line[..start].ends_with(" => ") && start > pad.len() + 4 {
            out.push_str(&line[..start - 1]);
            out.push('\n');
            wrap_line(&format!("{pad}  {}", &line[start..]), width, out);
            return true;
        }
        out.push_str(&line[..=outer]);
        out.push('\n');
        // A group's `(` (`=> (a ? b : c)`) takes no trailing comma: `(x,)`
        // is a syntax error.
        let comma = if bytes[outer] != b'(' || is_call(line[..outer].trim_end()) { "," } else { "" };
        for item in &outer_items {
            wrap_line(&format!("{pad}  {item}{comma}"), width, out);
        }
        wrap_line(&format!("{pad}{}", &line[outer_close..]), width, out);
        return true;
    }
    let start = (0..open).rev().find(|&j| !(ident(bytes[j]) || bytes[j] == b'.')).map_or(0, |j| j + 1);
    if line[..start].ends_with(" => ") && d[start] == Some(0) && start > pad.len() + 4 {
        out.push_str(&line[..start - 1]);
        out.push('\n');
        wrap_line(&format!("{pad}  {}", &line[start..]), width, out);
        return true;
    }
    out.push_str(&line[..=open]);
    out.push('\n');
    for item in &items {
        wrap_line(&format!("{pad}  {item},"), width, out);
    }
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
    true
}

/// An arrow body oxfmt may print after a hugged head: a call, a `?:`, or an
/// `as` at its top level.
fn expandable(body: &str) -> bool {
    let d = depths(body);
    let top = |pat: &str| body.match_indices(pat).any(|(i, _)| d[i] == Some(0));
    if top(" ? ") || top(" as ") {
        return true;
    }
    let bytes = body.as_bytes();
    let callee = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'.');
    match body.find('(') {
        Some(open) if open > 0 && body.ends_with(')') => {
            bytes[..open].iter().all(|&b| callee(b)) && (open + 1..body.len() - 1).all(|j| d[j] != Some(0))
        }
        _ => false,
    }
}

fn wrap_last_arrow(line: &str, width: usize, out: &mut String) -> bool {
    let (core, end) = match line.strip_suffix([';', ',']) {
        Some(c) => (c, &line[c.len()..]),
        None => (line, ""),
    };
    let pad = &line[..line.len() - line.trim_start().len()];
    let d = depths(core);
    let bytes = core.as_bytes();
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    let Some(open) = (1..core.len()).find(|&i| bytes[i] == b'(' && d[i] == Some(0) && ident(bytes[i - 1])) else {
        return false;
    };
    // The call is the rest of the line.
    if !core.ends_with(')') || (open + 1..core.len() - 1).any(|j| d[j] == Some(0)) {
        return false;
    }
    let inner = &core[open + 1..core.len() - 1];
    let di = depths(inner);
    let mut items = Vec::new();
    let mut start = 0;
    for j in 0..inner.len() {
        if inner.as_bytes()[j] == b',' && di[j] == Some(0) {
            items.push(inner[start..j].trim());
            start = j + 1;
        }
    }
    items.push(inner[start..].trim());
    let Some((last, rest)) = items.split_last() else { return false };
    let dl = depths(last);
    let Some(arrow) = (0..last.len()).find(|&i| dl[i] == Some(0) && last[i..].starts_with(" => ")) else {
        return false;
    };
    let body = &last[arrow + 4..];
    if body.starts_with(['{', '(', '[']) || rest.iter().any(|a| a.contains(" => ")) || !expandable(body) {
        return false;
    }
    let mut head = core[..=open].to_string();
    for a in rest {
        head.push_str(a);
        head.push_str(", ");
    }
    head.push_str(&last[..arrow]);
    head.push_str(" =>");
    let below = format!("{pad}  {body},");
    if cols(&head) > width || cols(&below) > width {
        return false;
    }
    emit_raw(&head, out);
    emit_raw(&below, out);
    emit_raw(&format!("{pad}){end}"), out);
    true
}

/// `const x = a.b(c);` whose value is a call oxfmt finds poorly breakable,
/// a name or a chain of fields called with nothing or one short operand (a
/// name, a field chain, or a literal of at most a quarter of the width):
/// broken after `=`, the value one indent in, where it then fits.
fn wrap_after_assign(line: &str, width: usize, out: &mut String) -> bool {
    let trimmed = line.trim_start();
    let pad = &line[..line.len() - trimmed.len()];
    if !(trimmed.starts_with("const ") || trimmed.starts_with("let ")) {
        return false;
    }
    let d = depths(line);
    let Some(eq) = (0..line.len()).find(|&i| d[i] == Some(0) && line[i..].starts_with(" = ")) else {
        return false;
    };
    let Some(value) = line[eq + 3..].strip_suffix(';') else { return false };
    let path = |s: &str| {
        !s.is_empty()
            && s.split('.')
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$'))
    };
    let short = |s: &str| {
        let s = s.trim();
        s.is_empty()
            || (cols(s) <= width / 4
                && (path(s) || s.chars().all(|c| c.is_ascii_digit()) || (s.starts_with('"') && s.ends_with('"'))))
    };
    let poor = match value.strip_suffix(')').and_then(|v| v.split_once('(')) {
        Some((callee, arg)) => path(callee) && !arg.contains(',') && !arg.contains('(') && short(arg),
        None => path(value),
    };
    let below = format!("{pad}  {value};");
    if !poor || cols(&below) > width {
        return false;
    }
    emit_raw(&line[..eq + 2], out);
    emit_raw(&below, out);
    true
}

/// `if (cond) return value;` when it does not fit: the condition on its
/// own `if` line when that fits, else opened, then `return` one indent in.
fn wrap_if_return(line: &str, width: usize, out: &mut String) -> bool {
    let Some((pad, cond, val)) = parse_if_return(line) else {
        return false;
    };
    let header = format!("{pad}if ({cond})");
    if cols(&header) <= width {
        wrap_line(&header, width, out);
    } else {
        wrap_line(&format!("{pad}if ("), width, out);
        wrap_line(&format!("{pad}  {cond}"), width, out);
        wrap_line(&format!("{pad})"), width, out);
    }
    wrap_line(&format!("{pad}  return {val};"), width, out);
    true
}

fn parse_if_return(line: &str) -> Option<(&str, &str, &str)> {
    let pad_len = line.len() - line.trim_start().len();
    let rest = &line[pad_len..];
    let inner = rest.strip_prefix("if (")?.strip_suffix(';')?;
    let d = depths(rest);
    let open = 3usize;
    let close = (open + 1..rest.len()).find(|&j| d[j] == Some(0) && rest.as_bytes()[j] == b')')?;
    let after = rest[close + 1..].strip_prefix(" return ")?;
    let val = after.strip_suffix(';')?;
    if val.is_empty() || val.contains('\n') {
        return None;
    }
    let _ = inner;
    Some((&line[..pad_len], &rest[open + 1..close], val))
}

/// `if (cond) {` whose condition does not fit on the line.
fn wrap_if_open(line: &str, width: usize, out: &mut String) -> bool {
    let pad_len = line.len() - line.trim_start().len();
    let rest = &line[pad_len..];
    let Some(inner) = rest.strip_prefix("if (") else {
        return false;
    };
    if !inner.ends_with(") {") {
        return false;
    }
    let d = depths(rest);
    let open = 3usize;
    let Some(close) = (open + 1..rest.len()).find(|&j| d[j] == Some(0) && rest.as_bytes()[j] == b')') else {
        return false;
    };
    if &rest[close..] != ") {" {
        return false;
    }
    let pad = &line[..pad_len];
    let cond = &rest[open + 1..close];
    wrap_line(&format!("{pad}if ("), width, out);
    wrap_line(&format!("{pad}  {cond}"), width, out);
    wrap_line(&format!("{pad}) {{"), width, out);
    true
}

/// `for (init; test; step) {` with the three clauses on their own lines.
fn wrap_for(line: &str, width: usize, out: &mut String) -> bool {
    let pad_len = line.len() - line.trim_start().len();
    let rest = &line[pad_len..];
    let Some(inner) = rest.strip_prefix("for (") else {
        return false;
    };
    if !inner.ends_with(") {") {
        return false;
    }
    let d = depths(rest);
    let open = 4usize;
    let Some(close) = (open + 1..rest.len()).find(|&j| d[j] == Some(0) && rest.as_bytes()[j] == b')') else {
        return false;
    };
    if &rest[close..] != ") {" {
        return false;
    }
    let header = &rest[open + 1..close];
    let hd = depths(header);
    let mut parts = Vec::new();
    let mut start = 0;
    for (j, &b) in header.as_bytes().iter().enumerate() {
        if b == b';' && hd[j] == Some(0) {
            parts.push(header[start..j].trim().to_string());
            start = j + 1;
        }
    }
    let last = header[start..].trim();
    if !last.is_empty() {
        parts.push(last.to_string());
    }
    if parts.len() != 3 {
        return false;
    }
    let pad = &line[..pad_len];
    wrap_line(&format!("{pad}for ("), width, out);
    wrap_line(&format!("{pad}  {};", parts[0]), width, out);
    wrap_line(&format!("{pad}  {};", parts[1]), width, out);
    wrap_line(&format!("{pad}  {}", parts[2]), width, out);
    wrap_line(&format!("{pad}) {{"), width, out);
    true
}

fn wrap_bracket(line: &str, width: usize, require_excess: bool, out: &mut String) -> bool {
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
        wrap_line(&format!("{pad}  {item}{comma}"), width, out);
    }
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
    true
}

/// A long `{ … }` or `( … )` with no top-level comma (a one-field object
/// or a single argument) still opens, so the contents can wrap.
fn wrap_fat_group(line: &str, width: usize, out: &mut String) -> bool {
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
    wrap_line(&format!("{pad}  {inner}{trail}"), width, out);
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
    true
}

/// The outermost call or parameter list on the line holding one item that
/// oxfmt does not hug (an object, array, or arrow is hugged): it opens, the
/// item one indent in with a trailing comma, so `unsafeMakeYen(` then
/// `Int.i64.add(..),` and `(` then `fields: Readonly<{ .. }>,`.
fn wrap_sole_item(line: &str, width: usize, out: &mut String) -> bool {
    let d = depths(line);
    let bytes = line.as_bytes();
    let Some(open) = (0..bytes.len()).find(|&i| bytes[i] == b'(' && d[i] == Some(0)) else { return false };
    let Some(close) = (open + 1..bytes.len()).find(|&j| bytes[j] == b')' && d[j] == Some(0)) else { return false };
    let before = line[..open].trim_end();
    let call = is_call(before);
    let after = &line[close + 1..];
    // Parameters open only where the arrow's head does not fit; else its
    // body breaks (`wrap_arrow_first`, `wrap_bracket`).
    let params = before.ends_with('=')
        && (after.starts_with(": ") || after.starts_with(" =>"))
        && arrow_split(line, width).is_none();
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
    wrap_line(&format!("{pad}  {item},"), width, out);
    wrap_line(&format!("{pad}{}", &line[close..]), width, out);
    true
}

/// Whether a `(` after `before` opens a call's arguments, where a trailing
/// comma is allowed, rather than a group (`return (`, `!== (`) or a
/// statement's head (`if (`), where it is a syntax error, or with two items
/// the comma operator.
fn is_call(before: &str) -> bool {
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
fn wrap_type_fields(line: &str, width: usize, out: &mut String) -> bool {
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

/// Break after `=>` when the head fits, so a long body can wrap on its own.
fn wrap_arrow(line: &str, width: usize, out: &mut String) -> bool {
    let d = depths(line);
    let bytes = line.as_bytes();
    let mut at = None;
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if d[i] == Some(0) && line[i..].starts_with(" => ") {
            at = Some(i);
        }
        i += 1;
    }
    let at = match at {
        Some(a) => a,
        None => return false,
    };
    let head = &line[..=at + 3];
    let body = line[at + 4..].trim();
    if body.is_empty() || head.len() > width {
        return false;
    }
    // A `?:` body is parenthesized on the arrow's line, bare below it.
    let (core, end) = match body.strip_suffix([';', ',']) {
        Some(c) => (c, &body[c.len()..]),
        None => (body, ""),
    };
    let inner = strip_outer(core);
    let body = if inner.len() < core.len() && is_ternary(inner) { format!("{inner}{end}") } else { body.to_string() };
    let pad_len = line.len() - line.trim_start().len();
    wrap_line(head.trim_end(), width, out);
    wrap_line(&format!("{}{body}", " ".repeat(pad_len + 2)), width, out);
    true
}

/// Split at the loosest of `||` / `&&` / `??` on the line.
fn wrap_logical(line: &str, width: usize, out: &mut String) -> bool {
    let Some(parts) = split_logical(line) else {
        return false;
    };
    let pad_len = line.len() - line.trim_start().len();
    let pad = &line[..pad_len];
    // A `?:` branch's test (`: a &&`) goes on four past its `:`, as oxfmt
    // lays out a chain of `?:`.
    let branch = line.trim_start().starts_with("? ") || line.trim_start().starts_with(": ");
    for (i, part) in parts.into_iter().enumerate() {
        let more = if branch && i > 0 { "    " } else { "" };
        wrap_line(format!("{pad}{more}{part}").trim_end(), width, out);
    }
    true
}

/// A condition on a line of its own (what `wrap_if_return` and
/// `wrap_if_open` leave between `if (` and `)`, or an arrow's body after
/// `wrap_arrow`) breaks at its top-level
/// `||`, else `&&`, before any call in it opens: `n === 0 ||` then
/// `Slice.at(b, start) === 45`, not `Slice.at(` alone.
fn wrap_condition(line: &str, width: usize, out: &mut String) -> bool {
    let expr = line.trim_start();
    let statement =
        ["return ", "const ", "let ", "? ", ": ", "if ", "for ", "while "].iter().any(|p| expr.starts_with(p));
    let d = depths(expr);
    let arrow = (0..expr.len()).any(|i| d[i] == Some(0) && expr[i..].starts_with(" => "));
    let ternary = find_ternary(expr).is_some();
    // A `;` ends an arrow's body that `wrap_arrow` moved to its own line.
    if statement || arrow || ternary || expr.ends_with(['{', ',', '(']) || top_assign(expr).is_some() {
        return false;
    }
    // The loosest operator at the top: `||`, then `&&`, then a comparison.
    let ops = [" || ", " && ", " === ", " !== ", " <= ", " >= ", " < ", " > "];
    let Some(parts) = ops.iter().find_map(|op| split_at_op(expr, op, 0)) else {
        return false;
    };
    let pad = &line[..line.len() - expr.len()];
    for part in parts {
        wrap_line(format!("{pad}{part}").trim_end(), width, out);
    }
    true
}

fn split_logical(line: &str) -> Option<Vec<String>> {
    let expr = line.trim_start();
    let d = depths(expr);
    let mut min_or: Option<usize> = None;
    let mut min_and: Option<usize> = None;
    let mut min_coalesce: Option<usize> = None;
    let mut i = 0;
    while i < expr.len() {
        if let Some(depth) = d.get(i).copied().flatten() {
            let rest = &expr[i..];
            if rest.starts_with(" || ") {
                min_or = Some(min_or.map_or(depth, |m| m.min(depth)));
                i += 4;
                continue;
            }
            if rest.starts_with(" && ") {
                min_and = Some(min_and.map_or(depth, |m| m.min(depth)));
                i += 4;
                continue;
            }
            if rest.starts_with(" ?? ") {
                min_coalesce = Some(min_coalesce.map_or(depth, |m| m.min(depth)));
                i += 4;
                continue;
            }
        }
        i += 1;
    }
    let (op, depth) = if let Some(d0) = min_or {
        (" || ", d0)
    } else if let Some(d0) = min_and {
        (" && ", d0)
    } else {
        (" ?? ", min_coalesce?)
    };
    split_at_op(expr, op, depth)
}

fn split_at_op(expr: &str, op: &str, depth: usize) -> Option<Vec<String>> {
    let d = depths(expr);
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i + op.len() <= expr.len() {
        if d.get(i).copied().flatten() == Some(depth) && expr[i..].starts_with(op) {
            let part = expr[start..i].trim();
            if !part.is_empty() {
                parts.push(format!("{part}{op}"));
            }
            i += op.len();
            start = i;
            continue;
        }
        i += 1;
    }
    let last = expr[start..].trim();
    if !last.is_empty() {
        parts.push(last.to_string());
    }
    (parts.len() >= 2).then_some(parts)
}

/// `const x: T = a === b ? c : d;` breaks after `=`, the `?:` one indent
/// in on a line of its own or split there, as oxfmt prints a conditional
/// whose test is a binary expression. A test that is a name or a `!`
/// stays on the `=` line (`wrap_ternary`).
fn wrap_assign_ternary(line: &str, width: usize, out: &mut String) -> bool {
    let pad = &line[..line.len() - line.trim_start().len()];
    let rest = line.trim_start();
    let Some(eq) = top_assign(rest) else { return false };
    let rhs = &rest[eq + 3..];
    let Some((test, _, _)) = find_ternary(rhs.trim_end_matches(';')) else { return false };
    if top_prec(test) >= PREC_UNARY {
        return false;
    }
    wrap_line(&format!("{pad}{} =", &rest[..eq]), width, out);
    wrap_line(&format!("{pad}  {rhs}"), width, out);
    true
}

/// `test ? then : else` with `?` / `:` on their own continuation lines.
fn wrap_ternary(line: &str, width: usize, out: &mut String) -> bool {
    let Some((first, then_line, else_line)) = split_ternary(line) else {
        return false;
    };
    wrap_line(&first, width, out);
    branch(&then_line, width, out);
    branch(&else_line, width, out);
    true
}

/// A branch of a split `?:`: a `?:` in it splits too, however short, as
/// oxfmt breaks a chain of conditionals whole.
fn branch(line: &str, width: usize, out: &mut String) {
    if !wrap_ternary(line, width, out) {
        wrap_line(line, width, out);
    }
}

/// Whether `s` is a `?:` at its top level.
pub(crate) fn is_ternary(s: &str) -> bool {
    find_ternary(s).is_some()
}

fn split_ternary(line: &str) -> Option<(String, String, String)> {
    let pad_len = line.len() - line.trim_start().len();
    let pad = &line[..pad_len];
    let mut rest = &line[pad_len..];
    let mut first_prefix = pad.to_string();
    if let Some(r) = rest.strip_prefix("return ") {
        first_prefix.push_str("return ");
        rest = r;
    } else if let Some(r) = rest.strip_prefix("? ") {
        first_prefix.push_str("? ");
        rest = r;
    } else if let Some(r) = rest.strip_prefix(": ") {
        first_prefix.push_str(": ");
        rest = r;
    } else if let Some(eq) = top_assign(rest) {
        first_prefix.push_str(&rest[..=eq + 2]);
        rest = rest[eq + 3..].trim_start();
    }
    let (test, then, else_) = find_ternary(rest)?;
    // A nested `?:` is parenthesized on one line, bare once split.
    let bare = |s: &str| -> String {
        let inner = strip_outer(s);
        if inner.len() < s.len() && find_ternary(inner).is_some() {
            inner.to_string()
        } else {
            s.to_string()
        }
    };
    let (then, else_) = (bare(then), bare(else_.trim_end_matches(';')) + if else_.ends_with(';') { ";" } else { "" });
    let inner = format!("{pad}  ");
    Some((format!("{first_prefix}{test}"), format!("{inner}? {then}"), format!("{inner}: {else_}")))
}

fn top_assign(s: &str) -> Option<usize> {
    let d = depths(s);
    let mut last = None;
    let mut i = 0;
    while i + 3 <= s.len() {
        if d.get(i) == Some(&Some(0)) && s[i..].starts_with(" = ") {
            last = Some(i);
        }
        i += 1;
    }
    last
}

fn find_ternary(s: &str) -> Option<(&str, &str, &str)> {
    let d = depths(s);
    let mut q = None;
    let mut i = 0;
    while i + 3 <= s.len() {
        if d.get(i) == Some(&Some(0)) && s[i..].starts_with(" ? ") {
            q = Some(i);
            break;
        }
        i += 1;
    }
    let q = q?;
    // The `:` that pairs with this `?`: a `?:` nested in the middle operand
    // opens and closes before it.
    let mut c = None;
    let mut open = 0usize;
    i = q + 3;
    while i + 3 <= s.len() {
        if d.get(i) == Some(&Some(0)) {
            if s[i..].starts_with(" ? ") {
                open += 1;
            } else if s[i..].starts_with(" : ") {
                if open == 0 {
                    c = Some(i);
                    break;
                }
                open -= 1;
            }
        }
        i += 1;
    }
    let c = c?;
    Some((s[..q].trim(), s[q + 3..c].trim(), s[c + 3..].trim()))
}

/// A long `(…)` group whose contents can wrap: open it so a nested `?:` or
/// `&&` is not stuck behind an outer pair.
fn wrap_outer_parens(line: &str, width: usize, out: &mut String) -> bool {
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
fn wrap_top_commas(line: &str, width: usize, out: &mut String) -> bool {
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

/// Where the body starts when `line` is an arrow whose head, up to the
/// first top-level `=>`, fits and whose body is a literal (`({`, `{`, `[`):
/// that bracket opens, so the parameters stay on the head's line
/// (`(a: A, b: B): T => ({` then the fields). `0` for any other line.
fn arrow_body(line: &str, width: usize) -> usize {
    arrow_split(line, width).filter(|&from| hugs(&line[from..])).unwrap_or(0)
}

fn arrow_split(line: &str, width: usize) -> Option<usize> {
    let d = depths(line);
    (0..line.len())
        .find(|&i| d[i] == Some(0) && line[i..].starts_with(" => "))
        .filter(|&at| at + 3 <= width)
        .map(|at| at + 4)
}

/// A body oxfmt keeps on the arrow's line, opening its bracket: an
/// object, block, or array that is the whole body, not the head of a longer
/// expression (`({ .. } satisfies T)[k]`).
fn hugs(body: &str) -> bool {
    if !(body.starts_with("({") || body.starts_with('{') || body.starts_with('[')) {
        return false;
    }
    let d = depths(body);
    let close = (1..body.len()).find(|&j| d[j] == Some(0) && matches!(body.as_bytes()[j], b')' | b'}' | b']'));
    // Unclosed on the line: a block, or a literal already opened.
    close.is_none_or(|c| body[c + 1..].trim_start_matches([')', ',', ';']).is_empty())
}

/// An arrow whose head fits and whose body is not a literal breaks after
/// `=>`, the body one indent in (`(a: A): T =>` then `f(a, b)`), as oxfmt
/// prints it, rather than opening the parameters or the body's call.
fn wrap_arrow_first(line: &str, width: usize, out: &mut String) -> bool {
    match arrow_split(line, width) {
        Some(from) if !hugs(&line[from..]) => wrap_arrow(line, width, out),
        _ => false,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_a_pair_around_everything() {
        assert_eq!(strip_outer("((a === b))"), "a === b");
        assert_eq!(strip_outer("(a) || (b)"), "(a) || (b)");
        assert_eq!(strip_outer("(\")\" + x)"), "\")\" + x");
        assert_eq!(strip_outer("f(x)"), "f(x)");
    }

    #[test]
    fn wraps_the_outermost_bracket_with_commas() {
        let line = "  return Result.ok({ kind: \"AwaitingConsent\", request, auth: f(a, b), note: \"x, y\" });";
        assert_eq!(
            wrap(line, 40),
            "  return Result.ok({\n    kind: \"AwaitingConsent\",\n    request,\n    auth: f(a, b),\n    note: \"x, y\",\n  });\n"
        );
        let sig = "export const f = (a: A, b: /* x, y */ B): R => {";
        assert_eq!(wrap(sig, 30), "export const f = (\n  a: A,\n  b: /* x, y */ B,\n): R => {\n");
        assert_eq!(wrap("short(a, b);", 40), "short(a, b);\n");
        let header = "for (let i = (0 as Usize), $e = (n as Usize); i < $e; i = (i + 1) as Usize) {";
        let wrapped = wrap(header, 40);
        assert!(wrapped.lines().all(|l| l.len() <= 40), "{wrapped}");
        assert!(wrapped.contains("for (\n"), "{wrapped}");
        let small = "if (!is_upper(Slice.at(b, i)) && !is_digit(Slice.at(b, i))) return Result.err(e);";
        assert_eq!(
            wrap(small, 60),
            "if (!is_upper(Slice.at(b, i)) && !is_digit(Slice.at(b, i)))\n  return Result.err(e);\n"
        );
        assert!(!wrap(small, 60).contains("Slice.at(\n"), "small calls stay closed");
        assert_eq!(
            wrap("f(r: Result<A, B>, n: Map<K, V<W>>, b: boolean): Result<A, B> => r;", 30),
            "f(\n  r: Result<A, B>,\n  n: Map<K, V<W>>,\n  b: boolean,\n): Result<A, B> => r;\n"
        );
        assert_eq!(wrap("// a, very, long, comment, line, here", 10), "// a, very, long, comment, line, here\n");
    }

    #[test]
    fn line_comment_is_not_code() {
        // An apostrophe or a lone `(` in a comment opens nothing.
        let d = depths("// don't (\nf(x)");
        assert_eq!(d[9], None);
        assert_eq!(d[10..], [Some(0), Some(0), Some(0), Some(1), Some(0)]);
    }

    #[test]
    fn wraps_if_return_and_logical_and_ternary() {
        let line = "          if (amountToCapture !== null && (amountToCapture < (1n as I64) || amountToCapture > capturable)) return Result.err({ kind: \"InvalidCaptureAmount\", capturable });";
        let out = wrap(line, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.contains("if ("), "{out}");
        assert!(out.contains("return Result.err("), "{out}");
        let ident = "export const isIdentChar = (c: U8): boolean => c >= (48 as U8) && c <= (57 as U8) || c >= (65 as U8) && c <= (90 as U8) || c === (45 as U8);";
        let out = wrap(ident, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.contains("=>\n"), "{out}");
        let tern = "  return reusable !== null ? Result.err({ kind: \"ConsentRequired\" }) : Result.ok({ kind: \"AwaitingPassword\", request, failures: (0 as U32), notice: { kind: \"Clear\" } });";
        let out = wrap(tern, 80);
        assert!(out.contains("?\n") || out.contains("? "), "{out}");
        assert!(out.lines().all(|l| l.len() <= 80), "{out}");
    }

    #[test]
    fn splits_a_ternary_before_the_and_in_its_else() {
        let line = "      const matches: boolean = pkce.method.kind === \"Plain\" ? verifier === pkce.challenge : verifierS256 !== null && verifierS256 === pkce.challenge;";
        assert_eq!(
            wrap(line, 100),
            "      const matches: boolean =\n        pkce.method.kind === \"Plain\"\n          ? verifier === pkce.challenge\n          : verifierS256 !== null && verifierS256 === pkce.challenge;\n"
        );
    }

    #[test]
    fn a_ternary_splits_before_its_branches_open() {
        let line = "          return !apart ? divide(Int.i64.mul(line.amount, 100n as I64), Int.i64.add(100n as I64, percent(rate)), conversion) : (0n as I64);";
        assert_eq!(
            wrap(line, 100),
            "          return !apart\n            ? divide(\n                Int.i64.mul(line.amount, 100n as I64),\n                Int.i64.add(100n as I64, percent(rate)),\n                conversion,\n              )\n            : (0n as I64);\n"
        );
        // A `?:` in the middle operand: its `:` is not the outer one.
        let nested = "  return reusable !== null ? request.prompt.no_interaction && needsConsent ? Result.err(e) : Result.ok(x) : Result.ok(y);";
        assert_eq!(
            wrap(nested, 80),
            "  return reusable !== null\n    ? request.prompt.no_interaction && needsConsent\n      ? Result.err(e)\n      : Result.ok(x)\n    : Result.ok(y);\n"
        );
    }

    #[test]
    fn breaks_a_condition_at_its_or_before_a_call() {
        let line = "        if (n === 0 || n > 63 || Slice.at(b, start) === 45 || Slice.at(b, Int.usize.sub(i, 1 as Usize)) === 45) return Result.err({ kind: \"BadDomain\" });";
        assert_eq!(
            wrap(line, 100),
            "        if (\n          n === 0 ||\n          n > 63 ||\n          Slice.at(b, start) === 45 ||\n          Slice.at(b, Int.usize.sub(i, 1 as Usize)) === 45\n        )\n          return Result.err({ kind: \"BadDomain\" });\n"
        );
    }

    #[test]
    fn an_arrow_keeps_its_parameters_and_opens_or_moves_its_body() {
        let ctor = "  RequiresCapture: (method: PaymentMethod, capturable: I64): Status => ({ kind: \"RequiresCapture\", method, capturable }),";
        assert_eq!(
            wrap(ctor, 100),
            "  RequiresCapture: (method: PaymentMethod, capturable: I64): Status => ({\n    kind: \"RequiresCapture\",\n    method,\n    capturable,\n  }),\n"
        );
        let call = "export const by = (n: U8, m: U8, c: Char, d: Char): Ordering => Ord.then(Ord.cmp(n, m), Ord.cmpStr(c, d));";
        assert_eq!(
            wrap(call, 100),
            "export const by = (n: U8, m: U8, c: Char, d: Char): Ordering =>\n  Ord.then(Ord.cmp(n, m), Ord.cmpStr(c, d));\n"
        );
        let generic = "  new: (code: string): Result<Sku, OrderError> => (code.length === 0) ? Result.err({ kind: \"EmptySku\" }) : Result.ok(x),";
        assert!(wrap(generic, 100).starts_with("  new: (code: string): Result<Sku, OrderError> =>\n"));
    }

    #[test]
    fn an_object_heading_a_longer_body_does_not_hug() {
        let line = "export const digitCount = (d: OtpDigits): Usize => ({ Six: 6, Seven: 7, Eight: 8 } satisfies Record<OtpDigits[\"kind\"], number>)[d.kind] as Usize;";
        assert!(wrap(line, 100).starts_with("export const digitCount = (d: OtpDigits): Usize =>\n  ({ Six"));
        let block =
            "export const validateRequestWithAVeryLongName = (params: AuthorizationParams, client: Client): R => {";
        assert!(!wrap(block, 100).contains("=>\n"), "{}", wrap(block, 100));
    }

    #[test]
    fn wraps_long_import_and_closed_ctor() {
        let imp =
            "import { type Char, type F64, type I32, type U64, type U8, type Usize } from \"./purecrate-runtime.ts\";";
        let out = wrap(imp, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
        assert!(out.contains("import {\n"), "{out}");
        let ctor = "export const AuthorizationRequest$of = (fields: Readonly<{ client_id: string; redirect_uri: string; scope: string; state: string; nonce: string | null; pkce: Pkce | null; prompt: Prompt; max_age: I64 | null; wants_mfa: boolean }>): AuthorizationRequest => fields as AuthorizationRequest;";
        let out = wrap(ctor, 100);
        assert!(out.lines().all(|l| l.len() <= 100), "{out}");
    }

    #[test]
    fn a_group_broken_inside_takes_no_trailing_comma() {
        let line = "export const f = (s: string, t: string, xs: ReadonlyArray<I32>, a: I32): I32 => (Str.slice(t, 3 as Usize, 3 as Usize).includes(\"a\") ? Iter.sum(Iter.map(xs, (x: I32): I32 => Int.i32.mul(x, -1 as I32)), Int.i32.add, 0 as I32) : 2 < 5 ? (s.length === 0 ? a : (1 as I32)) : Iter.sum(xs, Int.i32.add, 0 as I32));";
        let out = wrap(line, 100);
        assert!(!out.contains("I32),\n  );"), "{out}");
    }

    #[test]
    fn a_group_after_a_keyword_takes_no_trailing_comma() {
        // `(x,)` is a syntax error; with two items, the comma operator.
        assert!(is_call("f") && is_call("Int.i32.add") && is_call("g(a)") && is_call("xs[0]"));
        assert!(is_call("notif") && is_call("format"));
        for before in ["return", "  return", "a !==", "if", "while", "typeof", "throw", "=", ""] {
            assert!(!is_call(before), "{before}");
        }
        let line = "  return (aVeryLongLocalNameForTheValue !== 2147483647) !== (somethingElseEntirely && anotherThingToTest);";
        let out = wrap(line, 60);
        assert!(!out.contains(",\n"), "{out}");
    }

    #[test]
    fn a_cast_states_its_type() {
        assert_eq!(cast_type("512 as Usize"), Some("Usize"));
        assert_eq!(cast_type("Int.u8.and(x, 15 as U8) as number as Usize"), Some("Usize"));
        assert_eq!(cast_type("(b.length as Usize)"), Some("Usize"));
        assert_eq!(cast_type("a + b as I32"), None);
        assert_eq!(cast_type("c ? (1 as I32) : (2 as I32)"), None);
        assert_eq!(cast_type("f(1 as I32)"), None);
    }
}
