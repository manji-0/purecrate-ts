//! Breaking a long line at a top-level `&&` / `||` or a `?:`.

use super::*;

/// Split at the loosest of `||` / `&&` / `??` on the line.
pub(super) fn wrap_logical(line: &str, width: usize, out: &mut String) -> bool {
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
pub(super) fn wrap_condition(line: &str, width: usize, out: &mut String) -> bool {
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

pub(crate) fn split_at_op(expr: &str, op: &str, depth: usize) -> Option<Vec<String>> {
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
pub(super) fn wrap_assign_ternary(line: &str, width: usize, out: &mut String) -> bool {
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

/// `const x = a && b;` that does not fit: broken after `=`, the value one
/// indent in on a line of its own where it fits there, else one operand of
/// its top-level `||` (else `&&`) per line, as oxfmt lays out a logical
/// initializer.
pub(super) fn wrap_assign_logical(line: &str, width: usize, out: &mut String) -> bool {
    let pad = &line[..line.len() - line.trim_start().len()];
    let rest = line.trim_start();
    if !(rest.starts_with("const ") || rest.starts_with("let ")) {
        return false;
    }
    let Some(eq) = top_assign(rest) else { return false };
    let Some(value) = rest[eq + 3..].strip_suffix(';') else { return false };
    let d = depths(value);
    let arrow = (0..value.len()).any(|i| d[i] == Some(0) && value[i..].starts_with(" => "));
    if arrow || find_ternary(value).is_some() {
        return false;
    }
    let Some(parts) = split_at_op(value, " || ", 0).or_else(|| split_at_op(value, " && ", 0)) else {
        return false;
    };
    wrap_line(&format!("{pad}{} =", &rest[..eq]), width, out);
    let whole = format!("{pad}  {value};");
    if cols(&whole) <= width {
        emit_raw(&whole, out);
        return true;
    }
    let last = parts.len() - 1;
    for (i, part) in parts.into_iter().enumerate() {
        let end = if i == last { ";" } else { "" };
        wrap_line(&format!("{pad}  {}{end}", part.trim_end()), width, out);
    }
    true
}

/// `return a && b;` that does not fit: the value in parentheses, one
/// indent in on a line of its own where it fits there, else one operand of
/// its top-level `||` (else `&&`, else `??`) per line, as oxfmt lays out a
/// logical return value.
pub(super) fn wrap_return_logical(line: &str, width: usize, out: &mut String) -> bool {
    let pad = &line[..line.len() - line.trim_start().len()];
    let Some(value) = line.trim_start().strip_prefix("return ").and_then(|v| v.strip_suffix(';')) else {
        return false;
    };
    let d = depths(value);
    let arrow = (0..value.len()).any(|i| d[i] == Some(0) && value[i..].starts_with(" => "));
    if arrow || find_ternary(value).is_some() {
        return false;
    }
    let Some(parts) = [" || ", " && ", " ?? "].iter().find_map(|op| split_at_op(value, op, 0)) else {
        return false;
    };
    emit_raw(&format!("{pad}return ("), out);
    let whole = format!("{pad}  {value}");
    if cols(&whole) <= width {
        emit_raw(&whole, out);
    } else {
        for part in parts {
            wrap_line(&format!("{pad}  {}", part.trim_end()), width, out);
        }
    }
    emit_raw(&format!("{pad});"), out);
    true
}

/// An item of a bracket that `wrap_bracket`, `wrap_fat_group`, or
/// `wrap_sole_item` opened, `a && b,`: where it does not fit, one operand of
/// its top-level `||` (else `&&`) per line, the ones after the first one
/// indent in, as oxfmt lays out a logical argument or element.
pub(super) fn wrap_item(line: &str, width: usize, out: &mut String) {
    if cols(line) <= width || !wrap_logical_item(line, width, out) {
        wrap_line(line, width, out);
    }
}

fn wrap_logical_item(line: &str, width: usize, out: &mut String) -> bool {
    let pad = &line[..line.len() - line.trim_start().len()];
    let Some(item) = line.trim_start().strip_suffix(',') else { return false };
    let d = depths(item);
    let top = |s: &str| (0..item.len()).any(|i| d[i] == Some(0) && item[i..].starts_with(s));
    if top(": ") || top(" => ") || find_ternary(item).is_some() || top_assign(item).is_some() {
        return false;
    }
    let Some(parts) = split_at_op(item, " || ", 0).or_else(|| split_at_op(item, " && ", 0)) else {
        return false;
    };
    let last = parts.len() - 1;
    for (i, part) in parts.into_iter().enumerate() {
        let (more, end) = (if i == 0 { "" } else { "  " }, if i == last { "," } else { "" });
        wrap_line(&format!("{pad}{more}{}{end}", part.trim_end()), width, out);
    }
    true
}

/// A `?:` branch, `? a === b` or `: a && b`, that does not fit: broken at
/// its top-level `||`, else `&&`, else comparison, the operands after the
/// first one indent in, as oxfmt lays out a branch.
pub(super) fn wrap_branch_operator(line: &str, width: usize, out: &mut String) -> bool {
    let pad = &line[..line.len() - line.trim_start().len()];
    let rest = line.trim_start();
    let Some(prefix) = ["? ", ": "].into_iter().find(|p| rest.starts_with(p)) else { return false };
    let body = &rest[2..];
    let (body, end) = match body.strip_suffix(';') {
        Some(b) => (b, ";"),
        None => (body, ""),
    };
    if find_ternary(body).is_some() {
        return false;
    }
    let logical = split_at_op(body, " || ", 0).or_else(|| split_at_op(body, " && ", 0));
    // A comparison breaks only where its left side fits; else the left side
    // opens and the right stays after its `)`.
    let compared = || {
        let ops = [" === ", " !== ", " <= ", " >= ", " < ", " > "];
        ops.iter().find_map(|op| split_at_op(body, op, 0)).filter(|p| cols(&format!("{pad}{prefix}{}", p[0])) <= width)
    };
    let Some(parts) = logical.or_else(compared) else { return false };
    let last = parts.len() - 1;
    for (i, part) in parts.into_iter().enumerate() {
        let lead = if i == 0 { format!("{pad}{prefix}") } else { format!("{pad}  ") };
        let tail = if i == last { end } else { "" };
        wrap_line(&format!("{lead}{}{tail}", part.trim_end()), width, out);
    }
    true
}

/// A parenthesized group, `(a || f(x)) &&` or `(c ? a : b));`, that does not
/// fit: it stays open on its line, its `||` / `&&` operands or its `?:`
/// branches one indent in, the `)` after the last, as oxfmt hugs a group.
pub(super) fn wrap_paren_group(line: &str, width: usize, out: &mut String) -> bool {
    let indent = &line[..line.len() - line.trim_start().len()];
    // After a `?:` branch's `? ` / `: `, the group's lines sit past it.
    let (lead, rest, pad) = match ["? ", ": "].into_iter().find(|p| line.trim_start().starts_with(p)) {
        Some(p) => (format!("{indent}{p}"), &line.trim_start()[2..], format!("{indent}  ")),
        None => (indent.to_string(), line.trim_start(), indent.to_string()),
    };
    if !rest.starts_with('(') {
        return false;
    }
    let d = depths(rest);
    let Some(close) = (1..rest.len()).find(|&i| d[i] == Some(0) && rest.as_bytes()[i] == b')') else { return false };
    let (inner, after) = (&rest[1..close], &rest[close..]);
    // Only what closes the line follows: `)`, `,`, `;`, or a last operator.
    let trailing = after.trim_end_matches([')', ',', ';']);
    if !(trailing.is_empty() || trailing == ") &&" || trailing == ") ||" || [" &&", " ||"].contains(&trailing)) {
        return false;
    }
    let lines: Vec<String> = if let Some((test, then, else_)) = find_ternary(inner) {
        vec![test.to_string(), format!("  ? {then}"), format!("  : {else_}")]
    } else {
        let Some(parts) = split_at_op(inner, " || ", 0).or_else(|| split_at_op(inner, " && ", 0)) else {
            return false;
        };
        parts
            .iter()
            .enumerate()
            .map(|(i, p)| if i == 0 { p.trim_end().to_string() } else { format!("  {}", p.trim_end()) })
            .collect()
    };
    let last = lines.len() - 1;
    for (i, l) in lines.iter().enumerate() {
        let open = if i == 0 { format!("{lead}(") } else { pad.clone() };
        let shut = if i == last { after } else { "" };
        wrap_line(&format!("{open}{l}{shut}"), width, out);
    }
    true
}

/// `test ? then : else` with `?` / `:` on their own continuation lines.
pub(super) fn wrap_ternary(line: &str, width: usize, out: &mut String) -> bool {
    let Some((first, then_line, else_line)) = split_ternary(line) else {
        return false;
    };
    // A nested `?:`'s test (`: a &&` then `? b`) goes on four past its `:`
    // (`wrap_logical`), not as a branch's value does.
    let nested = first.trim_start().starts_with("? ") || first.trim_start().starts_with(": ");
    if !(nested && cols(&first) > width && wrap_logical(&first, width, out)) {
        wrap_line(&first, width, out);
    }
    branch(&then_line, width, out);
    branch(&else_line, width, out);
    true
}

/// A branch of a split `?:`: a `?:` in it splits too, however short, as
/// oxfmt breaks a chain of conditionals whole.
pub(super) fn branch(line: &str, width: usize, out: &mut String) {
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
