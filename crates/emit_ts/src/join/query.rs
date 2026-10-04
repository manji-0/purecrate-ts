//! Questions about an expression folding asks: whether it is pure, has
//! no effect, ends, declares a name, or reads one.

use super::*;

/// Whether running `expr` always leaves the block: a `return`, `break`, or
/// `continue`, last or on every side.
pub(super) fn ends(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) | Expr::Break | Expr::Continue => true,
        Expr::Seq { first, then } => ends(first) || ends(then),
        Expr::Let { value, then, .. } => ends(value) || ends(then),
        Expr::Assign { value, .. } => ends(value),
        Expr::If { cond, then, else_ } => match constant(cond) {
            Some((_, b)) => ends(if b { then } else { else_ }),
            None => ends(then) && ends(else_),
        },
        Expr::Match { arms, .. } => !arms.is_empty() && arms.iter().all(|a| ends(&a.body)),
        Expr::At { expr, .. } => ends(expr),
        // `while true` nothing breaks out of (what a `?` in a loop's test
        // leaves when the test is decided): only a `return` leaves it.
        Expr::While { cond, body } => matches!(**cond, Expr::Lit(Lit::Bool(true))) && !breaks(body),
        _ => false,
    }
}

pub(crate) fn mentions(expr: &Expr, name: &Name) -> bool {
    expr.any(|e| matches!(e, Expr::Var(n) | Expr::Call { callee: purecrate_ir::Callee::Local(n), .. } if n == name))
}

/// Whether evaluating `expr` can neither panic nor change anything: it reads
/// names and fields, compares, and builds values. Integer arithmetic is a
/// call by now (it may overflow), as is everything else that may panic.
pub(super) fn pure(expr: &Expr) -> bool {
    let here = matches!(
        expr,
        Expr::Lit(_)
            | Expr::Var(_)
            | Expr::Field { .. }
            | Expr::Construct { .. }
            | Expr::Tuple(_)
            | Expr::Array(_)
            | Expr::Ignored { .. }
            | Expr::Unary { .. }
            | Expr::Binary { .. }
    );
    here && expr.children().into_iter().all(pure)
}

/// Whether evaluating `expr` can neither panic nor change anything, as
/// `pure`, through `Option` and `Result` built, tested, and matched.
pub(super) fn effectless(expr: &Expr) -> bool {
    use purecrate_ir::Callee;
    match expr {
        Expr::Call {
            callee:
                Callee::OptionSome
                | Callee::OptionNone
                | Callee::ResultOk
                | Callee::ResultErr
                | Callee::OptionIsSome
                | Callee::OptionIsNone,
            args,
        } => args.iter().all(effectless),
        // Reads that cannot panic: a string's `len`, `as_str`, tests, and
        // pieces; a `Vec`'s `len` and `is_empty`; `String::from`. (Not
        // `char` methods: `to_digit` panics past radix 36.)
        Expr::Call { callee: Callee::Str(_) | Callee::VecLen | Callee::VecIsEmpty | Callee::StringFrom, args } => {
            args.iter().all(effectless)
        }
        Expr::Let { value, then, .. } => effectless(value) && effectless(then),
        Expr::Match { scrutinee, arms } => {
            effectless(scrutinee) && arms.iter().all(|a| a.guard.as_ref().is_none_or(effectless) && effectless(&a.body))
        }
        Expr::If { cond, then, else_ } => effectless(cond) && effectless(then) && effectless(else_),
        // Comparisons, `!`, `&&`, `||`: integer arithmetic is a call by now.
        Expr::Binary { left, right, .. } => effectless(left) && effectless(right),
        Expr::Unary { expr, .. } => effectless(expr),
        e => pure(e),
    }
}

/// `name` itself, or a copy of it (`String::from(&t)`, printed as `t`).
pub(super) fn reads_only(value: &Expr, name: &Name) -> bool {
    match value {
        Expr::Var(v) => v == name,
        Expr::Call { callee: purecrate_ir::Callee::StringFrom, args } => {
            matches!(args.as_slice(), [a] if reads_only(a, name))
        }
        _ => false,
    }
}

/// Whether printing `expr` as statements declares a name in its block.
pub(crate) fn declares(expr: &Expr) -> bool {
    match expr {
        Expr::Let { .. } | Expr::Try { .. } => true,
        Expr::Seq { first, then } => declares(first) || declares(then),
        Expr::Match { scrutinee, .. } => !is_place(scrutinee),
        _ => false,
    }
}

pub(super) fn root(place: &Expr) -> &Name {
    match place {
        Expr::Field { base, .. } => root(base),
        Expr::Var(n) => n,
        _ => unreachable!("a place starts from a variable"),
    }
}
