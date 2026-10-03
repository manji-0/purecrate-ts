//! A `match` that `tuple` lowered binds each variant field, `Some`, `Ok`, or
//! `Err` payload to a fresh `$` name, and each row then binds its own name
//! to that: `const $f1 = m.conversion; .. const conversion = $f1 as T;`.
//! Where such a `let` exists, the pattern binds the row's name directly, the
//! `let` goes, and every other read of the fresh name (a guard's, another
//! row's `let`) reads the row's name. `rename` numbers a name that shadows a
//! live one, but sibling scopes (two rows, a guard's two sides) may reuse
//! one: the row's name is taken only where neither the pattern nor the rest
//! of the arm binds it, so binding it a little earlier shadows nothing; the
//! value is the same, as the pattern reads it once either way.

use purecrate_ir::{Crate, Expr, Fn, Item, Name, Pattern, VariantBind};

pub fn merge(krate: Crate) -> Crate {
    let items = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(mut f) => {
                merge_in(&mut f.body);
                Item::Fn(Fn { ..f })
            }
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), items)
}

fn merge_in(expr: &mut Expr) {
    if let Expr::Match { arms, .. } = expr {
        for arm in arms.iter_mut().filter(|a| a.guard.is_none()) {
            let fresh: Vec<Name> = arm.pattern.bindings().into_iter().filter(|n| n.as_str().starts_with('$')).cloned().collect();
            for from in fresh {
                let mut body = arm.body.clone();
                let Some(to) = take_copy(&mut body, &from) else { continue };
                if arm.pattern.bindings().contains(&&to) || binds(&body, &to, &from) {
                    continue;
                }
                arm.body = body;
                rename_binding(&mut arm.pattern, &from, &to);
                rename_reads(&mut arm.body, &from, &to);
            }
        }
    }
    expr.children_mut().into_iter().for_each(merge_in);
}

/// Whether `expr` binds `name` anywhere: a `let`, an arm, a loop variable,
/// or a closure parameter. Another row's `let name = from` is the same
/// value under the same name, which the printer leaves out once merged.
fn binds(expr: &Expr, name: &Name, from: &Name) -> bool {
    let here = match expr {
        Expr::Let { name: n, value, .. } => n == name && !matches!(&**value, Expr::Var(v) if v == from),
        Expr::For { var, .. } | Expr::ForEach { var, .. } => var == name,
        Expr::Match { arms, .. } => arms.iter().any(|a| a.pattern.bindings().contains(&name)),
        Expr::Closure { params, .. } => params.iter().any(|p| &p.name == name),
        _ => false,
    };
    here || expr.children().into_iter().any(|c| binds(c, name, from))
}

fn rename_reads(expr: &mut Expr, from: &Name, to: &Name) {
    match expr {
        Expr::Var(n) if n == from => *n = to.clone(),
        _ => expr.children_mut().into_iter().for_each(|c| rename_reads(c, from, to)),
    }
}

/// Replaces the first immutable `let to = from; then` in `expr`, in
/// evaluation order, with `then` and returns `to`.
fn take_copy(expr: &mut Expr, from: &Name) -> Option<Name> {
    if let Expr::Let { name, mutable: false, value, then, .. } = expr {
        if matches!(&**value, Expr::Var(v) if v == from) {
            let to = name.clone();
            let rest = std::mem::replace(&mut **then, Expr::Unreachable);
            *expr = rest;
            return Some(to);
        }
    }
    expr.children_mut().into_iter().find_map(|c| take_copy(c, from))
}

fn rename_binding(pattern: &mut Pattern, from: &Name, to: &Name) {
    match pattern {
        Pattern::Var(n) if n == from => *n = to.clone(),
        Pattern::Variant { bind, .. } => match bind {
            VariantBind::Unit => {}
            VariantBind::Tuple(ps) => ps.iter_mut().for_each(|p| rename_binding(p, from, to)),
            VariantBind::Struct(ps) => ps.iter_mut().for_each(|(_, p)| rename_binding(p, from, to)),
        },
        Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => rename_binding(p, from, to),
        Pattern::Or(ps) | Pattern::Tuple(ps) => ps.iter_mut().for_each(|p| rename_binding(p, from, to)),
        _ => {}
    }
}
