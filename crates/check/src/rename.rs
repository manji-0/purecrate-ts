//! Gives every binding in a function body a name no other binding in that
//! function uses. Rust shadowing then survives both TS block scoping (no
//! duplicate `const`) and statement lowering, where a `let` target and an arm
//! binding of the same name meet in one scope.

use std::collections::HashMap;

use purecrate_ir::{Arm, Callee, ClosureParam, Crate, Expr, Fields, Fn, Item, Name, Param, Pattern, VariantBind};

pub fn rename(krate: Crate) -> Crate {
    let items: Vec<String> = krate
        .items
        .iter()
        .filter(|item| !matches!(item, Item::Fn(f) if f.owner.is_some()))
        .map(|item| item.name().as_str().to_string())
        .collect();
    let renamed = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(f) => Item::Fn(rename_fn(f, &items)),
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), renamed)
}

/// Top-level names are taken up front: a TS `const` shadows an import for the
/// whole block, including uses before it that Rust resolves to the item.
fn rename_fn(f: Fn, items: &[String]) -> Fn {
    let mut r = Renamer::default();
    items.iter().for_each(|n| {
        r.claim(n);
    });
    let mut env = Env::new();
    let params = f
        .params
        .into_iter()
        .map(|p| Param {
            name: r.bind(&p.name, &mut env),
            ..p
        })
        .collect();
    Fn {
        body: r.expr(f.body, &env),
        params,
        ..f
    }
}

/// Source name to the name it prints as, for the bindings in scope.
type Env = HashMap<String, Name>;

#[derive(Default)]
struct Renamer {
    /// How many bindings in this function have used each source name.
    used: HashMap<String, usize>,
}

impl Renamer {
    fn claim(&mut self, source: &str) -> Name {
        let count = self.used.entry(source.to_string()).or_insert(0);
        *count += 1;
        match *count {
            1 => Name::new(source),
            n => Name::new(format!("{source}${}", n - 1)),
        }
    }

    fn bind(&mut self, name: &Name, env: &mut Env) -> Name {
        let printed = self.claim(name.as_str());
        env.insert(name.as_str().to_string(), printed.clone());
        printed
    }

    fn pattern(&mut self, p: Pattern, env: &mut Env) -> Pattern {
        match p {
            Pattern::Var(n) => Pattern::Var(self.bind(&n, env)),
            Pattern::Variant { ty, variant, bind } => Pattern::Variant {
                ty,
                variant,
                bind: match bind {
                    VariantBind::Unit => VariantBind::Unit,
                    VariantBind::Tuple(ps) => {
                        VariantBind::Tuple(ps.into_iter().map(|p| self.pattern(p, env)).collect())
                    }
                    VariantBind::Struct(ps) => VariantBind::Struct(
                        ps.into_iter().map(|(f, p)| (f, self.pattern(p, env))).collect(),
                    ),
                },
            },
            Pattern::OptionSome(p) => Pattern::OptionSome(Box::new(self.pattern(*p, env))),
            Pattern::ResultOk(p) => Pattern::ResultOk(Box::new(self.pattern(*p, env))),
            Pattern::ResultErr(p) => Pattern::ResultErr(Box::new(self.pattern(*p, env))),
            other @ (Pattern::Wildcard | Pattern::Lit(_) | Pattern::OptionNone) => other,
        }
    }

    fn boxed(&mut self, e: Box<Expr>, env: &Env) -> Box<Expr> {
        Box::new(self.expr(*e, env))
    }

    fn all(&mut self, xs: Vec<Expr>, env: &Env) -> Vec<Expr> {
        xs.into_iter().map(|x| self.expr(x, env)).collect()
    }

    fn expr(&mut self, e: Expr, env: &Env) -> Expr {
        match e {
            Expr::At { .. } => unreachable!("`accept` removes positions before renaming"),
            Expr::Var(n) => Expr::Var(env.get(n.as_str()).cloned().unwrap_or(n)),
            Expr::Let {
                name,
                mutable,
                ty,
                value,
                then,
            } => {
                let value = self.boxed(value, env);
                let mut inner = env.clone();
                let name = self.bind(&name, &mut inner);
                Expr::Let {
                    name,
                    mutable,
                    ty,
                    value,
                    then: self.boxed(then, &inner),
                }
            }
            Expr::If { cond, then, else_ } => Expr::If {
                cond: self.boxed(cond, env),
                then: self.boxed(then, env),
                else_: self.boxed(else_, env),
            },
            Expr::Match { scrutinee, arms } => Expr::Match {
                scrutinee: self.boxed(scrutinee, env),
                arms: arms
                    .into_iter()
                    .map(|a| {
                        let mut inner = env.clone();
                        let pattern = self.pattern(a.pattern, &mut inner);
                        Arm {
                            pattern,
                            body: self.expr(a.body, &inner),
                        }
                    })
                    .collect(),
            },
            Expr::Call {
                callee: Callee::Local(n),
                args,
            } => Expr::Call {
                callee: Callee::Local(env.get(n.as_str()).cloned().unwrap_or(n)),
                args: self.all(args, env),
            },
            Expr::Call { callee, args } => Expr::Call {
                callee,
                args: self.all(args, env),
            },
            Expr::Closure { params, ret, body } => {
                let mut inner = env.clone();
                let params = params
                    .into_iter()
                    .map(|p| ClosureParam {
                        name: self.bind(&p.name, &mut inner),
                        ty: p.ty,
                    })
                    .collect();
                Expr::Closure {
                    params,
                    ret,
                    body: self.boxed(body, &inner),
                }
            }
            Expr::MethodCall { receiver, name, args } => Expr::MethodCall {
                receiver: self.boxed(receiver, env),
                name,
                args: self.all(args, env),
            },
            Expr::Construct {
                ty,
                variant,
                fields,
                base,
            } => Expr::Construct {
                ty,
                variant,
                fields: match fields {
                    Fields::Unit => Fields::Unit,
                    Fields::Positional(xs) => Fields::Positional(self.all(xs, env)),
                    Fields::Named(xs) => Fields::Named(
                        xs.into_iter().map(|(n, x)| (n, self.expr(x, env))).collect(),
                    ),
                },
                base: base.map(|b| self.boxed(b, env)),
            },
            Expr::Field { base, name } => Expr::Field {
                base: self.boxed(base, env),
                name,
            },
            Expr::Index { base, index } => Expr::Index {
                base: self.boxed(base, env),
                index: self.boxed(index, env),
            },
            Expr::Tuple(xs) => Expr::Tuple(self.all(xs, env)),
            Expr::Array(xs) => Expr::Array(self.all(xs, env)),
            Expr::Binary { op, left, right } => Expr::Binary {
                op,
                left: self.boxed(left, env),
                right: self.boxed(right, env),
            },
            Expr::Unary { op, expr } => Expr::Unary {
                op,
                expr: self.boxed(expr, env),
            },
            Expr::Return(v) => Expr::Return(self.boxed(v, env)),
            Expr::Assign { name, value } => Expr::Assign {
                name: env.get(name.as_str()).cloned().unwrap_or(name),
                value: self.boxed(value, env),
            },
            Expr::Seq { first, then } => Expr::Seq {
                first: self.boxed(first, env),
                then: self.boxed(then, env),
            },
            Expr::Try { expr, on } => Expr::Try {
                expr: self.boxed(expr, env),
                on,
            },
            Expr::Ignored { wrapper, expr } => Expr::Ignored {
                wrapper,
                expr: self.boxed(expr, env),
            },
            other @ (Expr::Lit(_) | Expr::Unreachable) => other,
        }
    }
}
