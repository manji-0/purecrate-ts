//! `Option`'s and `Result`'s combinators, each a two-arm `match`.

use super::*;

/// `Option`'s combinators that `option_method` writes as a `match`, with
/// how many arguments each takes, in the order a message lists them.
pub(super) const OPTION_METHODS: [(&str, usize); 15] = [
    ("is_some_and", 1),
    ("is_none_or", 1),
    ("unwrap", 0),
    ("expect", 1),
    ("unwrap_or", 1),
    ("unwrap_or_else", 1),
    ("ok_or", 1),
    ("ok_or_else", 1),
    ("or", 1),
    ("or_else", 1),
    ("map", 1),
    ("map_or", 2),
    ("map_or_else", 2),
    ("and_then", 1),
    ("filter", 1),
];

/// `Result`'s methods that `result_method` writes as a `match`, with how
/// many arguments each takes, in the order a message lists them.
pub(super) const RESULT_METHODS: [(&str, usize); 13] = [
    ("ok", 0),
    ("err", 0),
    ("is_ok", 0),
    ("is_err", 0),
    ("is_ok_and", 1),
    ("is_err_and", 1),
    ("unwrap_or", 1),
    ("unwrap_or_else", 1),
    ("map", 1),
    ("map_err", 1),
    ("map_or", 2),
    ("and_then", 1),
    ("or_else", 1),
];

