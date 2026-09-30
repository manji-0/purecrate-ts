//! Bindings the Rust leaves unused, so the printed TS passes a consumer's
//! `noUnusedLocals` and `noUnusedParameters`. rustc only warns about them
//! (and says nothing with a leading `_`); TS rejects every unread `const`,
//! whatever its name, but not a parameter or `for..of` variable that starts
//! with `_`.
//!
//! Runs after `rename`, so a name is bound once per function and "unused"
//! is "never mentioned". An unused `let` keeps its value as a statement, so
//! what Rust evaluates (and may panic on) is still evaluated.

use std::collections::HashSet;

use purecrate_ir::{Callee, ClosureParam, Crate, Expr, Fn, Item, Lit, Name, Param, Pattern, VariantBind};

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
        let mut used = HashSet::new();
        mentions(&body, &mut used);
        taken.extend(used.iter().cloned());
        let mut cx = Cx { used, taken: taken.clone() };
        let next = cx.expr(body.clone());
        if next == body {
            let params = f
                .params
                .into_iter()
                .map(|p| Param {
                    name: cx.param_name(p.name),
                    ty: p.ty,
                })
                .collect();
            return Fn { params, body: next, ..f };
        }
        body = next;
    }
}

struct Cx {
    used: HashSet<String>,
    /// Every name in the function and the crate's item names, so a `_x`
    /// made here meets none of them.
    taken: HashSet<String>,
}

impl Cx {
    fn is_used(&self, n: &Name) -> bool {
        self.used.contains(n.as_str())
    }

    /// An unread parameter or `for..of` variable, renamed with the `_` that
    /// TS exempts from its unused checks.
    fn param_name(&mut self, n: Name) -> Name {
        if self.is_used(&n) || n.as_str().starts_with('_') {
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
            Expr::Let { name, value, then, .. } if !self.is_used(&name) => {
                let value = self.expr(*value);
                let then = self.expr(*then);
                return if is_pure(&value) {
                    then
                } else {
                    Expr::Seq {
                        first: Box::new(value),
                        then: Box::new(then),
                    }
                };
            }
            Expr::Assign { name, value } if !self.is_used(&name) => {
                let value = self.expr(*value);
                return if is_pure(&value) {
                    Expr::Lit(Lit::Unit)
                } else {
                    Expr::Seq {
                        first: Box::new(value),
                        then: Box::new(Expr::Lit(Lit::Unit)),
                    }
                };
            }
            Expr::Match { scrutinee, arms } => Expr::Match {
                scrutinee,
                arms: arms
                    .into_iter()
                    .map(|mut a| {
                        a.pattern = self.pattern(a.pattern);
                        a
                    })
                    .collect(),
            },
            Expr::ForChars { var, string, body } => Expr::ForChars {
                var: self.param_name(var),
                string,
                body,
            },
            Expr::Closure { params, ret, body } => Expr::Closure {
                params: params
                    .into_iter()
                    .map(|p| ClosureParam {
                        name: self.param_name(p.name),
                        ty: p.ty,
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

    fn pattern(&self, p: Pattern) -> Pattern {
        match p {
            Pattern::Var(n) if !self.is_used(&n) => Pattern::Wildcard,
            Pattern::Variant { ty, variant, bind } => Pattern::Variant {
                ty,
                variant,
                bind: match bind {
                    VariantBind::Unit => VariantBind::Unit,
                    VariantBind::Tuple(ps) => VariantBind::Tuple(ps.into_iter().map(|p| self.pattern(p)).collect()),
                    VariantBind::Struct(fs) => {
                        VariantBind::Struct(fs.into_iter().map(|(f, p)| (f, self.pattern(p))).collect())
                    }
                },
            },
            Pattern::OptionSome(p) => Pattern::OptionSome(Box::new(self.pattern(*p))),
            Pattern::ResultOk(p) => Pattern::ResultOk(Box::new(self.pattern(*p))),
            Pattern::ResultErr(p) => Pattern::ResultErr(Box::new(self.pattern(*p))),
            Pattern::Or(ps) => Pattern::Or(ps.into_iter().map(|p| self.pattern(p)).collect()),
            Pattern::Tuple(ps) => Pattern::Tuple(ps.into_iter().map(|p| self.pattern(p)).collect()),
            other => other,
        }
    }
}

/// Names read anywhere in `expr`. An assignment is not a read: TS reports a
/// binding that is only written, so the writes go too.
fn mentions(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Var(n) => {
            out.insert(n.as_str().to_string());
        }
        Expr::Call {
            callee: Callee::Local(n), ..
        } => {
            out.insert(n.as_str().to_string());
        }
        _ => {}
    }
    expr.children().into_iter().for_each(|c| mentions(c, out));
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
