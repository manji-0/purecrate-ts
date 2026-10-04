//! `match` as an `if` chain: each branch's test, bindings, and body.

use super::*;
use crate::tx::Tx;

/// One branch of an `if` chain: its test (`None` for the last `else`), the
/// bindings its pattern reads, and its code.
pub(crate) struct Branch<'e> {
    pub test: Option<Tx>,
    pub prelude: String,
    pub body: &'e Expr,
}

/// `if (a) { .. } else if (b) { .. } else { .. }`. Where every branch
/// before the last jumps (each returns, where the value is returned), a
/// branch follows the `if` before it instead of an `else`, and a branch that
/// is one `return` sits on the `if`'s line: `if (s === "") return
/// Result.err(..);`. A returned value is the last statement of its block,
/// and `rename` already kept each branch's bindings apart from the block's;
/// elsewhere statements follow, so the last branch moves out of its `else`
/// only when it declares nothing they could meet.
pub(crate) fn emit_branches(branches: &[Branch], indent: usize, sink: Sink, tail: bool, out: &mut String) {
    // `else if (a) {} else { x }` (an arm that does nothing, `"" => {}`)
    // is `else if (!a) { x }`, where nothing is handed on: where the value
    // is returned, the empty arm still returns, and a `case` would
    // otherwise run into the next one.
    if let ([.., empty, last], Sink::Effect) = (branches, sink) {
        if let (Some(test), None, Expr::Lit(Lit::Unit), true) =
            (&empty.test, &last.test, empty.body, empty.prelude.is_empty() && last.prelude.is_empty())
        {
            let mut merged: Vec<Branch> = branches[..branches.len() - 2]
                .iter()
                .map(|b| Branch { test: b.test.clone(), prelude: b.prelude.clone(), body: b.body })
                .collect();
            merged.push(Branch { test: Some(test.clone().not_all()), prelude: String::new(), body: last.body });
            return emit_branches(&merged, indent, sink, tail, out);
        }
    }
    let pad = "  ".repeat(indent);
    let last = match branches.split_last() {
        Some((last, _)) if last.test.is_some() => None,
        Some((last, _)) if matches!(sink, Sink::Return) => Some(last),
        Some((last, rest)) if last.prelude.is_empty() && rest.iter().all(|b| ends_in_jump(b.body)) => Some(last),
        _ => None,
    };
    let flat_last = last.and_then(|last| {
        let mut body = String::new();
        emit_tail(last.body, indent, sink, &mut body, tail);
        let declares = declares(&body, &pad);
        let prelude: String =
            last.prelude.lines().map(|l| format!("{}\n", l.strip_prefix("  ").unwrap_or(l))).collect();
        (matches!(sink, Sink::Return) || tail || !declares).then(|| format!("{prelude}{body}"))
    });
    let Some(flat_last) = flat_last else {
        for (i, b) in branches.iter().enumerate() {
            // An `else` that is one `if` chain is `else if`, as oxlint's
            // `no-lonely-if` asks.
            if let (true, None, true) = (i > 0, &b.test, b.prelude.is_empty()) {
                let mut inner = String::new();
                emit_tail(b.body, indent + 1, sink, &mut inner, true);
                if let Some(chain) = lone_if(&inner, &pad) {
                    out.push_str(&format!("{pad}}} else {chain}"));
                    return;
                }
                out.push_str(&format!("{pad}}} else {{\n"));
                out.push_str(&inner);
                continue;
            }
            let head = match (i, &b.test) {
                (0, Some(test)) => format!("{pad}if {} {{\n", crate::expr::if_test(&test.print(), &pad)),
                (_, Some(test)) => format!("{pad}}} else if {} {{\n", crate::expr::if_test(&test.print(), &pad)),
                (_, None) => format!("{pad}}} else {{\n"),
            };
            out.push_str(&head);
            out.push_str(&b.prelude);
            emit_tail(b.body, indent + 1, sink, out, true);
        }
        out.push_str(&format!("{pad}}}\n"));
        return;
    };
    for b in &branches[..branches.len() - 1] {
        let test = b.test.as_ref().expect("only the last branch has no test");
        emit_guard(&test.print(), &b.prelude, b.body, indent, sink, out);
    }
    // The last branch: its prelude was printed one level in.
    out.push_str(&flat_last);
}

/// `text`, one level in from `pad`, as the `if` chain it is alone, one
/// level out and without its indent on the first line: `if (a) {` .. `}`.
/// `None` when it holds anything else, a blank line included.
fn lone_if(text: &str, pad: &str) -> Option<String> {
    let inner = format!("{pad}  ");
    let lines: Vec<&str> = text.lines().collect();
    let (first, last) = (lines.first()?, lines.last()?);
    if !first.starts_with(&format!("{inner}if ")) || *last != format!("{inner}}}") || !first.ends_with('{') {
        return None;
    }
    for line in &lines {
        let rest = line.strip_prefix(&inner)?;
        // A line of the chain itself, not of a block inside it.
        if !rest.starts_with(' ') && !(rest.starts_with("if ") || rest.starts_with("} else") || rest == "}") {
            return None;
        }
    }
    if lines.iter().filter(|l| l.strip_prefix(&inner).is_some_and(|r| r.starts_with("if "))).count() != 1 {
        return None;
    }
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let line = &line[2..];
        out.push_str(if i == 0 { line.trim_start() } else { line });
        out.push('\n');
    }
    Some(out)
}

/// `if (test) { prelude body }`, on one line when the body is one
/// `return` or jump and there is no prelude.
pub(super) fn emit_guard(test: &str, prelude: &str, body: &Expr, indent: usize, sink: Sink, out: &mut String) {
    let pad = "  ".repeat(indent);
    let mut inner = String::new();
    emit_tail(body, indent + 1, sink, &mut inner, true);
    let one = inner.trim_start();
    let jump = ["return ", "break ", "continue "].iter().any(|k| one.starts_with(k));
    if prelude.is_empty() && jump && inner.lines().count() == 1 {
        out.push_str(&format!("{pad}if {} {one}", crate::expr::if_test(test, &pad)));
    } else {
        out.push_str(&format!("{pad}if {} {{\n{prelude}{inner}{pad}}}\n", crate::expr::if_test(test, &pad)));
    }
}

/// A `match` on an integer or a `&str`: arms tried in order, the last (`_`)
/// as `else`. Well-formed strings are equal in UTF-8 exactly when they are in
/// UTF-16, so `===` is `str` equality.
pub(crate) fn emit_lit_chain(
    subject: &str,
    arms: &[purecrate_ir::Arm],
    indent: usize,
    sink: Sink,
    tail: bool,
    out: &mut String,
) {
    let branches: Vec<Branch> = arms
        .iter()
        .map(|arm| Branch { test: lit_tx(&arm.pattern, subject), prelude: String::new(), body: &arm.body })
        .collect();
    emit_branches(&branches, indent, sink, tail, out);
}