impl<'d, 'a> Typer<'d, 'a> {
    /// `Option`'s methods, each as the `match` std writes it. The typed
    /// receiver is bound to a fresh name first, so it is typed once however
    /// long the chain. An argument that is a value (`unwrap_or(d)`,
    /// `ok_or(e)`, `expect(m)`, `map_or(d, f)`'s `d`, `or(o)`) is evaluated
    /// before the `match`, whether or not the option is `Some`, as Rust
    /// does; a function (`map(f)`, `and_then(f)`, `unwrap_or_else(f)`, ..)
    /// runs only on the side that calls it. `None` when `name` is none of
    /// these.
    pub(super) fn option_method(
        &mut self,
        recv: Expr,
        inner: &Ty,
        name: &str,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
        if !OPTION_METHODS.contains(&(name, args.len())) {
            return None;
        }
        let what = format!("`Option::{name}`");
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
        let none = || Expr::Call { callee: Callee::OptionNone, args: vec![] };
        let wrap_some = |e: Expr| Expr::Call { callee: Callee::OptionSome, args: vec![e] };
        let rt = Ty::Option(Box::new(inner.clone()));
        let mut eager_let: Option<Bound> = None;
        let mut want = want.cloned();
        // The value argument, typed (and run) before the `match`. Bound to
        // a name (`lift` reads `ok_or(e)?` in that shape), but for
        // `expect`'s literal message.
        let mut eager = |me: &mut Self, arg: &Expr, hint: Option<Ty>| -> Expr {
            let (typed, ty) = me.expr(arg, hint.as_ref());
            if let (Expr::Lit(_), "expect") = (&typed, name) {
                return typed;
            }
            let name = me.fresh_name(&format!("{base}_or"));
            eager_let = Some((name.clone(), ty, typed));
            Expr::Var(name)
        };
        let rest = match name {
            "unwrap" => two(
                Pattern::Var(some.clone()),
                v(&some),
                panic_with(Expr::Lit(Lit::Str("called `Option::unwrap()` on a `None` value".into()))),
            ),
            "expect" => {
                let m = eager(self, &args[0], Some(Ty::Prim(Prim::Str)));
                two(Pattern::Var(some.clone()), v(&some), panic_with(m))
            }
            "unwrap_or" => {
                let d = eager(self, &args[0], Some(inner.clone()));
                two(Pattern::Var(some.clone()), v(&some), d)
            }
            "or" => {
                let o = eager(self, &args[0], Some(rt.clone()));
                two(Pattern::Var(some.clone()), wrap_some(v(&some)), o)
            }
            "ok_or" => {
                let hint = match want.as_ref().map(|w| self.norm(w)) {
                    Some(Ty::Result { err, .. }) => Some(*err),
                    _ => None,
                };
                let e = eager(self, &args[0], hint);
                if want.is_none() {
                    want = eager_let
                        .as_ref()
                        .and_then(|(_, t, _)| t.clone())
                        .or_else(|| self.expr_ty(&e))
                        .map(|t| Ty::result(inner.clone(), t));
                }
                two(
                    Pattern::Var(some.clone()),
                    Expr::Call { callee: Callee::ResultOk, args: vec![v(&some)] },
                    Expr::Call { callee: Callee::ResultErr, args: vec![e] },
                )
            }
            "unwrap_or_else" | "or_else" | "ok_or_else" => {
                let body = self.thunk(&what, &args[0])?;
                match name {
                    "unwrap_or_else" => two(Pattern::Var(some.clone()), v(&some), body),
                    "or_else" => two(Pattern::Var(some.clone()), wrap_some(v(&some)), body),
                    _ => {
                        if want.is_none() {
                            want = self.alone(&[], &body).map(|t| Ty::result(inner.clone(), t));
                        }
                        two(
                            Pattern::Var(some.clone()),
                            Expr::Call { callee: Callee::ResultOk, args: vec![v(&some)] },
                            Expr::Call { callee: Callee::ResultErr, args: vec![body] },
                        )
                    }
                }
            }
            "map" | "and_then" | "is_some_and" | "is_none_or" => {
                let (pattern, body) = self.arm_fn(&what, &args[0])?;
                match name {
                    "map" => two(pattern, wrap_some(body), none()),
                    "and_then" => two(pattern, body, none()),
                    "is_some_and" => two(pattern, body, Expr::Lit(Lit::Bool(false))),
                    _ => two(pattern, body, Expr::Lit(Lit::Bool(true))),
                }
            }
            // `filter(p)`: `p` reads the value, which is kept on `true`.
            "filter" => {
                let (pattern, body) = self.arm_fn(&what, &args[0])?;
                let kept = match &pattern {
                    Pattern::Var(x) => x.clone(),
                    _ => some.clone(),
                };
                let test =
                    Expr::If { cond: Box::new(body), then: Box::new(wrap_some(v(&kept))), else_: Box::new(none()) };
                two(Pattern::Var(kept), test, none())
            }
            "map_or" => {
                let d = eager(self, &args[0], want.clone());
                let (pattern, body) = self.arm_fn(&what, &args[1])?;
                if want.is_none() {
                    want = eager_let.as_ref().and_then(|(_, t, _)| t.clone()).or_else(|| self.expr_ty(&d));
                }
                two(pattern, body, d)
            }
            "map_or_else" => {
                let d = self.thunk(&what, &args[0])?;
                let (pattern, body) = self.arm_fn(&what, &args[1])?;
                if want.is_none() {
                    want = self.alone(&[], &d);
                }
                two(pattern, body, d)
            }
            _ => unreachable!("`OPTION_METHODS` lists every name"),
        };
        let lets: Vec<Bound> = std::iter::once((opt, Some(rt), recv)).chain(eager_let).collect();
        let (typed, t) = self.scoped(&lets, &rest, want.as_ref());
        if let Some(Ty::Option(o)) = t.as_ref().map(|t| self.norm(t)) {
            if matches!(self.norm(&o), Ty::Option(_)) {
                self.error(
                    Reason::NestedOption,
                    format!(
                    "{what} here makes an `Option<Option<_>>`, and both `None`s are `null` in TS; `match` the `Option`"
                ),
                );
                return Some((Expr::Lit(Lit::Unit), None));
            }
        }
        // `Result.ok(v)` alone leaves TS to infer the error type, which it
        // cannot; an annotated binding names it.
        let typed = match (&t, name) {
            (Some(ty), "ok_or" | "ok_or_else") => self.annotated(&format!("{base}_result"), ty, typed),
            _ => typed,
        };
        Some((wrap_lets(lets, typed), t))
    }

    /// `typed` bound to a fresh name annotated `ty`, then read.
    fn annotated(&mut self, base: &str, ty: &Ty, typed: Expr) -> Expr {
        let res = self.fresh_name(base);
        Expr::Let {
            name: res.clone(),
            mutable: false,
            ty: Some(ty.clone()),
            value: Box::new(typed),
            then: Box::new(Expr::Var(res)),
        }
    }

    /// The type of `body` typed on its own with `binds` in scope, its
    /// reports dropped: the type a combinator makes when the context gives
    /// none.
    fn alone(&mut self, binds: &[(&Pattern, &Ty)], body: &Expr) -> Option<Ty> {
        let before = self.out.len();
        let depth = self.scopes.len();
        for (p, t) in binds {
            self.bind(p, Some(t));
        }
        let (_, t) = self.expr(body, None);
        self.scopes.truncate(depth);
        self.out.truncate(before);
        t
    }

