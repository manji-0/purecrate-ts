//! Breaking a long statement at its head: after `=`, an `if (..) return`,
//! an `if (..) {`, a `for (..)`.

use super::*;

/// `const x = a.b(c);` whose value is a call oxfmt finds poorly breakable,
/// a name or a chain of fields called with nothing or one short operand (a
/// name, a field chain, or a literal of at most a quarter of the width):
/// broken after `=`, the value one indent in, where it then fits.
pub(super) fn wrap_after_assign(line: &str, width: usize, out: &mut String) -> bool {
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
pub(super) fn wrap_if_return(line: &str, width: usize, out: &mut String) -> bool {
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
pub(super) fn wrap_if_open(line: &str, width: usize, out: &mut String) -> bool {
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
pub(super) fn wrap_for(line: &str, width: usize, out: &mut String) -> bool {
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
