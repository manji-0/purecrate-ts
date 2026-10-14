//! Reading a printed expression at its top level: its `&&` / `||`
//! operands.

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
