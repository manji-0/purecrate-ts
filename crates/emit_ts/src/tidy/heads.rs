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

/// `if (cond) return value;` (or `continue;`, `break;`) when it does not
/// fit: the condition on its own `if` line when that fits, else opened,
/// then the statement one indent in.
pub(super) fn wrap_if_return(line: &str, width: usize, out: &mut String) -> bool {
    let jump = paren_head(line, "if").and_then(|(pad, cond, after)| match after {
        " continue;" | " break;" => Some((pad, cond, after.trim_start())),
        _ => None,
    });
    if let Some((pad, cond, stmt)) = jump {
        let header = format!("{pad}if ({cond})");
        if cols(&header) <= width {
            wrap_line(&header, width, out);
        } else {
            wrap_line(&format!("{pad}if ("), width, out);
            opened_test(pad, cond, width, out);
            wrap_line(&format!("{pad})"), width, out);
        }
        wrap_line(&format!("{pad}  {stmt}"), width, out);
        return true;
    }
    let Some((pad, cond, val)) = parse_if_return(line) else {
        return false;
    };
    let header = format!("{pad}if ({cond})");
    if cols(&header) <= width {
        wrap_line(&header, width, out);
    } else {
        wrap_line(&format!("{pad}if ("), width, out);
        opened_test(pad, cond, width, out);
        wrap_line(&format!("{pad})"), width, out);
    }
    wrap_line(&format!("{pad}  return {val};"), width, out);
    true
}

/// A test between an opened `(` and its `)`: its top-level `||` (else
/// `&&`) breaks with the parentheses, one operand a line, though the test
/// would fit on one, as oxfmt prints it.
/// An operand that does not fit breaks after its comparison, the right
/// side one indent further.
fn opened_test(pad: &str, cond: &str, width: usize, out: &mut String) {
    let parts = split_at_op(cond, " || ", 0).or_else(|| split_at_op(cond, " && ", 0));
    for part in parts.unwrap_or_else(|| vec![cond.to_string()]) {
        let line = format!("{pad}  {}", part.trim_end());
        let (body, tail) = match ["||", "&&"].iter().find_map(|op| part.trim_end().strip_suffix(op)) {
            Some(b) => (b.trim_end(), &part.trim_end()[b.trim_end().len()..]),
            None => (part.trim_end(), ""),
        };
        let ops = [" === ", " !== ", " <= ", " >= ", " < ", " > "];
        let compared = ops.iter().find_map(|op| split_at_op(body, op, 0).filter(|p| p.len() == 2));
        match compared {
            Some(sides) if cols(&line) > width => {
                wrap_line(&format!("{pad}  {}", sides[0].trim_end()), width, out);
                wrap_line(&format!("{pad}    {}{tail}", sides[1].trim_end()), width, out);
            }
            _ => wrap_line(&line, width, out),
        }
    }
}

fn parse_if_return(line: &str) -> Option<(&str, &str, &str)> {
    let (pad, cond, after) = paren_head(line, "if")?;
    let val = after.strip_prefix(" return ")?.strip_suffix(';')?;
    if val.is_empty() || val.contains('\n') {
        return None;
    }
    Some((pad, cond, val))
}

/// `line` as `<pad><keyword> (<inside>)<after>`, the parenthesis closed at
/// its own depth.
fn paren_head<'a>(line: &'a str, keyword: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let pad_len = line.len() - line.trim_start().len();
    let rest = line[pad_len..].strip_prefix(keyword)?.strip_prefix(' ')?;
    if !rest.starts_with('(') {
        return None;
    }
    let d = depths(rest);
    let close = (1..rest.len()).find(|&j| d[j] == Some(0) && rest.as_bytes()[j] == b')')?;
    Some((&line[..pad_len], &rest[1..close], &rest[close + 1..]))
}

/// `if (cond) {` (or `} else if (cond) {`, or `while (cond) {`) whose
/// condition does not fit on the line: the condition on lines of its own,
/// as oxfmt lays it out.
pub(super) fn wrap_if_open(line: &str, width: usize, out: &mut String) -> bool {
    let trimmed = line.trim_start();
    let pad = &line[..line.len() - trimmed.len()];
    let (keyword, head, rest) = match trimmed.strip_prefix("} else ") {
        Some(rest) => ("if", "} else if (", format!("{pad}{rest}")),
        None if trimmed.starts_with("while ") => ("while", "while (", line.to_string()),
        None => ("if", "if (", line.to_string()),
    };
    let Some((_, cond, " {")) = paren_head(&rest, keyword) else {
        return false;
    };
    wrap_line(&format!("{pad}{head}"), width, out);
    opened_test(pad, cond, width, out);
    wrap_line(&format!("{pad}) {{"), width, out);
    true
}

/// `for (init; test; step) {` with the three clauses on their own lines.
pub(super) fn wrap_for(line: &str, width: usize, out: &mut String) -> bool {
    let Some((pad, header, " {")) = paren_head(line, "for") else {
        return false;
    };
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
    wrap_line(&format!("{pad}for ("), width, out);
    // Declarators that do not fit on a line break after each comma, the
    // later ones one indent further (`let i = ..,` then `end = ..;`).
    let init = format!("{pad}  {};", parts[0]);
    let d0 = depths(&parts[0]);
    let commas: Vec<usize> =
        (0..parts[0].len()).filter(|&j| d0[j] == Some(0) && parts[0][j..].starts_with(", ")).collect();
    if cols(&init) > width && !commas.is_empty() {
        let mut start = 0;
        for (n, &c) in commas.iter().enumerate() {
            let more = if n == 0 { "" } else { "  " };
            wrap_line(&format!("{pad}  {more}{},", parts[0][start..c].trim()), width, out);
            start = c + 2;
        }
        wrap_line(&format!("{pad}    {};", parts[0][start..].trim()), width, out);
    } else {
        wrap_line(&init, width, out);
    }
    wrap_line(&format!("{pad}  {};", parts[1]), width, out);
    wrap_line(&format!("{pad}  {}", parts[2]), width, out);
    wrap_line(&format!("{pad}) {{"), width, out);
    true
}
