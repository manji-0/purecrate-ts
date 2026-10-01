//! Gives every binding a name no other binding in the same JS scope uses.
//! Match arms and other block-scoped bindings reuse the Rust name; a name is
//! numbered (`method$1`) only when it shadows a live binding or an import.
//!
//! Every function, method, `const`, parameter, and local gets its TS
//! spelling here too, `to_camel` (`compare_pre_ids` → `comparePreIds`), so a
//! local whose spelling meets another's is told apart like a shadowed one.
//! Fields, types, and variants keep their names: a field is the JSON key.

use std::collections::{HashMap, HashSet};

use purecrate_ir::{
    to_camel, Arm, Callee, ClosureParam, Crate, Expr, Fields, Fn, Item, Name, Param, Pattern,
    VariantBind,
};

fn camel(n: &Name) -> Name {
    Name::new(to_camel(n.as_str()))
}

pub fn rename(krate: Crate) -> Crate {
    let items: Vec<String> = krate
        .items
        .iter()
        .filter(|item| !matches!(item, Item::Fn(f) if f.owner.is_some()))
        .map(|item| to_camel(item.name().as_str()))
        .collect();
    let renamed = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(f) => {
                let f = rename_fn(f, &items);
                Item::Fn(Fn {
                    name: camel(&f.name),
                    ..f
                })
            }
            Item::Const(c) => Item::Const(purecrate_ir::Const {
                name: camel(&c.name),
                ..c
            }),
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), renamed)
}

/// Top-level names are taken up front: a TS `const` shadows an import for the
/// whole block, including uses before it that Rust resolves to the item.
fn rename_fn(f: Fn, items: &[String]) -> Fn {
    let mut r = Renamer;
    let mut cx = Cx {
        env: HashMap::new(),
        taken: items.iter().cloned().collect(),
    };
    let params = f
        .params
        .into_iter()
        .map(|p| Param {
            name: r.bind(&p.name, &mut cx),
            ..p
        })
        .collect();
    Fn {
        body: r.stmt_binds(f.body, &mut cx),
        params,
        ..f
    }
}

/// Source name to the name it prints as, for the bindings in scope.
type Env = HashMap<String, Name>;

/// One JS block: `env` maps a source name to what it prints as now, `taken`
/// every printed name declared in this block or an outer one. Shadowing
/// overwrites `env` but leaves the earlier printed name in `taken`, so a
/// third `let intent` still sees the first.
#[derive(Clone)]
struct Cx {
    env: Env,
    taken: HashSet<String>,
}

struct Renamer;

impl Renamer {
    fn pick(&self, name: &Name, taken: &HashSet<String>) -> Name {
        // A name `check` made (`$major9`) drops the counter that kept it
        // apart while typing: claiming it here keeps it apart again.
        let source = name.as_str();
        let source = match source
            .strip_prefix('$')
            .map(|rest| rest.trim_end_matches(|c: char| c.is_ascii_digit()))
        {
            Some(base) if !base.is_empty() => &source[..1 + base.len()],
            _ => source,
        };
        let want = to_camel(source);
        if !taken.contains(want.as_str()) {
            Name::new(want)
        } else {
            let mut n = 1;
            loop {
                let cand = format!("{want}${n}");
                if !taken.contains(cand.as_str()) {
                    break Name::new(cand);
                }
                n += 1;
            }
        }
    }

    /// Occupies the printed name in this JS block without changing lookups,
    /// so `let v = match { Some(v) => v }` can number the pattern (the
    /// assignment target is already live) while `let n = n + 1` still reads
    /// the outer `n`.
    fn reserve(&mut self, name: &Name, cx: &mut Cx) -> Name {
        let printed = self.pick(name, &cx.taken);
        cx.taken.insert(printed.as_str().to_string());
        printed
    }

    fn bind(&mut self, name: &Name, cx: &mut Cx) -> Name {
        let printed = self.reserve(name, cx);
        cx.env.insert(name.as_str().to_string(), printed.clone());
        printed
    }

