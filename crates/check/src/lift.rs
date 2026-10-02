//! Hoists every `?` into the value of its own `let`, in evaluation order, so
//! the printer only meets `let x = e?` and can emit an early `return`.
//! `position` has already rejected `?` in places this pass cannot reach.

use purecrate_ir::{Arm, BinOp, Callee, Crate, Expr, Fields, Fn, Item, Lit, Name, TryOn, UnOp};

pub fn lift(krate: Crate) -> Crate {
    let items = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(f) => Item::Fn(Fn {
                body: lift_body(f.body),
                ..f
            }),
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), items)
}

/// A closure body is printed as its own arrow body, where `?` returns from
/// the closure, so each is lifted on its own, innermost first.
fn lift_body(mut body: Expr) -> Expr {
    lift_closures(&mut body);
    let mut body = Lifter::for_body(&body).stmt(body);
    guard_ok_or(&mut body);
    body
}

/// `let x = opt.ok_or(e)?` as a guard: `ok_or` lowers to a `match` that
/// builds a `Result` only for `?` to take it apart again, which printed as
/// an inline function. `e` still runs before the test, as `ok_or` is eager:
///
/// ```text
/// let $opt = opt; let $arg = e;
/// if $opt.is_none() { return Err($arg) }
/// let x = $opt;
/// ```
fn guard_ok_or(expr: &mut Expr) {
    if let Expr::Let { name, mutable, ty, value, then } = expr {
        if let Some((opt, opt_ty, recv, arg, arg_ty, e)) = ok_or_try(value) {
            let then = std::mem::replace(&mut **then, Expr::Unreachable);
            let guard = Expr::If {
                cond: Box::new(Expr::Call { callee: Callee::OptionIsNone, args: vec![Expr::Var(opt.clone())] }),
                then: Box::new(Expr::Return(Box::new(Expr::Call { callee: Callee::ResultErr, args: vec![Expr::Var(arg.clone())] }))),
                else_: Box::new(Expr::Lit(Lit::Unit)),
            };
            let bind = Expr::Let { name: name.clone(), mutable: *mutable, ty: ty.clone(), value: Box::new(Expr::Var(opt.clone())), then: Box::new(then) };
            *expr = Expr::Let {
                name: opt,
                mutable: false,
                ty: opt_ty,
                value: Box::new(recv),
                then: Box::new(Expr::Let {
                    name: arg,
                    mutable: false,
                    ty: arg_ty,
                    value: Box::new(e),
                    then: Box::new(Expr::Seq { first: Box::new(guard), then: Box::new(bind) }),
                }),
            };
        }
    }
    expr.children_mut().into_iter().for_each(guard_ok_or);
}

/// The pieces of `Try` on what `option_method` builds for `ok_or`:
/// `let $opt = recv; let $arg = e; let $res = match $opt { Some(s) => Ok(s),
/// None => Err($arg) }; $res`.
#[allow(clippy::type_complexity)]
fn ok_or_try(value: &Expr) -> Option<(Name, Option<purecrate_ir::Ty>, Expr, Name, Option<purecrate_ir::Ty>, Expr)> {
    let Expr::Try { expr, on: Some(TryOn::Result) } = value else { return None };
    let Expr::Let { name: opt, ty: opt_ty, value: recv, then, .. } = &**expr else { return None };
    let Expr::Let { name: arg, ty: arg_ty, value: e, then, .. } = &**then else { return None };
    let Expr::Let { name: res, value: m, then, .. } = &**then else { return None };
    if **then != Expr::Var(res.clone()) {
        return None;
    }
    let Expr::Match { scrutinee, arms } = &**m else { return None };
    let [some, none] = arms.as_slice() else { return None };
    let ok = match (&some.pattern, &some.body) {
        (purecrate_ir::Pattern::OptionSome(p), Expr::Call { callee: Callee::ResultOk, args }) => {
            matches!((&**p, args.as_slice()), (purecrate_ir::Pattern::Var(s), [Expr::Var(v)]) if s == v)
        }
        _ => false,
    };
    let err = matches!((&none.pattern, &none.body), (purecrate_ir::Pattern::OptionNone, Expr::Call { callee: Callee::ResultErr, args })
        if args.as_slice() == [Expr::Var(arg.clone())]);
    (**scrutinee == Expr::Var(opt.clone()) && ok && err && some.guard.is_none() && none.guard.is_none())
        .then(|| (opt.clone(), opt_ty.clone(), (**recv).clone(), arg.clone(), arg_ty.clone(), (**e).clone()))
}

