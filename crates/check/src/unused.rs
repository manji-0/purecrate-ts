//! Bindings the Rust leaves unused, so the printed TS passes a consumer's
//! `noUnusedLocals` and `noUnusedParameters`. rustc only warns about them
//! (and says nothing with a leading `_`); TS rejects every unread `const`,
//! whatever its name, but not a parameter or `for..of` variable that starts
//! with `_`.
//!
//! A binding is unused when it is not read in its JS scope (a later binding
//! of the same name in another arm does not count). An unused `let` keeps
//! its value as a statement, so what Rust evaluates (and may panic on) is
//! still evaluated.

use std::collections::HashSet;

use purecrate_ir::{Arm, Callee, ClosureParam, Crate, Expr, Fn, Item, Name, Param, Pattern, VariantBind};

pub fn drop_unused(krate: Crate) -> Crate {
    let items: HashSet<String> = krate.items.iter().map(|item| item.name().as_str().to_string()).collect();
    let dropped = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(f) => Item::Fn(drop_in_fn(f, &items)),
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), dropped)
}

/// Repeated until nothing changes: dropping `let x = $v1` leaves `$v1`,
/// bound by the arm pattern `check::tuple` made, unused in turn.
fn drop_in_fn(f: Fn, items: &HashSet<String>) -> Fn {
    let mut taken: HashSet<String> = items.clone();
    taken.extend(f.params.iter().map(|p| p.name.as_str().to_string()));
    let mut body = f.body;
    loop {
        let mut cx = Cx { taken: taken.clone() };
        let next = cx.expr(body.clone());
        if next == body {
            let params = f
                .params
                .into_iter()
                .map(|p| {
                    let used = used_in(&next, p.name.as_str());
                    Param { name: cx.param_name(p.name, used), ty: p.ty }
                })
                .collect();
            return Fn { params, body: next, ..f };
        }
        body = next;
    }
}

struct Cx {
    /// Every name in the function and the crate's item names, so a `_x`
    /// made here meets none of them.
    taken: HashSet<String>,
}

impl Cx {
    /// An unread parameter or `for..of` variable, renamed with the `_` that
    /// TS exempts from its unused checks.
    fn param_name(&mut self, n: Name, used: bool) -> Name {
        if used || n.as_str().starts_with('_') {
            return n;
        }
        let base = format!("_{}", n.as_str());
        let mut fresh = base.clone();
        let mut k = 1;
        while self.taken.contains(&fresh) {
            fresh = format!("{base}${k}");
            k += 1;
        }
        self.taken.insert(fresh.clone());
        Name::new(fresh)
    }

    fn expr(&mut self, expr: Expr) -> Expr {
        let mut expr = match expr {
            // `collect::<T>()` and `sum::<T>()` are a typed `let $name = e; $name`
            // so the turbofish is the `want` during typing. The name is used once,
            // as that tail, so the binding prints as nothing.
            Expr::Let { name, mutable: false, value, then, .. }
                if name.as_str().starts_with('$') && matches!(then.as_ref(), Expr::Var(n) if n == &name) =>
            {
                return self.expr(*value);
            }
            Expr::Let { name, value, then, .. } if !used_in(&then, name.as_str()) => {
                let value = self.expr(*value);
                let then = self.expr(strip_assigns(*then, &name));
                return if is_pure(&value) { then } else { Expr::Seq { first: Box::new(value), then: Box::new(then) } };
            }
            Expr::Match { scrutinee, arms } => Expr::Match {
                scrutinee,
                arms: arms
                    .into_iter()
                    .map(|a| {
                        let used = |n: &str| a.guard.as_ref().is_some_and(|g| used_in(g, n)) || used_in(&a.body, n);
                        Arm { pattern: drop_pattern(a.pattern, used), guard: a.guard, body: a.body }
                    })
                    .collect(),
            },
            Expr::ForEach { var, over, source, body } => {
                let used = used_in(&body, var.as_str());
                Expr::ForEach { var: self.param_name(var, used), over, source, body }
            }
            Expr::Closure { params, ret, body } => Expr::Closure {
                params: params
                    .into_iter()
                    .map(|p| {
                        let used = used_in(&body, p.name.as_str());
                        ClosureParam { name: self.param_name(p.name, used), ty: p.ty }
                    })
                    .collect(),
                ret,
                body,
            },
            other => other,
        };
        for child in expr.children_mut() {
            let owned = std::mem::replace(child, Expr::Unreachable);
            *child = self.expr(owned);
        }
        expr
    }
}

