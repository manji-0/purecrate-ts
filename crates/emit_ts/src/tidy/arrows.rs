//! Breaking a long line at an arrow function: its body, the last arrow
//! argument, or a composition of calls.

use super::*;

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
pub(super) fn wrap_composition(line: &str, width: usize, out: &mut String) -> bool {
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

pub(super) fn wrap_last_arrow(line: &str, width: usize, out: &mut String) -> bool {
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
    // `=> (a ? b : c)`: oxfmt drops the parentheses once the body breaks.
    let body = match strip_outer(body) {
        inner if inner.len() < body.len() && find_ternary(inner).is_some() => inner,
        _ => body,
    };
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
    // A `?:` body breaks on its own lines under the hugged head, as oxfmt
    // prints it; anything else must fit there.
    let ternary = find_ternary(body).is_some();
    if cols(&head) > width || (cols(&below) > width && !ternary) {
        return false;
    }
    emit_raw(&head, out);
    wrap_line(&below, width, out);
    emit_raw(&format!("{pad}){end}"), out);
    true
}

/// Break after `=>` when the head fits, so a long body can wrap on its own.
pub(super) fn wrap_arrow(line: &str, width: usize, out: &mut String) -> bool {
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

/// Where the body starts when `line` is an arrow whose head, up to the
/// first top-level `=>`, fits and whose body is a literal (`({`, `{`, `[`):
/// that bracket opens, so the parameters stay on the head's line
/// (`(a: A, b: B): T => ({` then the fields). `0` for any other line.
pub(super) fn arrow_body(line: &str, width: usize) -> usize {
    arrow_split(line, width).filter(|&from| hugs(&line[from..])).unwrap_or(0)
}

pub(super) fn arrow_split(line: &str, width: usize) -> Option<usize> {
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
pub(super) fn wrap_arrow_first(line: &str, width: usize, out: &mut String) -> bool {
    match arrow_split(line, width) {
        Some(from) if !hugs(&line[from..]) => wrap_arrow(line, width, out),
        _ => false,
    }
}