fn lift_closures(expr: &mut Expr) {
    if let Expr::Closure { body, .. } = expr {
        let inner = std::mem::replace(&mut **body, Expr::Unreachable);
        **body = lift_body(inner);
        return;
    }
    expr.children_mut().into_iter().for_each(lift_closures);
}

/// A binding placed in front of the statement: `let name = inner?` when the
/// kind is set, else `let name = inner`.
type Hoisted = Vec<(Name, Expr, Option<Option<TryOn>>)>;

#[derive(Default)]
struct Lifter {
    /// Every name the body binds, and those this pass has made.
    taken: std::collections::HashSet<String>,
}

impl Lifter {
    fn for_body(body: &Expr) -> Self {
        let mut taken = std::collections::HashSet::new();
        fn walk(e: &Expr, out: &mut std::collections::HashSet<String>) {
            match e {
                Expr::Let { name, .. } | Expr::For { var: name, .. } | Expr::ForEach { var: name, .. } => {
                    out.insert(name.as_str().to_string());
                }
                Expr::Match { arms, .. } => {
                    for a in arms {
                        out.extend(a.pattern.bindings().into_iter().map(|n| n.as_str().to_string()));
                    }
                }
                Expr::Closure { params, .. } => out.extend(params.iter().map(|p| p.name.as_str().to_string())),
                _ => {}
            }
            e.children().into_iter().for_each(|c| walk(c, out));
        }
        walk(body, &mut taken);
        Lifter { taken }
    }

    /// `$<f><What>` for `f(..)` (`$removeSkuResult`), `$<what>` otherwise,
    /// kept apart from every name taken. The printer drops the `$`.
    fn fresh(&mut self, inner: &Expr, what: &str) -> Name {
        let base = match inner {
            Expr::Call { callee: Callee::Fn(f) | Callee::Method { name: f, .. }, .. } => {
                format!("${}{what}", purecrate_ir::to_camel(f.as_str()))
            }
            _ => format!("${}", what.to_ascii_lowercase()),
        };
        let name = (1..).map(|i| if i == 1 { base.clone() } else { format!("{base}{i}") }).find(|n| !self.taken.contains(n)).expect("a free name");
        self.taken.insert(name.clone());
        Name::new(name)
    }

