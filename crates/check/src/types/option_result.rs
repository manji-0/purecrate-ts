//! `Option`'s and `Result`'s combinators, each a two-arm `match`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// `unwrap_or`, `ok_or`, and `map` on an `Option`, as the `match` std
    /// writes them. The typed receiver is bound to a fresh name first, so it
    /// is typed once however long the chain. `unwrap_or(d)` and `ok_or(e)`
    /// evaluate their argument whether or not the option is `Some`, as Rust
    /// does; `map(f)` calls `f` only on `Some`. `None` when `name` is none of
    /// these.
    pub(super) fn option_method(
        &mut self,
        recv: Expr,
        inner: &Ty,
        name: &str,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
        let [arg] = args else { return None };
        // Named after the receiver when it is a variable or a field
        // (`$major`, `$majorOr`), so the printed code says what it holds.
        let base = match recv.unpositioned() {
            Expr::Var(n) | Expr::Field { name: n, .. } if !n.as_str().starts_with(['$', '[']) && n.as_str() != "0" => {
                n.as_str().to_string()
            }
            _ => "opt".to_string(),
        };
        let opt = self.fresh_name(&base);
        let some = self.fresh_name("some");
        let v = |n: &Name| Expr::Var(n.clone());
        let two = |some_pat: Pattern, some_body: Expr, none_body: Expr| Expr::Match {
            scrutinee: Box::new(Expr::Var(opt.clone())),
            arms: vec![
                Arm { guard: None, pattern: Pattern::OptionSome(Box::new(some_pat)), body: some_body },
                Arm { guard: None, pattern: Pattern::OptionNone, body: none_body },
            ],
        };
        let mut eager_let: Option<(Name, Option<Ty>, Expr)> = None;
        let mut want = want.cloned();
        let rest = match name {
            "unwrap_or" | "ok_or" => {
                let eager = self.fresh_name(&format!("{base}_or"));
                let hint = if name == "unwrap_or" {
                    Some(inner.clone())
                } else {
                    match want.as_ref().map(|w| self.norm(w)) {
                        Some(Ty::Result { err, .. }) => Some(*err),
                        _ => None,
                    }
                };
                // The argument is typed (and runs) before the match.
                let (arg_typed, arg_ty) = self.expr(arg, hint.as_ref());
                let (some_body, none_body) = if name == "unwrap_or" {
                    (v(&some), v(&eager))
                } else {
                    if want.is_none() {
                        want = arg_ty.clone().map(|e| Ty::result(inner.clone(), e));
                    }
                    (
                        Expr::Call { callee: Callee::ResultOk, args: vec![v(&some)] },
                        Expr::Call { callee: Callee::ResultErr, args: vec![v(&eager)] },
                    )
                };
                eager_let = Some((eager, arg_ty, arg_typed));
                two(Pattern::Var(some.clone()), some_body, none_body)
            }
            "map" => {
                let (pattern, body) = match arg.unpositioned() {
                    Expr::Closure { params, body, .. } if params.len() == 1 => {
                        if body.exits() {
                            self.error(Reason::Closure, "a closure passed to `Option::map` may not use `?` or `return` in v0; write the `match`".into());
                            return Some((recv, None));
                        }
                        (Pattern::Var(params[0].name.clone()), (**body).clone())
                    }
                    Expr::Var(f) => {
                        (Pattern::Var(some.clone()), Expr::Call { callee: Callee::Fn(f.clone()), args: vec![v(&some)] })
                    }
                    e if self.variant_fn(e).is_some() => {
                        let build = self.variant_fn(e)?;
                        (Pattern::Var(some.clone()), build(v(&some)))
                    }
                    _ => {
                        self.error(Reason::Closure, "`Option::map` takes a closure `|x| ..`, a function name, or a one-field tuple variant in v0".into());
                        return Some((recv, None));
                    }
                };
                two(
                    pattern,
                    Expr::Call { callee: Callee::OptionSome, args: vec![body] },
                    Expr::Call { callee: Callee::OptionNone, args: vec![] },
                )
            }
            _ => return None,
        };
        let rt = Ty::Option(Box::new(inner.clone()));
        self.scopes.push((opt.as_str().to_string(), Some(rt.clone())));
        if let Some((eager, ty, _)) = &eager_let {
            self.scopes.push((eager.as_str().to_string(), ty.clone()));
        }
        let (typed, t) = self.expr(&rest, want.as_ref());
        if eager_let.is_some() {
            self.scopes.pop();
        }
        self.scopes.pop();
        // `Result.ok(v)` alone leaves TS to infer the error type, which it
        // cannot; an annotated binding names it.
        let typed = match (&t, name) {
            (Some(ty), "ok_or") => {
                let res = self.fresh_name(&format!("{base}_result"));
                Expr::Let {
                    name: res.clone(),
                    mutable: false,
                    ty: Some(ty.clone()),
                    value: Box::new(typed),
                    then: Box::new(Expr::Var(res)),
                }
            }
            _ => typed,
        };
        let typed = match eager_let {
            Some((name, ty, value)) => {
                Expr::Let { name, mutable: false, ty, value: Box::new(value), then: Box::new(typed) }
            }
            None => typed,
        };
        let e = Expr::Let { name: opt, mutable: false, ty: Some(rt), value: Box::new(recv), then: Box::new(typed) };
        Some((e, t))
    }

    /// The `f` of `Result::map(f)` or `map_err(f)` as an arm's pattern and
    /// body: a closure of one parameter (`|_| ..` binds nothing), a function
    /// name, or a one-field tuple variant. `None` after reporting anything
    /// else.
    pub(super) fn arm_fn(&mut self, method: &str, f: &Expr) -> Option<(Pattern, Expr)> {
        match f.unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 1 => {
                if body.exits() {
                    self.error(Reason::Closure, format!("a closure passed to `Result::{method}` may not use `?` or `return` in v0; write the `match`"));
                    return None;
                }
                let p = &params[0];
                let pattern = if p.name.as_str() == "_" { Pattern::Wildcard } else { Pattern::Var(p.name.clone()) };
                Some((pattern, (**body).clone()))
            }
            Expr::Var(g) => {
                let x = self.fresh_name("x");
                Some((Pattern::Var(x.clone()), Expr::Call { callee: Callee::Fn(g.clone()), args: vec![Expr::Var(x)] }))
            }
            e if self.variant_fn(e).is_some() => {
                let build = self.variant_fn(e)?;
                let x = self.fresh_name("x");
                Some((Pattern::Var(x.clone()), build(Expr::Var(x))))
            }
            _ => {
                self.error(Reason::Closure, format!("`Result::{method}` takes a closure `|x| ..`, a function name, or a one-field tuple variant in v0"));
                None
            }
        }
    }

    /// `ok()`, `map(f)`, and `map_err(f)` on a `Result`, as the `match` std
    /// writes them; `f` runs only on `Ok` or only on `Err`. The receiver is bound to a fresh name
    /// first. `None` when `name` is neither.
    pub(super) fn result_method(
        &mut self,
        recv: Expr,
        ok: &Ty,
        err: &Ty,
        name: &str,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
        let r = self.fresh_name("result");
        let value = self.fresh_name("value");
        let v = |n: &Name| Expr::Var(n.clone());
        let two = |ok_body: Expr, err_pat: Pattern, err_body: Expr| Expr::Match {
            scrutinee: Box::new(Expr::Var(r.clone())),
            arms: vec![
                Arm { guard: None, pattern: Pattern::ResultOk(Box::new(Pattern::Var(value.clone()))), body: ok_body },
                Arm { guard: None, pattern: Pattern::ResultErr(Box::new(err_pat)), body: err_body },
            ],
        };
        // `Ok(None)` and `Err(_)` would both be `null`.
        if name == "ok" && args.is_empty() && matches!(self.norm(ok), Ty::Option(_)) {
            self.error(Reason::NestedOption, format!(
                "`ok()` of `Result<{}, _>` is `Option<Option<_>>`, and both `None`s are `null` in TS; `match` the `Result`",
                show(ok)
            ));
            return Some((Expr::Lit(Lit::Unit), None));
        }
        let (rest, want) = match (name, args) {
            ("ok", []) => (
                two(
                    Expr::Call { callee: Callee::OptionSome, args: vec![v(&value)] },
                    Pattern::Wildcard,
                    Expr::Call { callee: Callee::OptionNone, args: vec![] },
                ),
                want.cloned().or_else(|| Some(Ty::option(ok.clone()))),
            ),
            ("map" | "map_err", [f]) => {
                let (pattern, body) = self.arm_fn(name, f)?;
                let on_ok = name == "map";
                // With no type from the context, the new type is the body's:
                // typed once on its own here, its reports dropped.
                let want = want.cloned().or_else(|| {
                    let before = self.out.len();
                    let depth = self.scopes.len();
                    self.bind(&pattern, Some(if on_ok { ok } else { err }));
                    let (_, t) = self.expr(&body, None);
                    self.scopes.truncate(depth);
                    self.out.truncate(before);
                    t.map(|t| if on_ok { Ty::result(t, err.clone()) } else { Ty::result(ok.clone(), t) })
                });
                let rest = if on_ok {
                    let e = self.fresh_name("error");
                    Expr::Match {
                        scrutinee: Box::new(Expr::Var(r.clone())),
                        arms: vec![
                            Arm {
                                guard: None,
                                pattern: Pattern::ResultOk(Box::new(pattern)),
                                body: Expr::Call { callee: Callee::ResultOk, args: vec![body] },
                            },
                            Arm {
                                guard: None,
                                pattern: Pattern::ResultErr(Box::new(Pattern::Var(e.clone()))),
                                body: Expr::Call { callee: Callee::ResultErr, args: vec![v(&e)] },
                            },
                        ],
                    }
                } else {
                    two(
                        Expr::Call { callee: Callee::ResultOk, args: vec![v(&value)] },
                        pattern,
                        Expr::Call { callee: Callee::ResultErr, args: vec![body] },
                    )
                };
                (rest, want)
            }
            _ => return None,
        };
        let rt = Ty::result(ok.clone(), err.clone());
        self.scopes.push((r.as_str().to_string(), Some(rt.clone())));
        let (typed, t) = self.expr(&rest, want.as_ref());
        self.scopes.pop();
        // `Result.ok(v)` alone leaves TS to infer the error type, which it
        // cannot; an annotated binding names it.
        let typed = match (&t, name) {
            (Some(ty), "map" | "map_err") => {
                let res = self.fresh_name("mapped");
                Expr::Let {
                    name: res.clone(),
                    mutable: false,
                    ty: Some(ty.clone()),
                    value: Box::new(typed),
                    then: Box::new(Expr::Var(res)),
                }
            }
            _ => typed,
        };
        let e = Expr::Let { name: r, mutable: false, ty: Some(rt), value: Box::new(recv), then: Box::new(typed) };
        Some((e, t))
    }
}