    fn bind_let(&mut self, name: Name, value: Box<Expr>, cx: &mut Cx) -> (Name, Box<Expr>) {
        let printed = self.reserve(&name, cx);
        let value = self.boxed(value, cx);
        cx.env.insert(name.as_str().to_string(), printed.clone());
        (printed, value)
    }

    fn pattern(&mut self, p: Pattern, cx: &mut Cx) -> Pattern {
        match p {
            Pattern::Var(n) => Pattern::Var(self.bind(&n, cx)),
            Pattern::Variant { ty, variant, bind } => Pattern::Variant {
                ty,
                variant,
                bind: match bind {
                    VariantBind::Unit => VariantBind::Unit,
                    VariantBind::Tuple(ps) => {
                        VariantBind::Tuple(ps.into_iter().map(|p| self.pattern(p, cx)).collect())
                    }
                    VariantBind::Struct(ps) => VariantBind::Struct(
                        ps.into_iter()
                            .map(|(f, p)| (f, self.pattern(p, cx)))
                            .collect(),
                    ),
                },
            },
            Pattern::OptionSome(p) => Pattern::OptionSome(Box::new(self.pattern(*p, cx))),
            Pattern::ResultOk(p) => Pattern::ResultOk(Box::new(self.pattern(*p, cx))),
            Pattern::ResultErr(p) => Pattern::ResultErr(Box::new(self.pattern(*p, cx))),
            Pattern::Or(ps) => Pattern::Or(ps.into_iter().map(|p| self.pattern(p, cx)).collect()),
            Pattern::Tuple(ps) => {
                Pattern::Tuple(ps.into_iter().map(|p| self.pattern(p, cx)).collect())
            }
            other @ (Pattern::Wildcard
            | Pattern::Lit(_)
            | Pattern::Range { .. }
            | Pattern::OptionNone) => other,
        }
    }

    /// Rewrites in place, reusing the allocation.
    fn boxed(&mut self, mut e: Box<Expr>, cx: &Cx) -> Box<Expr> {
        *e = self.expr(std::mem::replace(&mut *e, Expr::Unreachable), cx);
        e
    }

    fn all(&mut self, xs: Vec<Expr>, cx: &Cx) -> Vec<Expr> {
        xs.into_iter().map(|x| self.expr(x, cx)).collect()
    }

    /// A statement in a sequence shares the JS block with what follows, so
    /// a `let` here is live for the rest of the sequence.
    fn stmt_binds(&mut self, e: Expr, cx: &mut Cx) -> Expr {
        match e {
            Expr::Let {
                name,
                mutable,
                ty,
                value,
                then,
            } => {
                let (name, value) = self.bind_let(name, value, cx);
                Expr::Let {
                    name,
                    mutable,
                    ty,
                    value,
                    then: Box::new(self.stmt_binds(*then, cx)),
                }
            }
            Expr::Seq { first, then } => Expr::Seq {
                first: Box::new(self.stmt_binds(*first, cx)),
                then: Box::new(self.stmt_binds(*then, cx)),
            },
            other => self.expr(other, cx),
        }
    }