    fn stmt(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Let {
                name,
                mutable,
                ty,
                value,
                then,
            } => {
                let (value, hoisted) = match *value {
                    Expr::Try { expr, on } => {
                        let (inner, hoisted) = self.extract(*expr);
                        (
                            Expr::Try {
                                expr: Box::new(inner),
                                on,
                            },
                            hoisted,
                        )
                    }
                    v if v.needs_statements() => (self.stmt(v), Vec::new()),
                    v => self.extract(v),
                };
                let then = self.stmt(*then);
                wrap(
                    hoisted,
                    Expr::Let {
                        name,
                        mutable,
                        ty,
                        value: Box::new(value),
                        then: Box::new(then),
                    },
                )
            }
            Expr::If { cond, then, else_ } => {
                let (cond, hoisted) = self.extract(*cond);
                let (then, else_) = (self.stmt(*then), self.stmt(*else_));
                wrap(
                    hoisted,
                    Expr::If {
                        cond: Box::new(cond),
                        then: Box::new(then),
                        else_: Box::new(else_),
                    },
                )
            }
            Expr::Match { scrutinee, arms } => {
                let (scrutinee, hoisted) = self.extract(*scrutinee);
                let arms = arms
                    .into_iter()
                    .map(|a| Arm {
                        guard: None,
                        pattern: a.pattern,
                        body: self.stmt(a.body),
                    })
                    .collect();
                wrap(
                    hoisted,
                    Expr::Match {
                        scrutinee: Box::new(scrutinee),
                        arms,
                    },
                )
            }
            Expr::Return(value) => {
                let (value, hoisted) = self.extract(*value);
                wrap(hoisted, Expr::Return(Box::new(value)))
            }
            Expr::Assign { name, value } if value.needs_statements() => Expr::Assign {
                name,
                value: Box::new(self.stmt(*value)),
            },
            Expr::Assign { name, value } => {
                let (value, hoisted) = self.extract(*value);
                wrap(
                    hoisted,
                    Expr::Assign {
                        name,
                        value: Box::new(value),
                    },
                )
            }
            Expr::Seq { first, then } => Expr::Seq {
                first: Box::new(self.stmt(*first)),
                then: Box::new(self.stmt(*then)),
            },
            // The condition runs before every pass, so a `?` in it cannot be
            // hoisted in front of the loop: the loop becomes `while true`
            // whose body computes the condition first and leaves when false.
            Expr::While { cond, body } => {
                let (cond, hoisted) = self.extract(*cond);
                let body = self.stmt(*body);
                if hoisted.is_empty() {
                    return Expr::While { cond: Box::new(cond), body: Box::new(body) };
                }
                let exit = Expr::If {
                    cond: Box::new(Expr::Unary { op: UnOp::Not, expr: Box::new(cond) }),
                    then: Box::new(Expr::Break),
                    else_: Box::new(Expr::Lit(Lit::Unit)),
                };
                Expr::While {
                    cond: Box::new(Expr::Lit(Lit::Bool(true))),
                    body: Box::new(wrap(hoisted, Expr::Seq { first: Box::new(exit), then: Box::new(body) })),
                }
            }
            // The bounds run once before the loop; a `?` in the body leaves
            // from inside it.
            Expr::For { var, ty, start, end, body } => {
                let mut hoisted = Vec::new();
                let start = self.extract_into(*start, &mut hoisted);
                let end = self.extract_into(*end, &mut hoisted);
                let body = self.stmt(*body);
                wrap(
                    hoisted,
                    Expr::For {
                        var,
                        ty,
                        start: Box::new(start),
                        end: Box::new(end),
                        body: Box::new(body),
                    },
                )
            }
            Expr::ForEach { var, over, source: string, body } => {
                let mut hoisted = Vec::new();
                let string = self.extract_into(*string, &mut hoisted);
                let body = self.stmt(*body);
                wrap(
                    hoisted,
                    Expr::ForEach {
                        var,
                        over,
                        source: Box::new(string),
                        body: Box::new(body),
                    },
                )
            }
            other => {
                let (value, hoisted) = self.extract(other);
                wrap(hoisted, value)
            }
        }
    }

    /// Rewrites in place, reusing the allocation.
    fn boxed(&mut self, mut e: Box<Expr>, out: &mut Hoisted) -> Box<Expr> {
        *e = self.extract_into(std::mem::replace(&mut *e, Expr::Unreachable), out);
        e
    }

    /// Replaces each `?` in a strict position with a fresh variable.
    fn extract(&mut self, expr: Expr) -> (Expr, Hoisted) {
        let mut hoisted = Vec::new();
        let expr = self.extract_into(expr, &mut hoisted);
        (expr, hoisted)
    }

    fn extract_into(&mut self, expr: Expr, out: &mut Hoisted) -> Expr {
        match expr {
            Expr::Try { expr, on } => {
                let inner = self.boxed(expr, out);
                let name = self.fresh(&inner, if on == Some(TryOn::Option) { "Opt" } else { "Result" });
                // The hoisted value is the `Result` itself, tested in place
                // (see `wrap`), and read here as its payload. A `None` is
                // `null`, so an `Option` is its own payload; so is the guard
                // `ok_or` becomes, which binds the payload.
                let guarded = ok_or_try(&Expr::Try { expr: inner.clone(), on }).is_some();
                let read = if on == Some(TryOn::Option) || guarded {
                    Expr::Var(name.clone())
                } else {
                    Expr::Field { base: Box::new(Expr::Var(name.clone())), name: Name::new("value") }
                };
                out.push((name, *inner, Some(on)));
                read
            }
            Expr::Call { callee, args } => Expr::Call {
                callee,
                args: self.in_order(args, out),
            },
            Expr::Construct {
                ty,
                variant,
                fields,
                base,
            } => {
                let (shape, names, values): (Shape, Vec<Name>, Vec<Expr>) = match fields {
                    Fields::Unit => (Shape::Unit, Vec::new(), Vec::new()),
                    Fields::Positional(xs) => (Shape::Positional, Vec::new(), xs),
                    Fields::Named(xs) => {
                        let (names, values) = xs.into_iter().unzip();
                        (Shape::Named, names, values)
                    }
                };
                let mut all = values;
                let has_base = base.is_some();
                all.extend(base.map(|b| *b));
                let mut all = self.in_order(all, out);
                // TS evaluates `...base` before the fields; Rust evaluates it
                // after. When the base can panic, the fields go first.
                let base = if has_base {
                    let b = all.pop().expect("pushed above");
                    if !pure(&b) {
                        all = all.into_iter().map(|x| self.spill(x, out)).collect();
                    }
                    Some(Box::new(self.spill(b, out)))
                } else {
                    None
                };
                let fields = match shape {
                    Shape::Unit => Fields::Unit,
                    Shape::Positional => Fields::Positional(all),
                    Shape::Named => Fields::Named(names.into_iter().zip(all).collect()),
                };
                Expr::Construct {
                    ty,
                    variant,
                    fields,
                    base,
                }
            }
            Expr::Tuple(xs) => Expr::Tuple(self.in_order(xs, out)),
            Expr::Array(xs) => Expr::Array(self.in_order(xs, out)),
            Expr::Field { base, name } => Expr::Field {
                base: self.boxed(base, out),
                name,
            },
            Expr::Index { base, index } => {
                let mut xs = self.in_order(vec![*base, *index], out);
                let index = xs.pop().expect("two");
                let base = xs.pop().expect("two");
                Expr::Index {
                    base: Box::new(base),
                    index: Box::new(index),
                }
            }
            Expr::Unary { op, expr } => Expr::Unary {
                op,
                expr: self.boxed(expr, out),
            },
            Expr::Ignored { wrapper, expr } => Expr::Ignored {
                wrapper,
                expr: self.boxed(expr, out),
            },
            Expr::Binary {
                op: op @ (BinOp::And | BinOp::Or),
                left,
                right,
            } => Expr::Binary {
                op,
                left: self.boxed(left, out),
                right,
            },
            Expr::Binary { op, left, right } => {
                let mut xs = self.in_order(vec![*left, *right], out);
                let right = xs.pop().expect("two");
                let left = xs.pop().expect("two");
                Expr::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }
            other => other,
        }
    }

    /// Siblings evaluated left to right. Hoisting a `?` out of one moves it
    /// in front of the whole statement, so every earlier sibling that could
    /// panic is bound first, keeping Rust's order.
    fn in_order(&mut self, xs: Vec<Expr>, out: &mut Hoisted) -> Vec<Expr> {
        let mut done: Vec<Expr> = Vec::with_capacity(xs.len());
        for x in xs {
            let mut own = Vec::new();
            let x = self.extract_into(x, &mut own);
            if !own.is_empty() {
                done = std::mem::take(&mut done).into_iter().map(|d| self.spill(d, out)).collect();
            }
            out.extend(own);
            done.push(x);
        }
        done
    }

    /// Binds `expr` to a fresh variable unless evaluating it cannot panic.
    /// Literal shapes keep their shape, so TS still sees the literal type.
    fn spill(&mut self, expr: Expr, out: &mut Hoisted) -> Expr {
        match expr {
            e if pure(&e) => e,
            Expr::Construct {
                ty,
                variant,
                fields,
                base,
            } => {
                let fields = match fields {
                    Fields::Unit => Fields::Unit,
                    Fields::Positional(xs) => Fields::Positional(xs.into_iter().map(|x| self.spill(x, out)).collect()),
                    Fields::Named(xs) => Fields::Named(xs.into_iter().map(|(n, x)| (n, self.spill(x, out))).collect()),
                };
                let base = base.map(|b| Box::new(self.spill(*b, out)));
                Expr::Construct {
                    ty,
                    variant,
                    fields,
                    base,
                }
            }
            Expr::Tuple(xs) => Expr::Tuple(xs.into_iter().map(|x| self.spill(x, out)).collect()),
            Expr::Array(xs) => Expr::Array(xs.into_iter().map(|x| self.spill(x, out)).collect()),
            Expr::Ignored { wrapper, expr } => Expr::Ignored {
                wrapper,
                expr: Box::new(self.spill(*expr, out)),
            },
            other => {
                let name = self.fresh(&other, "Value");
                out.push((name.clone(), other, None));
                Expr::Var(name)
            }
        }
    }
}