fn drop_pattern(p: Pattern, used: impl std::ops::Fn(&str) -> bool + Copy) -> Pattern {
    match p {
        Pattern::Var(n) if !used(n.as_str()) => Pattern::Wildcard,
        Pattern::Variant { ty, variant, bind } => Pattern::Variant {
            ty,
            variant,
            bind: match bind {
                VariantBind::Unit => VariantBind::Unit,
                VariantBind::Tuple(ps) => VariantBind::Tuple(ps.into_iter().map(|p| drop_pattern(p, used)).collect()),
                VariantBind::Struct(fs) => {
                    VariantBind::Struct(fs.into_iter().map(|(f, p)| (f, drop_pattern(p, used))).collect())
                }
            },
        },
        Pattern::OptionSome(p) => Pattern::OptionSome(Box::new(drop_pattern(*p, used))),
        Pattern::ResultOk(p) => Pattern::ResultOk(Box::new(drop_pattern(*p, used))),
        Pattern::ResultErr(p) => Pattern::ResultErr(Box::new(drop_pattern(*p, used))),
        Pattern::Or(ps) => Pattern::Or(ps.into_iter().map(|p| drop_pattern(p, used)).collect()),
        Pattern::Tuple(ps) => Pattern::Tuple(ps.into_iter().map(|p| drop_pattern(p, used)).collect()),
        other => other,
    }
}

/// Whether `name` is read in `expr`, stopping at a nested binder of that name.
fn used_in(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Var(n) => n.as_str() == name,
        Expr::Call { callee: Callee::Local(n), args } => n.as_str() == name || args.iter().any(|a| used_in(a, name)),
        Expr::Assign { value, .. } => used_in(value, name),
        Expr::Let { name: n, value, then, .. } => used_in(value, name) || (n.as_str() != name && used_in(then, name)),
        Expr::Match { scrutinee, arms } => used_in(scrutinee, name) || arms.iter().any(|a| arm_uses(a, name)),
        Expr::For { var, start, end, body, .. } => {
            used_in(start, name) || used_in(end, name) || (var.as_str() != name && used_in(body, name))
        }
        Expr::ForEach { var, source, body, .. } => {
            used_in(source, name) || (var.as_str() != name && used_in(body, name))
        }
        Expr::Closure { params, body, .. } => !params.iter().any(|p| p.name.as_str() == name) && used_in(body, name),
        _ => expr.children().iter().any(|c| used_in(c, name)),
    }
}

fn arm_uses(a: &Arm, name: &str) -> bool {
    if a.pattern.bindings().iter().any(|b| b.as_str() == name) {
        return false;
    }
    a.guard.as_ref().is_some_and(|g| used_in(g, name)) || used_in(&a.body, name)
}

/// Writes to `name` after its unused `let` was dropped: keep a non-pure
/// value as a statement, drop the rest.
fn strip_assigns(expr: Expr, name: &Name) -> Expr {
    match expr {
        Expr::Assign { name: n, value } if n == *name => {
            if is_pure(&value) {
                Expr::Lit(purecrate_ir::Lit::Unit)
            } else {
                *value
            }
        }
        other => {
            let mut other = other;
            for child in other.children_mut() {
                let owned = std::mem::replace(child, Expr::Unreachable);
                *child = strip_assigns(owned, name);
            }
            other
        }
    }
}

/// Dropping it changes nothing: no call, arithmetic, or index that could
/// panic or loop.
fn is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Var(_) | Expr::Lit(_) | Expr::Closure { .. } => true,
        Expr::Field { base, .. } => is_pure(base),
        _ => false,
    }
}
