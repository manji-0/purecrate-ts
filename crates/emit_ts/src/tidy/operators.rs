//! Reading a printed expression at its top level: its `&&` / `||`
//! operands, and a `?:`.

use super::*;

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

/// Whether `s` is a `?:` at its top level.
pub(crate) fn is_ternary(s: &str) -> bool {
    find_ternary(s).is_some()
}

pub(super) fn find_ternary(s: &str) -> Option<(&str, &str, &str)> {
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
