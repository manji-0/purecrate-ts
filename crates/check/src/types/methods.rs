//! Method calls: the crate's own methods, the std allow-list, `Option`'s
//! combinators, `char` and `str` methods, widening, and `as`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// `x.m(args)` is `T::m(x, args)` for the crate's own `impl T`. The
    /// receiver is typed once, to find `T`, and the typed receiver is the
    /// call's first argument: typing it again would double the work at every
    /// link of a chain.
    pub(super) fn method_call(&mut self, receiver: &Expr, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        let before = self.out.len();
        let (recv, rt) = self.expr(receiver, None);
        if name.as_str() == "len" && args.is_empty() {
            if let Some(rt) = &rt {
                if matches!(self.norm(rt), Ty::Vec(_)) {
                    let e = Expr::Call {
                        callee: Callee::VecLen,
                        args: vec![recv],
                    };
                    return (e, self.expect(want, Some(Ty::Prim(Prim::Usize))));
                }
            }
        }
        if let Some(rt) = rt.as_ref().filter(|_| args.is_empty()) {
            let callee = match (self.norm(rt), name.as_str()) {
                (Ty::Vec(_), "is_empty") => Some(Callee::VecIsEmpty),
                (Ty::Option(_), "is_some") => Some(Callee::OptionIsSome),
                (Ty::Option(_), "is_none") => Some(Callee::OptionIsNone),
                _ => None,
            };
            if let Some(callee) = callee {
                let e = Expr::Call { callee, args: vec![recv] };
                return (e, self.expect(want, Some(Ty::bool())));
            }
        }
        if let Some(Ty::Option(inner)) = rt.as_ref().map(|t| self.norm(t)) {
            if let Some(typed) = self.option_method(recv.clone(), &inner, name.as_str(), args, want) {
                return typed;
            }
        }
        if name.as_str() == "as_bytes" && args.is_empty() {
            if let Some(rt) = &rt {
                if matches!(self.norm(rt), Ty::Prim(Prim::String | Prim::Str)) {
                    let e = Expr::Call {
                        callee: Callee::StrBytes,
                        args: vec![recv],
                    };
                    return (e, self.expect(want, Some(Ty::Vec(Box::new(Ty::Prim(Prim::U8))))));
                }
            }
        }
        if let Some(m) = CharMethod::from_name(name.as_str()) {
            if rt.as_ref().is_some_and(|t| self.norm(t) == Ty::Prim(Prim::Char)) {
                return self.char_method(m, recv, args, want);
            }
        }
        if let Some(m) = StrMethod::from_name(name.as_str()) {
            if let Some(rt) = &rt {
                let rt = self.norm(rt);
                // `str::as_str` is unstable; only `String` has one.
                if matches!(rt, Ty::Prim(Prim::String)) || (rt == Ty::Prim(Prim::Str) && m != StrMethod::AsStr) {
                    return self.str_method(m, recv, args, want);
                }
            }
        }
        let owner = rt.as_ref().and_then(|t| match self.norm(t) {
            Ty::Named(n) => {
                let params = self.defs.methods.get(&(n.as_str(), name.as_str())).map(|f| f.params.len());
                params.map(|p| (n, p))
            }
            _ => None,
        });
        match (owner, rt) {
            (Some((ty, params)), Some(rt)) => {
                if params != args.len() + 1 {
                    self.error(Reason::ConstructShape, format!(
                        "`{}.{}` takes {} argument(s) after the receiver, got {}",
                        ty.as_str(),
                        name.as_str(),
                        params.saturating_sub(1),
                        args.len()
                    ));
                }
                let (param_tys, ret) = self
                    .defs
                    .methods
                    .get(&(ty.as_str(), name.as_str()))
                    .map(|f| sig(f))
                    .unwrap_or_default();
                self.expect(param_tys.first(), Some(rt));
                let mut typed = vec![recv];
                for (a, p) in args.iter().zip(param_tys.iter().skip(1).map(Some).chain(std::iter::repeat(None))) {
                    typed.push(self.expr(a, p).0);
                }
                let e = Expr::Call {
                    callee: Callee::Method { ty, name: name.clone() },
                    args: typed,
                };
                (e, self.expect(want, ret))
            }
            (Some(_), None) => unreachable!("an owner is found only from a known receiver type"),
            (None, Some(rt)) => {
                let rt = self.norm(&rt);
                let message = match std_methods(&rt) {
                    None => format!(
                        "`{}` has no method `{}` in the crate's own `impl` blocks",
                        show(&rt),
                        name.as_str()
                    ),
                    Some(list) => format!(
                        "`.{}()` on `{}` is not on the std allow-list; allowed: {}",
                        name.as_str(),
                        show(&rt),
                        list
                    ),
                };
                self.out.push(Diagnostic::at(self.item, Reason::MethodCall, message).about(name.as_str()));
                (receiver.clone(), None)
            }
            (None, None) => {
                if self.out.len() == before {
                    self.out.push(
                        Diagnostic::at(self.item, Reason::NeedsAnnotation, format!(
                            "the receiver type of `.{}()` is not known here; annotate the binding",
                            name.as_str()
                        ))
                        .about(name.as_str()),
                    );
                }
                (receiver.clone(), None)
            }
        }
    }

    /// `e as T`: only a fieldless enum to an integer type that holds every
    /// discriminant, looked up in a table. Everything else keeps its old
    /// rejection.
    pub(super) fn cast(&mut self, inner: &Expr, to: &Ty, want: Option<&Ty>) -> Typed {
        let (e, t) = self.expr(inner, None);
        let target = match to {
            Ty::Prim(p) => p.int(),
            _ => None,
        };
        let enum_def = t.as_ref().and_then(|t| match self.norm(t) {
            Ty::Named(n) => self.defs.enums.get(n.as_str()).copied(),
            _ => None,
        });
        match (enum_def, target) {
            (Some(en), Some(it)) if crate::consts::is_fieldless(en) => {
                match crate::consts::discriminants(self.defs, en) {
                    Ok(table) => {
                        let (lo, hi) = it.bounds();
                        if let Some((v, d)) = table.iter().find(|(_, d)| !(lo..=hi).contains(d)) {
                            self.error(Reason::Cast, format!(
                                "`{} as {}` would not hold `{}::{}` = {d}; cast to a type that holds every discriminant",
                                en.name.as_str(),
                                it.as_str(),
                                en.name.as_str(),
                                v.as_str()
                            ));
                        }
                        // `E::A as T` is a constant.
                        let known = match inner.unpositioned() {
                            Expr::Construct { variant: Some(v), .. } => table.iter().find(|(n, _)| n == v).map(|(_, d)| *d),
                            _ => None,
                        };
                        let folded = match known {
                            Some(value) => Expr::Lit(Lit::Int { value, ty: Some(it) }),
                            None => Expr::Call {
                                callee: Callee::Discriminant { to: it, table },
                                args: vec![e],
                            },
                        };
                        (folded, self.expect(want, Some(to.clone())))
                    }
                    // Reported once, on the enum.
                    Err(_) => (e, None),
                }
            }
            _ => {
                let what = t.as_ref().map(show).unwrap_or_else(|| "?".into());
                self.error(Reason::Cast, format!(
                    "`{what} as {}` is not in v0: `as` reads only a fieldless enum's discriminant; widen integers with `T::from(x)`",
                    show(to)
                ));
                (e, None)
            }
        }
    }

    pub(super) fn fresh_name(&mut self, what: &str) -> Name {
        self.fresh += 1;
        Name::new(format!("${what}{}", self.fresh))
    }

    /// `unwrap_or`, `ok_or`, and `map` on an `Option`, as the `match` std
    /// writes them. The typed receiver is bound to a fresh name first, so it
    /// is typed once however long the chain. `unwrap_or(d)` and `ok_or(e)`
    /// evaluate their argument whether or not the option is `Some`, as Rust
    /// does; `map(f)` calls `f` only on `Some`. `None` when `name` is none of
    /// these.
    pub(super) fn option_method(&mut self, recv: Expr, inner: &Ty, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let [arg] = args else { return None };
        let opt = self.fresh_name("opt");
        let some = self.fresh_name("some");
        let v = |n: &Name| Expr::Var(n.clone());
        let two = |some_pat: Pattern, some_body: Expr, none_body: Expr| Expr::Match {
            scrutinee: Box::new(Expr::Var(opt.clone())),
            arms: vec![
                Arm { pattern: Pattern::OptionSome(Box::new(some_pat)), body: some_body },
                Arm { pattern: Pattern::OptionNone, body: none_body },
            ],
        };
        let mut eager_let: Option<(Name, Option<Ty>, Expr)> = None;
        let mut want = want.cloned();
        let rest = match name {
            "unwrap_or" | "ok_or" => {
                let eager = self.fresh_name("arg");
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
                        if leaves(body) {
                            self.error(Reason::Closure, "a closure passed to `Option::map` may not use `?` or `return` in v0; write the `match`".into());
                            return Some((recv, None));
                        }
                        (Pattern::Var(params[0].name.clone()), (**body).clone())
                    }
                    Expr::Var(f) => (
                        Pattern::Var(some.clone()),
                        Expr::Call { callee: Callee::Fn(f.clone()), args: vec![v(&some)] },
                    ),
                    _ => {
                        self.error(Reason::Closure, "`Option::map` takes a closure `|x| ..` or a function name in v0".into());
                        return Some((recv, None));
                    }
                };
                two(pattern, Expr::Call { callee: Callee::OptionSome, args: vec![body] }, Expr::Call { callee: Callee::OptionNone, args: vec![] })
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
                let res = self.fresh_name("res");
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
            Some((name, ty, value)) => Expr::Let {
                name,
                mutable: false,
                ty,
                value: Box::new(value),
                then: Box::new(typed),
            },
            None => typed,
        };
        let e = Expr::Let {
            name: opt,
            mutable: false,
            ty: Some(rt),
            value: Box::new(recv),
            then: Box::new(typed),
        };
        Some((e, t))
    }

    /// `c.m(args)` for an allow-listed `char` method.
    pub(super) fn char_method(&mut self, m: CharMethod, recv: Expr, args: &[Expr], want: Option<&Ty>) -> Typed {
        if args.len() != m.args() {
            self.error(Reason::ConstructShape, format!(
                "`char::{}` takes {} argument(s) after the receiver, got {}",
                m.name(),
                m.args(),
                args.len()
            ));
        }
        let (params, ret) = char_sig(m);
        let mut typed = vec![recv];
        for (a, p) in args.iter().zip(&params) {
            typed.push(self.expr(a, Some(p)).0);
        }
        let e = Expr::Call {
            callee: Callee::Char(m),
            args: typed,
        };
        (e, self.expect(want, Some(ret)))
    }

    /// `s.m(needles)` for an allow-listed `str` method. A needle must be a
    /// string: `char` and closure patterns are not in the allow-list.
    pub(super) fn str_method(&mut self, m: StrMethod, recv: Expr, args: &[Expr], want: Option<&Ty>) -> Typed {
        if args.len() != m.needles() {
            self.error(Reason::ConstructShape, format!(
                "`str::{}` takes {} argument(s) after the receiver, got {}",
                m.name(),
                m.needles(),
                args.len()
            ));
        }
        let mut typed = vec![recv];
        for a in args {
            let (e, t) = self.expr(a, None);
            match t.map(|t| self.norm(&t)) {
                Some(Ty::Prim(Prim::String | Prim::Str)) | Some(Ty::Never) | None => {}
                Some(other) => self.error(Reason::TypeMismatch, format!(
                    "`str::{}` takes a `&str` pattern in v0, found `{}`",
                    m.name(),
                    show(&other)
                )),
            }
            typed.push(e);
        }
        let ret = match m {
            StrMethod::Len => Ty::Prim(Prim::Usize),
            StrMethod::AsStr => Ty::Prim(Prim::Str),
            StrMethod::IsEmpty | StrMethod::StartsWith | StrMethod::EndsWith | StrMethod::Contains => Ty::bool(),
        };
        let e = Expr::Call {
            callee: Callee::Str(m),
            args: typed,
        };
        (e, self.expect(want, Some(ret)))
    }

    /// `to::from(x)`: the argument is typed on its own, then must widen to
    /// `to` without loss. Narrowing has no `From` in std and stays rejected.
    pub(super) fn int_from(&mut self, to: IntTy, args: &[Expr], want: Option<&Ty>) -> Typed {
        let typed: Vec<Typed> = args.iter().map(|a| self.expr(a, None)).collect();
        let arg = typed.first().and_then(|(_, t)| t.clone()).map(|t| self.norm(&t));
        if arg == Some(Ty::Prim(Prim::Char)) {
            if !matches!(to, IntTy::U32 | IntTy::U64) {
                self.error(Reason::NumericOp, format!(
                    "`{}::from` does not take `char`: std converts a `char` only to `u32`, `u64`, and `u128`",
                    to.as_str()
                ));
            }
            let e = Expr::Call {
                callee: Callee::CharCode(to),
                args: typed.into_iter().map(|(e, _)| e).collect(),
            };
            return (e, self.expect(want, Some(Ty::Prim(to.into()))));
        }
        let from = match typed.first().map(|(_, t)| t.clone()) {
            Some(Some(t)) if t != Ty::Never => match self.num(&t) {
                Some(Num::Int(f)) if f.widens_to(to) => Some(f),
                Some(Num::Int(f)) => {
                    self.error(Reason::NumericOp, format!(
                        "`{}::from` does not take `{}`: std has no lossless conversion between them",
                        to.as_str(),
                        f.as_str()
                    ));
                    None
                }
                _ => {
                    self.error(Reason::TypeMismatch, format!(
                        "`{}::from` takes an integer in v0, found `{}`",
                        to.as_str(),
                        show(&t)
                    ));
                    None
                }
            },
            _ => None,
        };
        let e = Expr::Call {
            callee: Callee::IntFrom { from, to },
            args: typed.into_iter().map(|(e, _)| e).collect(),
        };
        (e, self.expect(want, Some(Ty::Prim(to.into()))))
    }
}

