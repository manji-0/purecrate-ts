//! Questions about an expression the statement printer and `join` ask:
//! whether it is a place, what it binds or reads, its scrutinee's type.

use super::*;

/// `x`, `x.a.b`: references TS can narrow through `switch (x.kind)`.
pub(crate) fn is_place(expr: &Expr) -> bool {
    match expr {
        Expr::Var(_) => true,
        Expr::Field { base, .. } => is_place(base),
        _ => false,
    }
}

/// `place` is `of` or a field reached through it (`s.method` within `s`),
/// read through `clone()`.
pub(crate) fn within(place: &Expr, of: &Expr) -> bool {
    let place = peel_identity(place);
    place == of || matches!(place, Expr::Field { base, .. } if within(base, of))
}

pub(crate) fn peel(expr: &Expr) -> &Expr {
    match expr {
        Expr::Ignored { expr, .. } => peel(expr),
        other => other,
    }
}

/// `let $x = e; $x` when `e` is already an expression: the binding names a
/// type (`collect::<T>()`, `sum::<T>()`) and is not a second evaluation.
pub(crate) fn peel_identity(expr: &Expr) -> &Expr {
    identity_let(expr).map_or(expr, |(_, value)| peel_identity(value))
}

/// The annotation of the outermost `let x: T = e; x` that `peel_identity`
/// takes off `expr`.
pub(super) fn peeled_ty(expr: &Expr) -> Option<&Ty> {
    identity_let(expr).and_then(|(ty, value)| ty.or_else(|| peeled_ty(value)))
}

/// `let x: T = e; x` as its annotation and `e`.
fn identity_let(expr: &Expr) -> Option<(Option<&Ty>, &Expr)> {
    match expr {
        Expr::Let { name, mutable: false, ty, value, then }
            if matches!(then.as_ref(), Expr::Var(n) if n == name) && !value.needs_statements() =>
        {
            Some((ty.as_ref(), value))
        }
        _ => None,
    }
}

/// Without an annotation a variant literal widens to `{ kind: string }`.
pub(crate) fn scrutinee_ty(arms: &[purecrate_ir::Arm]) -> Option<&purecrate_ir::Name> {
    arms.iter().find_map(|arm| match &arm.pattern {
        Pattern::Variant { ty, .. } => Some(ty),
        Pattern::Or(alts) => alts.iter().find_map(|alt| match alt {
            Pattern::Variant { ty, .. } => Some(ty),
            _ => None,
        }),
        _ => None,
    })
}

pub(crate) fn declares_at_top(expr: &Expr) -> bool {
    match expr {
        Expr::Let { .. } | Expr::Try { .. } | Expr::Seq { .. } | Expr::Assign { .. } => true,
        Expr::Match { scrutinee, .. } => !is_place(scrutinee),
        _ => false,
    }
}

/// A type whose values may be object literals: a crate type or a `Result`.
pub(super) fn holds_object(ty: &Ty) -> bool {
    ty.any(&|t| matches!(t, Ty::Named(_) | Ty::Result { .. } | Ty::Fn { .. }))
}

/// Whether `name` is read or assigned anywhere in `expr`.
pub(super) fn mentions(expr: &Expr, name: &Name) -> bool {
    expr.any(|e| {
        matches!(e, Expr::Var(n) | Expr::Assign { name: n, .. } | Expr::Call { callee: purecrate_ir::Callee::Local(n), .. } if n == name)
    })
}

/// Whether `expr` binds or assigns `name`, or holds a closure that reads it.
pub(crate) fn touches(expr: &Expr, name: &Name) -> bool {
    expr.any(|e| match e {
        Expr::Assign { name: n, .. } => n == name,
        Expr::Closure { body, .. } => body.reads(name),
        e => e.own_bindings().contains(&name),
    })
}
