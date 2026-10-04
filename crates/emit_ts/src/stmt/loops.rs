//! Loops and jumps: `for`, `while`, labels, and whether a body breaks out.

use super::*;

/// Whether a `break` or `continue` of this loop (not of a loop inside it)
/// is in `expr`.
pub(crate) fn jumps_out(expr: &Expr) -> bool {
    jumps(expr, |e| matches!(e, Expr::Break | Expr::Continue))
}

/// Whether a `break` in `expr` leaves the loop `expr` is the body of.
pub(crate) fn breaks(expr: &Expr) -> bool {
    jumps(expr, |e| matches!(e, Expr::Break))
}

/// Whether `expr` holds a jump `jump` picks out, of this loop (not of a
/// loop inside it, nor of a closure).
pub(super) fn jumps(expr: &Expr, jump: fn(&Expr) -> bool) -> bool {
    expr.search(|e| match e {
        Expr::For { .. } | Expr::ForEach { .. } | Expr::While { .. } | Expr::Closure { .. } => Some(false),
        e => jump(e).then_some(true),
    })
}

/// Whether a jump of this loop is inside a `match`, which may print as a
/// `switch`.
fn jumps_from_match(expr: &Expr, in_match: bool) -> bool {
    match expr {
        Expr::Break | Expr::Continue => in_match,
        Expr::For { .. } | Expr::ForEach { .. } | Expr::While { .. } | Expr::Closure { .. } => false,
        Expr::Match { scrutinee, arms } => {
            jumps_from_match(scrutinee, in_match) || arms.iter().any(|a| jumps_from_match(&a.body, true))
        }
        other => other.children().into_iter().any(|c| jumps_from_match(c, in_match)),
    }
}

/// Prints a loop's head (after its `label: `, if it needs one) and body.
/// A jump inside a `match` names its loop: a bare JS `break` inside the
/// `switch` a `match` prints as would leave the `switch`, not the loop.
/// Elsewhere it is bare.
pub(crate) fn emit_loop(head: &str, body: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let label =
        jumps_out(body).then(|| if jumps_from_match(body, false) { temp("loop", indent) } else { String::new() });
    let prefix = label.as_ref().filter(|l| !l.is_empty()).map(|l| format!("{l}: ")).unwrap_or_default();
    out.push_str(&format!("{pad}{prefix}{head} {{\n"));
    LOOPS.with(|l| l.borrow_mut().push(label));
    emit_stmts(body, indent + 1, Sink::Effect, out);
    LOOPS.with(|l| l.borrow_mut().pop());
    out.push_str(&format!("{pad}}}\n"));
}

/// Printed statements that never fall through: a `break;` after them would
/// be unreachable, which a consumer's `allowUnreachableCode: false` rejects.
pub(crate) fn ends_in_jump(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) | Expr::Break | Expr::Continue => true,
        Expr::Seq { then, .. } | Expr::Let { then, .. } => ends_in_jump(then),
        Expr::If { then, else_, .. } => expr.needs_statements() && ends_in_jump(then) && ends_in_jump(else_),
        // Printed as a `switch` whose `default` returns, or an `if` chain
        // ending in `else`: it jumps when every arm does.
        Expr::Match { arms, .. } => !arms.is_empty() && arms.iter().all(|a| ends_in_jump(&a.body)),
        // `for (;;)` that no `break` leaves.
        Expr::While { cond, body } => **cond == Expr::Lit(Lit::Bool(true)) && !breaks(body),
        _ => false,
    }
}

/// A jump that is all of a block (`{ break; }` is the jump then `()`).
pub(super) fn lone_jump(expr: &Expr) -> &Expr {
    match expr {
        Expr::Seq { first, then }
            if **then == Expr::Lit(Lit::Unit) && matches!(**first, Expr::Break | Expr::Continue | Expr::Return(_)) =>
        {
            first
        }
        other => other,
    }
}

/// ` label` of the innermost loop, or nothing when its jumps are bare.
pub(super) fn jump_label() -> String {
    let l = innermost_loop();
    if l.is_empty() {
        l
    } else {
        format!(" {l}")
    }
}

pub(crate) fn innermost_loop() -> String {
    LOOPS
        .with(|l| l.borrow().last().cloned().flatten())
        .expect("`check::accept` puts `break` and `continue` only inside a loop that is labelled for them")
}

/// `for (let i = start, $e = end; i < $e; i = i + 1)`: the bounds are
/// evaluated once, in order, as Rust evaluates the range. `i + 1` cannot
/// overflow below `end`. A brand does not survive `+`, so the step casts
/// back to the bounds' type, which `check::accept` records.
pub(crate) fn emit_for(var: &str, ty: IntTy, start: &Expr, end: &Expr, body: &Expr, indent: usize, out: &mut String) {
    let one = if ty.is_big() { "1n" } else { "1" };
    // A literal end is the same value however often it is read.
    let head = if matches!(peel(end), Expr::Lit(_)) {
        format!(
            "for (let {var} = {}; {var} < {}; {var} = ({var} + {one}) as {})",
            emit_item(start, indent),
            match peel(end) {
                Expr::Lit(lit) => bare_lit(lit),
                _ => unreachable!("matched above"),
            },
            ty.ts_name()
        )
    } else {
        let bound = temp("end", indent);
        format!(
            "for (let {var} = {}, {bound} = {}; {var} < {bound}; {var} = ({var} + {one}) as {})",
            emit_item(start, indent),
            emit_item(end, indent),
            ty.ts_name()
        )
    };
    emit_loop(&head, body, indent, out);
}

/// What `for` and the consuming methods walk, from the source's text.
pub(crate) fn iterable(over: purecrate_ir::Over, source: crate::tx::Tx) -> String {
    match over {
        // A JS string iterates by code point, as `chars` does by scalar
        // value; the two agree on well-formed strings (design/01 §6).
        // Bare as a `for..of` source or an argument; a binary source keeps
        // its parentheses under `as`.
        purecrate_ir::Over::Chars if source.prec() < crate::tidy::PREC_UNARY => {
            format!("({}) as Iterable<Char>", source.print())
        }
        purecrate_ir::Over::Chars => format!("{} as Iterable<Char>", crate::tidy::strip_outer(&source.print())),
        // The UTF-8 bytes, as `as_bytes` reads them.
        purecrate_ir::Over::Bytes => format!("Str.bytes({})", source.print()),
        purecrate_ir::Over::Items => source.print(),
    }
}
