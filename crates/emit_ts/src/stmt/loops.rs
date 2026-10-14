//! Loops and jumps: `for`, `while`, labels, and whether a body breaks out.

use super::*;

/// Whether a `break` or `continue` of this loop (not of a loop inside it)
/// is in `expr`.
pub(crate) fn jumps_out(expr: &Expr) -> bool {
    jumps(expr, |e| matches!(e, Expr::Break(_) | Expr::Continue))
}

/// Whether a `break` in `expr` leaves the loop `expr` is the body of.
pub(crate) fn breaks(expr: &Expr) -> bool {
    jumps(expr, |e| matches!(e, Expr::Break(_)))
}

/// Whether a `break` in `expr` leaves the `loop` `expr` is the body of with
/// a value.
pub(crate) fn breaks_with_value(expr: &Expr) -> bool {
    jumps(expr, |e| matches!(e, Expr::Break(Some(_))))
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
        Expr::Break(_) | Expr::Continue => in_match,
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
    emit_loop_or(head, body, indent, None, out);
}

/// `emit_loop`, or `instead` where the body prints as nothing (a `?` the
/// flow decides never returns): oxfmt writes an empty body on two lines,
/// which lint refuses.
fn emit_loop_or(head: &str, body: &Expr, indent: usize, instead: Option<String>, out: &mut String) {
    let pad = "  ".repeat(indent);
    let label = jumps_out(body).then(|| if jumps_from_match(body, false) { temp("loop") } else { String::new() });
    LOOPS.with(|l| l.borrow_mut().push(label.clone()));
    // The body is a block of its own: nothing after it meets its names.
    crate::TAIL.with(|t| t.set(true));
    let mut inner = String::new();
    emit_stmts(body, indent + 1, Sink::Effect, &mut inner);
    LOOPS.with(|l| l.borrow_mut().pop());
    if let (true, Some(instead)) = (inner.is_empty(), instead) {
        out.push_str(&instead);
        return;
    }
    // A `for..of` variable the printed body no longer reads (what read it
    // was dropped as an unused value's effect-free part) is `_`, which TS
    // and oxlint exempt.
    let head = match head.strip_prefix("for (const ").and_then(|h| h.split_once(" of ")) {
        Some((var, rest)) if var != "_" && !reads_word(&inner, var) => format!("for (const _ of {rest}"),
        _ => head.to_string(),
    };
    // A jump names its loop only from inside a `switch`: a `match` that
    // printed as an `if` chain (or a `break` with a value that returns it)
    // needs no label. The jumps of a loop inside this one name that loop.
    let label = label.filter(|l| !l.is_empty()).filter(|l| {
        let named = ["break", "continue"].iter().any(|j| inner.contains(&format!("{j} {l};")));
        if inner.contains("switch (") || !named {
            return named;
        }
        for jump in ["break", "continue"] {
            inner = inner.replace(&format!("{jump} {l};"), &format!("{jump};"));
        }
        false
    });
    let prefix = label.map(|l| format!("{l}: ")).unwrap_or_default();
    out.push_str(&format!("{pad}{prefix}{head} {{\n"));
    out.push_str(&inner);
    out.push_str(&format!("{pad}}}\n"));
}

/// Whether `text` has `word` as an identifier, not inside a longer one.
fn reads_word(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(ident) && !text[at + word.len()..].chars().next().is_some_and(ident)
    })
}

/// Printed statements that never fall through: a `break;` after them would
/// be unreachable, which a consumer's `allowUnreachableCode: false` rejects.
pub(crate) fn ends_in_jump(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) | Expr::Break(_) | Expr::Continue => true,
        Expr::Seq { then, .. } | Expr::Let { then, .. } => ends_in_jump(then),
        // `v = w?` where `w` is known to fail: the value's statements leave.
        Expr::Assign { value, .. } => value.needs_statements() && ends_in_jump(value),
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
            if **then == Expr::Lit(Lit::Unit)
                && matches!(**first, Expr::Break(_) | Expr::Continue | Expr::Return(_)) =>
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
/// back to the bounds' type, which `check::accept` records. `a..=b` tests
/// `i <= $e`; its last step, past `b`, is a JS number or `bigint` the body
/// never sees, so it cannot wrap. `(a..b).rev()` starts at `b - 1` and steps
/// down while `i >= a` (`(a..=b).rev()` at `b`), likewise never seen past `a`.
pub(crate) fn emit_for(
    var: &str,
    ty: IntTy,
    (start, end): (&Expr, &Expr),
    (inclusive, rev): (bool, bool),
    body: &Expr,
    indent: usize,
    out: &mut String,
) {
    let one = if ty.is_big() { "1n" } else { "1" };
    let t = ty.ts_name();
    // A literal bound is the same value however often it is read.
    let lit = |e: &Expr| match peel(e) {
        Expr::Lit(lit) => Some(bare_lit(lit)),
        _ => None,
    };
    let head = if rev {
        // Rust evaluates `start` first: a bound read on every test is a
        // temporary declared before `i`.
        let first = match peel(end) {
            Expr::Lit(Lit::Int { value, .. }) if !inclusive => {
                let n = if ty.is_big() { format!("{}n", value - 1) } else { (value - 1).to_string() };
                format!("{n} as {t}")
            }
            _ if inclusive => emit_item(end, indent),
            _ => format!("({} - {one}) as {t}", emit_item(end, indent)),
        };
        match lit(start) {
            Some(s) => format!("for (let {var} = {first}; {var} >= {s}; {var} = ({var} - {one}) as {t})"),
            None => {
                let bound = temp("start");
                format!(
                    "for (let {bound} = {}, {var} = {first}; {var} >= {bound}; {var} = ({var} - {one}) as {t})",
                    emit_item(start, indent)
                )
            }
        }
    } else {
        let test = if inclusive { "<=" } else { "<" };
        match lit(end) {
            Some(e) => format!(
                "for (let {var} = {}; {var} {test} {e}; {var} = ({var} + {one}) as {t})",
                emit_item(start, indent)
            ),
            None => {
                let bound = temp("end");
                format!(
                    "for (let {var} = {}, {bound} = {}; {var} {test} {bound}; {var} = ({var} + {one}) as {t})",
                    emit_item(start, indent),
                    emit_item(end, indent),
                )
            }
        }
    };
    // A bound already named (bound before, for what precedes it) is read
    // only by the loop, which stays.
    let named = |e: &Expr| matches!(peel(e), Expr::Var(_));
    let bounds = (!named(start) && !named(end)).then(|| {
        let mut bounds = String::new();
        Sink::Effect.finish_expr(start, indent, &mut bounds);
        Sink::Effect.finish_expr(end, indent, &mut bounds);
        bounds
    });
    emit_loop_or(&head, body, indent, bounds, out);
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