    fn expr(&mut self, e: Expr, cx: &Cx) -> Expr {
        match e {
            Expr::At { .. } => unreachable!("`accept` removes positions before renaming"),
            // Not bound here: a function or a `const`.
            Expr::Var(n) => Expr::Var(cx.env.get(n.as_str()).cloned().unwrap_or_else(|| camel(&n))),
            Expr::Let {
                name,
                mutable,
                ty,
                value,
                then,
            } => {
                let mut inner = cx.clone();
                let (name, value) = self.bind_let(name, value, &mut inner);
                Expr::Let {
                    name,
                    mutable,
                    ty,
                    value,
                    then: Box::new(self.stmt_binds(*then, &mut inner)),
                }
            }
            Expr::For {
                var,
                ty,
                start,
                end,
                body,
            } => {
                let start = self.boxed(start, cx);
                let end = self.boxed(end, cx);
                let mut inner = cx.clone();
                let var = self.bind(&var, &mut inner);
                Expr::For {
                    var,
                    ty,
                    start,
                    end,
                    body: self.boxed(body, &inner),
                }
            }
            Expr::ForEach {
                var,
                over,
                source: string,
                body,
            } => {
                let string = self.boxed(string, cx);
                let mut inner = cx.clone();
                let var = self.bind(&var, &mut inner);
                Expr::ForEach {
                    var,
                    over,
                    source: string,
                    body: self.boxed(body, &inner),
                }
            }
            Expr::If { cond, then, else_ } => Expr::If {
                cond: self.boxed(cond, cx),
                then: self.boxed(then, cx),
                else_: self.boxed(else_, cx),
            },
            Expr::Match { scrutinee, arms } => Expr::Match {
                scrutinee: self.boxed(scrutinee, cx),
                arms: arms
                    .into_iter()
                    .map(|a| {
                        let mut inner = cx.clone();
                        let pattern = self.pattern(a.pattern, &mut inner);
                        Arm {
                            guard: None,
                            pattern,
                            body: self.stmt_binds(a.body, &mut inner),
                        }
                    })
                    .collect(),
            },
            Expr::Call {
                callee: Callee::Local(n),
                args,
            } => Expr::Call {
                callee: Callee::Local(cx.env.get(n.as_str()).cloned().unwrap_or_else(|| camel(&n))),
                args: self.all(args, cx),
            },
            Expr::Call { callee, args } => Expr::Call {
                callee: match callee {
                    Callee::Fn(n) => Callee::Fn(camel(&n)),
                    Callee::Method { ty, name } => Callee::Method {
                        ty,
                        name: camel(&name),
                    },
                    other => other,
                },
                args: self.all(args, cx),
            },
            Expr::Closure { params, ret, body } => {
                let mut inner = cx.clone();
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
            Expr::MethodCall {
                receiver,
                name,
                args,
            } => Expr::MethodCall {
                receiver: self.boxed(receiver, cx),
                name,
                args: self.all(args, cx),
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
                    Fields::Positional(xs) => Fields::Positional(self.all(xs, cx)),
                    Fields::Named(xs) => {
                        Fields::Named(xs.into_iter().map(|(n, x)| (n, self.expr(x, cx))).collect())
                    }
                },
                base: base.map(|b| self.boxed(b, cx)),
            },
            Expr::Field { base, name } => Expr::Field {
                base: self.boxed(base, cx),
                name,
            },
            Expr::Index { base, index } => Expr::Index {
                base: self.boxed(base, cx),
                index: self.boxed(index, cx),
            },
            Expr::Tuple(xs) => Expr::Tuple(self.all(xs, cx)),
            Expr::Array(xs) => Expr::Array(self.all(xs, cx)),
            Expr::Cast { expr, to } => Expr::Cast {
                expr: self.boxed(expr, cx),
                to,
            },
            Expr::While { cond, body } => Expr::While {
                cond: self.boxed(cond, cx),
                body: self.boxed(body, cx),
            },
            Expr::Break => Expr::Break,
            Expr::Continue => Expr::Continue,
            Expr::Binary { op, left, right } => Expr::Binary {
                op,
                left: self.boxed(left, cx),
                right: self.boxed(right, cx),
            },
            Expr::Unary { op, expr } => Expr::Unary {
                op,
                expr: self.boxed(expr, cx),
            },
            Expr::Return(v) => Expr::Return(self.boxed(v, cx)),
            Expr::Assign { name, value } => Expr::Assign {
                name: cx.env.get(name.as_str()).cloned().unwrap_or(name),
                value: self.boxed(value, cx),
            },
            Expr::Seq { first, then } => {
                let mut inner = cx.clone();
                let first = self.stmt_binds(*first, &mut inner);
                Expr::Seq {
                    first: Box::new(first),
                    then: Box::new(self.stmt_binds(*then, &mut inner)),
                }
            }
            Expr::Try { expr, on } => Expr::Try {
                expr: self.boxed(expr, cx),
                on,
            },
            Expr::Ignored { wrapper, expr } => Expr::Ignored {
                wrapper,
                expr: self.boxed(expr, cx),
            },
            other @ (Expr::Lit(_) | Expr::Unreachable) => other,
        }
    }
}
