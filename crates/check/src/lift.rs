//! Hoists every `?` into the value of its own `let`, in evaluation order, so
//! the printer only meets `let x = e?` and can emit an early `return`.
//! `position` has already rejected `?` in places this pass cannot reach.

use purecrate_ir::{Arm, Crate, Expr, Fields, Fn, Item, Name, TryOn};

pub fn lift(krate: Crate) -> Crate {
    let items = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(f) => Item::Fn(Fn {
                body: Lifter::default().stmt(f.body),
                ..f
            }),
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), items)
}

type Hoisted = Vec<(Name, Expr, Option<TryOn>)>;

#[derive(Default)]
struct Lifter {
    next: usize,
}

impl Lifter {
    fn fresh(&mut self) -> Name {
        self.next += 1;
        Name::new(format!("$q{}", self.next))
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
            other => {
                let (value, hoisted) = self.extract(other);
                wrap(hoisted, value)
            }
        }
    }

    fn boxed(&mut self, e: Box<Expr>, out: &mut Hoisted) -> Box<Expr> {
        Box::new(self.extract_into(*e, out))
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
                let name = self.fresh();
                out.push((name.clone(), *inner, on));
                Expr::Var(name)
            }
            Expr::Call { callee, args } => Expr::Call {
                callee,
                args: args.into_iter().map(|a| self.extract_into(a, out)).collect(),
            },
            Expr::Construct {
                ty,
                variant,
                fields,
            } => Expr::Construct {
                ty,
                variant,
                fields: match fields {
                    Fields::Unit => Fields::Unit,
                    Fields::Positional(xs) => {
                        Fields::Positional(xs.into_iter().map(|x| self.extract_into(x, out)).collect())
                    }
                    Fields::Named(xs) => Fields::Named(
                        xs.into_iter()
                            .map(|(n, x)| (n, self.extract_into(x, out)))
                            .collect(),
                    ),
                },
            },
            Expr::Tuple(xs) => Expr::Tuple(xs.into_iter().map(|x| self.extract_into(x, out)).collect()),
            Expr::Array(xs) => Expr::Array(xs.into_iter().map(|x| self.extract_into(x, out)).collect()),
            Expr::Field { base, name } => Expr::Field {
                base: self.boxed(base, out),
                name,
            },
            Expr::Unary { op, expr } => Expr::Unary {
                op,
                expr: self.boxed(expr, out),
            },
            Expr::Binary { op, left, right } => {
                let left = self.boxed(left, out);
                let right = if matches!(op, purecrate_ir::BinOp::And | purecrate_ir::BinOp::Or) {
                    right
                } else {
                    self.boxed(right, out)
                };
                Expr::Binary { op, left, right }
            }
            other => other,
        }
    }
}

fn wrap(hoisted: Hoisted, body: Expr) -> Expr {
    hoisted.into_iter().rev().fold(body, |then, (name, inner, on)| Expr::Let {
        name,
        mutable: false,
        ty: None,
        value: Box::new(Expr::Try {
            expr: Box::new(inner),
            on,
        }),
        then: Box::new(then),
    })
}