enum Shape {
    Unit,
    Positional,
    Named,
}

/// Evaluating it has no effect and cannot panic.
fn pure(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Var(_) | Expr::Closure { .. } => true,
        Expr::Field { base, .. } => pure(base),
        Expr::Call {
            callee: Callee::OptionNone,
            ..
        } => true,
        _ => false,
    }
}

/// `let name = inner; name?; body` for a hoisted `?`: the test is on the
/// binding itself, and the use reads its payload (`extract_into`). The
/// guard `ok_or` becomes keeps `let name = inner?`, which binds the payload.
fn wrap(hoisted: Hoisted, body: Expr) -> Expr {
    hoisted.into_iter().rev().fold(body, |then, (name, inner, on)| {
        let (value, then) = match on {
            Some(on) if ok_or_try(&Expr::Try { expr: Box::new(inner.clone()), on }).is_some() => {
                (Expr::Try { expr: Box::new(inner), on }, then)
            }
            Some(on) => (
                inner,
                Expr::Seq {
                    first: Box::new(Expr::Try { expr: Box::new(Expr::Var(name.clone())), on }),
                    then: Box::new(then),
                },
            ),
            None => (inner, then),
        };
        Expr::Let { name, mutable: false, ty: None, value: Box::new(value), then: Box::new(then) }
    })
}