/// A `?` or `return` that would leave a closure (not one nested in it).
pub(super) fn leaves(expr: &Expr) -> bool {
    match expr {
        Expr::Try { .. } | Expr::Return(_) => true,
        Expr::Closure { .. } => false,
        other => other.children().into_iter().any(leaves),
    }
}

/// The allow-listed methods on a std receiver, for a rejection message;
/// `None` for the crate's own types, whose methods are its `impl` blocks.
pub(super) fn std_methods(ty: &Ty) -> Option<String> {
    let names: Vec<&str> = match ty {
        Ty::Named(_) => return None,
        Ty::Prim(Prim::String) => StrMethod::ALL.iter().map(|m| m.name()).chain(["as_bytes"]).collect(),
        Ty::Prim(Prim::Str) => StrMethod::ALL
            .iter()
            .filter(|m| **m != StrMethod::AsStr)
            .map(|m| m.name())
            .chain(["as_bytes"])
            .collect(),
        Ty::Prim(Prim::Char) => CharMethod::ALL.iter().map(|m| m.name()).collect(),
        Ty::Vec(_) => vec!["len", "is_empty", "indexing `xs[i]`"],
        Ty::Option(_) => vec!["is_some", "is_none", "unwrap_or", "ok_or", "map"],
        _ => vec![],
    };
    Some(match (ty, names.is_empty()) {
        (Ty::Prim(Prim::U8), _) => "none; for `u8::is_ascii_*` use `matches!(b, b'0'..=b'9')` or `char::from(b)`".into(),
        (_, true) => "none; use operators, `match`, or `T::from`".into(),
        (_, false) => names.iter().map(|n| if n.contains(' ') { n.to_string() } else { format!("`{n}`") }).collect::<Vec<_>>().join(", "),
    })
}

/// The parameters after the receiver, and the result, of a `char` method.
pub(super) fn char_sig(m: CharMethod) -> (Vec<Ty>, Ty) {
    let char_ = || Ty::Prim(Prim::Char);
    let u32_ = || Ty::Prim(Prim::U32);
    match m {
        CharMethod::ToAsciiLowercase | CharMethod::ToAsciiUppercase => (Vec::new(), char_()),
        CharMethod::EqIgnoreAsciiCase => (vec![char_()], Ty::bool()),
        CharMethod::LenUtf8 => (Vec::new(), Ty::Prim(Prim::Usize)),
        CharMethod::IsDigit => (vec![u32_()], Ty::bool()),
        CharMethod::ToDigit => (vec![u32_()], Ty::option(u32_())),
        _ => (Vec::new(), Ty::bool()),
    }
}