    /// The type of an already typed literal.
    fn expr_ty(&mut self, e: &Expr) -> Option<Ty> {
        match e {
            Expr::Lit(_) => self.alone(&[], e),
            _ => None,
        }
    }

    /// The `f` of `unwrap_or_else(f)` and the like, as the body that runs
    /// in its place: a closure of no parameters, or a function name called
    /// with none. `None` after reporting anything else.
    pub(super) fn thunk(&mut self, what: &str, f: &Expr) -> Option<Expr> {
        match f.unpositioned() {
            Expr::Closure { params, body, .. } if params.is_empty() => {
                if self.exits_reported(what, body, "write the `match`") {
                    return None;
                }
                Some((**body).clone())
            }
            Expr::Var(g) => Some(Expr::Call { callee: Callee::Fn(g.clone()), args: vec![] }),
            _ => {
                self.error(Reason::Closure, format!("{what} takes a closure `|| ..` or a function name in v0"));
                None
            }
        }
    }

    /// Whether `body`, of a closure passed to `what`, uses `?` or `return`,
    /// which would leave the enclosing function once the closure is inlined;
    /// reported, `tail` saying what to write instead.
    pub(super) fn exits_reported(&mut self, what: &str, body: &Expr, tail: &str) -> bool {
        let exits = body.exits();
        if exits {
            self.error(
                Reason::Closure,
                format!("a closure passed to {what} may not use `?` or `return` in v0; {tail}"),
            );
        }
        exits
    }

    /// The `f` of `Result::map(f)`, `Option::and_then(f)`, and the like as
    /// an arm's pattern and body: a closure of one parameter (`|_| ..` binds
    /// nothing), a function name, or a one-field tuple variant. `what` names
    /// the method. `None` after reporting anything else.
    pub(super) fn arm_fn(&mut self, what: &str, f: &Expr) -> Option<(Pattern, Expr)> {
        match f.unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 1 => {
                if self.exits_reported(what, body, "write the `match`") {
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
                self.error(
                    Reason::Closure,
                    format!("{what} takes a closure `|x| ..`, a function name, or a one-field tuple variant in v0"),
                );
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
        let ok_value = || Pattern::Var(value.clone());
        let two = |ok_pat: Pattern, ok_body: Expr, err_pat: Pattern, err_body: Expr| Expr::Match {
            scrutinee: Box::new(Expr::Var(r.clone())),
            arms: vec![
                Arm { guard: None, pattern: Pattern::ResultOk(Box::new(ok_pat)), body: ok_body },
                Arm { guard: None, pattern: Pattern::ResultErr(Box::new(err_pat)), body: err_body },
            ],
        };
        let call = |callee: Callee, args: Vec<Expr>| Expr::Call { callee, args };
        // `Ok(None)` and `Err(_)` would both be `null`, and `None` and
        // `Err(None)`.
        if args.is_empty() && matches!(name, "ok" | "err") {
            let (side, of) = match name {
                "ok" => (ok, format!("Result<{}, _>", show(ok))),
                _ => (err, format!("Result<_, {}>", show(err))),
            };
            if matches!(self.norm(side), Ty::Option(_)) {
                self.error(Reason::NestedOption, format!(
                    "`{name}()` of `{of}` is `Option<Option<_>>`, and both `None`s are `null` in TS; `match` the `Result`"
                ));
                return Some((Expr::Lit(Lit::Unit), None));
            }
        }
        if matches!(name, "unwrap" | "expect" | "unwrap_err" | "expect_err") {
            self.error(Reason::MethodCall, format!(
                "`Result::{name}`'s panic message prints the other side with `{{:?}}`, which v0 does not reproduce; `match` the `Result`, or use `?`, `unwrap_or`, or `ok()` and `Option::{}`",
                if name.starts_with("unwrap") { "unwrap" } else { "expect" }
            ));
            return Some((Expr::Lit(Lit::Unit), None));
        }
        if !RESULT_METHODS.contains(&(name, args.len())) {
            return None;
        }
        let what = format!("`Result::{name}`");
        let mut eager_let: Option<Bound> = None;
        let (rest, want) = match (name, args) {
            ("ok", []) => (
                two(
                    ok_value(),
                    call(Callee::OptionSome, vec![v(&value)]),
                    Pattern::Wildcard,
                    call(Callee::OptionNone, vec![]),
                ),
                want.cloned().or_else(|| Some(Ty::option(ok.clone()))),
            ),
            ("err", []) => {
                let e = self.fresh_name("error");
                (
                    two(
                        ok_value(),
                        call(Callee::OptionNone, vec![]),
                        Pattern::Var(e.clone()),
                        call(Callee::OptionSome, vec![v(&e)]),
                    ),
                    want.cloned().or_else(|| Some(Ty::option(err.clone()))),
                )
            }
            ("is_ok" | "is_err", []) => {
                let on_ok = name == "is_ok";
                let rest =
                    two(ok_value(), Expr::Lit(Lit::Bool(on_ok)), Pattern::Wildcard, Expr::Lit(Lit::Bool(!on_ok)));
                (rest, Some(Ty::bool()))
            }
            ("is_ok_and" | "is_err_and", [f]) => {
                let (pattern, body) = self.arm_fn(&what, f)?;
                let rest = if name == "is_ok_and" {
                    two(pattern, body, Pattern::Wildcard, Expr::Lit(Lit::Bool(false)))
                } else {
                    two(ok_value(), Expr::Lit(Lit::Bool(false)), pattern, body)
                };
                (rest, Some(Ty::bool()))
            }
            // `d` runs before the `match`, as Rust evaluates it.
            ("unwrap_or" | "map_or", [d, ..]) => {
                let (typed, ty) = self.expr(d, if name == "unwrap_or" { Some(ok) } else { want });
                let d = match typed {
                    Expr::Lit(_) => typed,
                    _ => {
                        let n = self.fresh_name("result_or");
                        eager_let = Some((n.clone(), ty.clone(), typed));
                        Expr::Var(n)
                    }
                };
                if name == "unwrap_or" {
                    (two(ok_value(), v(&value), Pattern::Wildcard, d), want.cloned().or_else(|| Some(ok.clone())))
                } else {
                    let (pattern, body) = self.arm_fn(&what, &args[1])?;
                    (two(pattern, body, Pattern::Wildcard, d), want.cloned().or(ty))
                }
            }
            ("unwrap_or_else", [f]) => {
                let (pattern, body) = self.arm_fn(&what, f)?;
                (two(ok_value(), v(&value), pattern, body), want.cloned().or_else(|| Some(ok.clone())))
            }
            ("and_then", [f]) => {
                let (pattern, body) = self.arm_fn(&what, f)?;
                let want = want.cloned().or_else(|| self.alone(&[(&pattern, ok)], &body));
                let e = self.fresh_name("error");
                (two(pattern, body, Pattern::Var(e.clone()), call(Callee::ResultErr, vec![v(&e)])), want)
            }
            ("or_else", [f]) => {
                let (pattern, body) = self.arm_fn(&what, f)?;
                let want = want.cloned().or_else(|| self.alone(&[(&pattern, err)], &body));
                (two(ok_value(), call(Callee::ResultOk, vec![v(&value)]), pattern, body), want)
            }
            ("map" | "map_err", [f]) => {
                let (pattern, body) = self.arm_fn(&what, f)?;
                let on_ok = name == "map";
                // With no type from the context, the new type is the body's.
                let want = want.cloned().or_else(|| {
                    let t = self.alone(&[(&pattern, if on_ok { ok } else { err })], &body);
                    t.map(|t| if on_ok { Ty::result(t, err.clone()) } else { Ty::result(ok.clone(), t) })
                });
                let rest = if on_ok {
                    let e = self.fresh_name("error");
                    let err_body = call(Callee::ResultErr, vec![v(&e)]);
                    two(pattern, call(Callee::ResultOk, vec![body]), Pattern::Var(e), err_body)
                } else {
                    two(
                        ok_value(),
                        call(Callee::ResultOk, vec![v(&value)]),
                        pattern,
                        call(Callee::ResultErr, vec![body]),
                    )
                };
                (rest, want)
            }
            _ => unreachable!("`RESULT_METHODS` lists every name"),
        };
        let rt = Ty::result(ok.clone(), err.clone());
        let lets: Vec<Bound> = std::iter::once((r, Some(rt), recv)).chain(eager_let).collect();
        let (typed, t) = self.scoped(&lets, &rest, want.as_ref());
        // `Result.ok(v)` alone leaves TS to infer the error type, which it
        // cannot; an annotated binding names it.
        let typed = match (&t, name) {
            (Some(ty), "map" | "map_err" | "and_then" | "or_else") => self.annotated("mapped", ty, typed),
            _ => typed,
        };
        Some((wrap_lets(lets, typed), t))
    }
}

/// A panic with `message`, a `&str`.
fn panic_with(message: Expr) -> Expr {
    Expr::Call { callee: Callee::Panic, args: vec![message] }
}
