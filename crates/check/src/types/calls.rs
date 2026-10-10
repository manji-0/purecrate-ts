//! Calls, closures, struct and variant construction, and `?`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// The items `over` walks in a value of type `ty`.
    pub(super) fn walked(&self, over: purecrate_ir::Over, ty: &Ty) -> Option<Ty> {
        match (over, self.norm(ty)) {
            (purecrate_ir::Over::Chars, Ty::Prim(Prim::String | Prim::Str)) => Some(Ty::Prim(Prim::Char)),
            (purecrate_ir::Over::Bytes, Ty::Prim(Prim::String | Prim::Str)) => Some(Ty::Prim(Prim::U8)),
            (purecrate_ir::Over::Items, Ty::Vec(t)) => Some(*t),
            _ => None,
        }
    }

    /// Rust's `?` also converts the error with `From`; v0 has no traits, so
    /// the error type must already be the function's.
    pub(super) fn try_(&mut self, inner: &Expr, want: Option<&Ty>) -> Typed {
        if let Some(typed) = self.map_err_try(inner, want) {
            return typed;
        }
        let (inner, it) = self.expr(inner, None);
        let ret = self.norm(&self.ret);
        let (on, t) = match (it.map(|t| self.norm(&t)), &ret) {
            (Some(Ty::Result { ok, err }), Ty::Result { err: ret_err, .. }) => {
                if !self.same(&err, ret_err) {
                    self.error(
                        Reason::TryConversion,
                        format!(
                            "`?` on an error of type `{}` in a function returning `{}`; \
                         v0 has no `From` conversion, so the error types must match",
                            show(&err),
                            show(&ret)
                        ),
                    );
                }
                (Some(TryOn::Result), Some(*ok))
            }
            (Some(Ty::Option(inner_ty)), Ty::Option(_)) => (Some(TryOn::Option), Some(*inner_ty)),
            (Some(t @ (Ty::Result { .. } | Ty::Option(_))), _) => {
                self.error(
                    Reason::TryConversion,
                    format!("`?` on `{}` in a function returning `{}`", show(&t), show(&ret)),
                );
                (None, None)
            }
            (Some(t), _) => {
                self.error(Reason::TypeMismatch, format!("`?` needs a `Result` or `Option`, found `{}`", show(&t)));
                (None, None)
            }
            (None, _) => (None, None),
        };
        let e = Expr::Try { expr: Box::new(inner), on };
        (e, self.expect(want, t))
    }

    /// `r.map_err(f)?` is `match r { Ok(v) => v, Err(e) => return Err(f(e)) }`:
    /// the mapped `Result` is never built. `None` when `inner` is not a
    /// `map_err` on a `Result`.
    fn map_err_try(&mut self, inner: &Expr, want: Option<&Ty>) -> Option<Typed> {
        let Expr::MethodCall { receiver, name, args } = inner.unpositioned() else { return None };
        let [f] = args.as_slice() else { return None };
        if name.as_str() != "map_err" {
            return None;
        }
        let before = self.out.len();
        let (recv, rt) = self.expr(receiver, None);
        let Some(Ty::Result { ok, err }) = rt.as_ref().map(|t| self.norm(t)) else {
            // Typed again, reported once, on the general path.
            self.out.truncate(before);
            return None;
        };
        let (pattern, body) = self.arm_fn("map_err", f)?;
        let r = self.fresh_name("result");
        let value = self.fresh_name("value");
        let rest = Expr::Match {
            scrutinee: Box::new(Expr::Var(r.clone())),
            arms: vec![
                Arm {
                    guard: None,
                    pattern: Pattern::ResultOk(Box::new(Pattern::Var(value.clone()))),
                    body: Expr::Var(value),
                },
                Arm {
                    guard: None,
                    pattern: Pattern::ResultErr(Box::new(pattern)),
                    body: Expr::Return(Box::new(Expr::Call { callee: Callee::ResultErr, args: vec![body] })),
                },
            ],
        };
        let rt = Ty::result(*ok.clone(), *err);
        self.scopes.push((r.as_str().to_string(), Some(rt.clone())));
        let (typed, t) = self.expr(&rest, want.or(Some(&*ok)));
        self.scopes.pop();
        let e = Expr::Let { name: r, mutable: false, ty: Some(rt), value: Box::new(recv), then: Box::new(typed) };
        Some((e, t))
    }

    pub(super) fn is_local(&self, name: &str) -> bool {
        self.scopes.iter().any(|(n, _)| n == name)
    }

    /// Parameter types come from annotations or, failing that, from the
    /// closure type the context expects. The body is typed with the closure's
    /// return type in place of the function's, since `?` and `return` there
    /// leave the closure.
    pub(super) fn closure(
        &mut self,
        params: &[ClosureParam],
        ret: Option<&Ty>,
        body: &Expr,
        want: Option<&Ty>,
    ) -> Typed {
        let expected = match want.map(|w| self.norm(w)) {
            Some(Ty::Fn { params: ps, ret }) if ps.len() == params.len() => Some((ps, *ret)),
            _ => None,
        };
        let param_tys: Vec<Option<Ty>> = params
            .iter()
            .enumerate()
            .map(|(i, p)| p.ty.clone().or_else(|| expected.as_ref().map(|(ps, _)| ps[i].clone())))
            .collect();
        for (p, t) in params.iter().zip(&param_tys) {
            if t.is_none() {
                self.error(
                    Reason::NeedsAnnotation,
                    format!("the type of closure parameter `{0}` is not known here; write `|{0}: T|`", p.name.as_str()),
                );
            }
        }
        let ret = ret.cloned().or_else(|| expected.map(|(_, r)| r));
        if ret.is_none() && body.exits() {
            self.error(
                Reason::NeedsAnnotation,
                "a closure with `?` or `return` needs its return type; write `|..| -> T { .. }`".to_string(),
            );
        }
        let outer_ret = std::mem::replace(&mut self.ret, ret.clone().unwrap_or(Ty::Never));
        let depth = self.scopes.len();
        self.scopes.extend(params.iter().zip(&param_tys).map(|(p, t)| (p.name.as_str().to_string(), t.clone())));
        let (body, bt) = self.expr(body, ret.as_ref());
        self.scopes.truncate(depth);
        self.ret = outer_ret;
        let ret = ret.or(bt);
        let t = match (param_tys.iter().cloned().collect::<Option<Vec<Ty>>>(), &ret) {
            (Some(params), Some(r)) => Some(Ty::Fn { params, ret: Box::new(r.clone()) }),
            _ => None,
        };
        let e = Expr::Closure {
            params: params.iter().zip(param_tys).map(|(p, ty)| ClosureParam { name: p.name.clone(), ty }).collect(),
            ret,
            body: Box::new(body),
        };
        (e, self.expect(want, t))
    }

    pub(super) fn local_call(&mut self, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        let (params, ret) = match self.lookup(name.as_str()).map(|t| self.norm(&t)) {
            Some(Ty::Fn { params, ret }) => {
                if params.len() != args.len() {
                    self.error(
                        Reason::ConstructShape,
                        format!("closure `{}` takes {} argument(s), got {}", name.as_str(), params.len(), args.len()),
                    );
                }
                (params, Some(*ret))
            }
            Some(other) => {
                self.error(Reason::TypeMismatch, format!("`{}` is a `{}`, not a closure", name.as_str(), show(&other)));
                (Vec::new(), None)
            }
            None => (Vec::new(), None),
        };
        let args = args
            .iter()
            .zip(params.iter().map(Some).chain(std::iter::repeat(None)))
            .map(|(a, p)| self.expr(a, p).0)
            .collect();
        let e = Expr::Call { callee: Callee::Local(name.clone()), args };
        (e, self.expect(want, ret))
    }

    pub(super) fn call(&mut self, callee: &Callee, args: &[Expr], want: Option<&Ty>) -> Typed {
        if let Callee::Fn(n) | Callee::Local(n) = callee {
            if self.is_local(n.as_str()) {
                return self.local_call(n, args, want);
            }
        }
        let typed_args = |me: &mut Self, params: Vec<Ty>| -> Vec<Expr> {
            args.iter()
                .zip(params.iter().map(Some).chain(std::iter::repeat(None)))
                .map(|(a, p)| me.expr(a, p).0)
                .collect()
        };
        let (args, t) = match callee {
            Callee::Local(_) => (typed_args(self, Vec::new()), None),
            Callee::Fn(n) => {
                let sig = self.defs.free_fns.get(n.as_str()).map(|f| sig(f));
                let (params, ret) = sig.unwrap_or_default();
                (typed_args(self, params), ret)
            }
            Callee::Method { ty, name } => {
                let sig = self.defs.methods.get(&(ty.as_str(), name.as_str())).map(|f| sig(f));
                let (params, ret) = sig.unwrap_or_default();
                (typed_args(self, params), ret)
            }
            Callee::StructNew(n) => {
                let params = self
                    .defs
                    .structs
                    .get(n.as_str())
                    .map(|s| s.fields.iter().map(|f| f.ty.clone()).collect())
                    .unwrap_or_default();
                (typed_args(self, params), Some(Ty::Named(n.clone())))
            }
            Callee::Variant { ty, variant } => {
                let params = self
                    .defs
                    .enums
                    .get(ty.as_str())
                    .and_then(|e| e.variants.iter().find(|v| v.name == *variant))
                    .and_then(|v| match &v.fields {
                        VariantFields::Tuple(tys) => Some(tys.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                (typed_args(self, params), Some(Ty::Named(ty.clone())))
            }
            Callee::ResultOk | Callee::ResultErr => {
                let expected = match want.map(|w| self.norm(w)) {
                    Some(Ty::Result { ok, err }) => Some((*ok, *err)),
                    _ => None,
                };
                let inner =
                    expected.as_ref().map(
                        |(ok, err)| {
                            if matches!(callee, Callee::ResultOk) {
                                ok.clone()
                            } else {
                                err.clone()
                            }
                        },
                    );
                let args = typed_args(self, inner.into_iter().collect());
                (args, expected.map(|_| want.cloned().expect("checked")))
            }
            Callee::OptionSome => {
                let expected = match want.map(|w| self.norm(w)) {
                    Some(Ty::Option(inner)) => Some(*inner),
                    _ => None,
                };
                match expected {
                    Some(inner) => (typed_args(self, vec![inner]), want.cloned()),
                    None => {
                        let typed: Vec<Typed> = args.iter().map(|a| self.expr(a, None)).collect();
                        let inner = typed.first().and_then(|(_, t)| t.clone());
                        if inner.as_ref().is_some_and(|t| matches!(self.norm(t), Ty::Option(_))) {
                            self.error(
                                Reason::NestedOption,
                                format!(
                                "`Some` of `{}` is `Option<Option<_>>`, and both `None`s are `null` in TS; use an enum",
                                show(inner.as_ref().expect("checked"))
                            ),
                            );
                        }
                        let t = inner.map(Ty::option);
                        (typed.into_iter().map(|(e, _)| e).collect(), t)
                    }
                }
            }
            Callee::OptionNone => {
                let t = match want.map(|w| self.norm(w)) {
                    Some(Ty::Option(_)) => want.cloned(),
                    _ => None,
                };
                (typed_args(self, Vec::new()), t)
            }
            Callee::Int { ty, op: op @ IntOp::Method(_) } => {
                let mut args = args.to_vec();
                if let Some(recv) = args.first_mut() {
                    *recv = self.expr(recv, Some(&Ty::Prim((*ty).into()))).0;
                }
                let (e, t) = self.int_call(*ty, *op, Expr::Call { callee: callee.clone(), args });
                return (e, self.expect(want, t));
            }
            Callee::Int { ty, op } => {
                let t = Ty::Prim((*ty).into());
                // A shift amount keeps its own type.
                let n = if op.is_shift() { 1 } else { op.arity() };
                (typed_args(self, vec![t.clone(); n]), Some(t))
            }
            Callee::Fround => (typed_args(self, vec![Ty::Prim(Prim::F64)]), Some(Ty::Prim(Prim::F32))),
            Callee::AsFloat(ft) => {
                let t = Ty::Prim((*ft).into());
                (typed_args(self, vec![t.clone()]), Some(t))
            }
            Callee::Float { ty, m } => {
                let t = Ty::Prim((*ty).into());
                (typed_args(self, vec![t.clone()]), Some(if m.is_test() { Ty::bool() } else { t }))
            }
            Callee::FloatConst { ty, .. } => (typed_args(self, Vec::new()), Some(Ty::Prim((*ty).into()))),
            Callee::FloatFrom(to) => return self.float_from(*to, args, want),
            Callee::FloatToInt { from, to } => {
                (typed_args(self, vec![Ty::Prim((*from).into())]), Some(Ty::Prim((*to).into())))
            }
            Callee::IntToFloat { from, to } => {
                (typed_args(self, vec![Ty::Prim((*from).into())]), Some(Ty::Prim((*to).into())))
            }
            Callee::FloatToFloat { to } => {
                let from = if *to == FloatTy::F64 { FloatTy::F32 } else { FloatTy::F64 };
                (typed_args(self, vec![Ty::Prim(from.into())]), Some(Ty::Prim((*to).into())))
            }
            Callee::VecLen => (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::Prim(Prim::Usize))),
            // Written only by `binary`, typed.
            Callee::DeepEq => (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::bool())),
            // Written only by `method_call`, typed.
            Callee::VecPush | Callee::VecInsert => {
                (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::Prim(Prim::Unit)))
            }
            Callee::VecRemove => {
                let typed: Vec<Typed> = args.iter().map(|a| self.expr(a, None)).collect();
                let item = match typed[0].1.as_ref().map(|t| self.norm(t)) {
                    Some(Ty::Vec(item)) => Some(*item),
                    _ => None,
                };
                (typed.into_iter().map(|(e, _)| e).collect(), item)
            }
            // Written by `syntax` for `v[i] = x`: only a local's own array.
            Callee::VecSet => {
                let [v, at, value] = args else { unreachable!("`syntax` writes three arguments") };
                let (v, vt) = self.expr(v, None);
                let item = match vt.as_ref().map(|t| self.norm(t)) {
                    Some(Ty::Vec(item)) => Some(*item),
                    Some(other) => {
                        self.error(Reason::Index, format!("cannot index `{}`", show(&other)));
                        None
                    }
                    None => None,
                };
                if !matches!(v.unpositioned(), Expr::Var(_)) {
                    self.error(Reason::PlaceAssign, "`v[i] = x` writes a local `let mut v: Vec<T>` in v0, not a field or an element: build the new `Vec` and put it in a new value (design/02 §3.1)".to_string());
                }
                let (at, _) = self.expr(at, Some(&Ty::Prim(Prim::Usize)));
                let (value, _) = self.expr(value, item.as_ref());
                (vec![v, at, value], Some(Ty::Prim(Prim::Unit)))
            }
            Callee::VecIsEmpty | Callee::OptionIsSome | Callee::OptionIsNone => {
                (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::bool()))
            }
            // Written only by `binary`, on strings already typed.
            Callee::StrCmp => {
                (typed_args(self, vec![Ty::Prim(Prim::Str), Ty::Prim(Prim::Str)]), Some(Ty::Prim(Prim::I32)))
            }
            // Written only by `sequence`, typed: a `Vec` of what the stage gives.
            Callee::IterMap { .. } | Callee::IterFilter { .. } => {
                let typed: Vec<Typed> = args.iter().map(|a| self.expr(a, None)).collect();
                let item = match (callee, typed.get(1).and_then(|(_, t)| t.clone())) {
                    (Callee::IterMap { .. }, Some(Ty::Fn { ret, .. })) => Some(*ret),
                    (Callee::IterFilter { over }, _) => typed[0].1.as_ref().and_then(|t| self.walked(*over, t)),
                    _ => None,
                };
                (typed.into_iter().map(|(e, _)| e).collect(), item.map(|t| Ty::Vec(Box::new(t))))
            }
            // Written only by `consume`, typed.
            Callee::Consume { method, .. } => (
                args.iter().map(|a| self.expr(a, None).0).collect(),
                Some(match method {
                    purecrate_ir::Consume::All | purecrate_ir::Consume::Any => Ty::bool(),
                    purecrate_ir::Consume::Position => Ty::option(Ty::Prim(Prim::Usize)),
                    purecrate_ir::Consume::Count => Ty::Prim(Prim::Usize),
                    purecrate_ir::Consume::Sum(int) => Ty::Prim(Prim::from(*int)),
                }),
            ),
            // Written only by `collect`, typed: the source, then `f` if mapped.
            Callee::Collect { result, over } => {
                let typed: Vec<Typed> = args.iter().map(|a| self.expr(a, None)).collect();
                let item = match typed.get(1).and_then(|(_, t)| t.clone()) {
                    Some(Ty::Fn { ret, .. }) => Some(*ret),
                    _ if typed.len() == 1 => typed[0].1.as_ref().and_then(|t| self.walked(*over, t)),
                    _ => None,
                };
                let t = match (item, *result) {
                    (Some(Ty::Result { ok, err }), true) => Some(Ty::Result { ok: Box::new(Ty::Vec(ok)), err }),
                    (Some(item), false) => Some(Ty::Vec(Box::new(item))),
                    _ => None,
                };
                (typed.into_iter().map(|(e, _)| e).collect(), t)
            }
            // Written only by `cmp_method` and `ordering_method`, typed.
            Callee::OrdCmp { .. } | Callee::OrdCmpList { .. } | Callee::OrdThen => {
                (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::named(purecrate_ir::ORDERING)))
            }
            Callee::StrBytes => {
                (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::Vec(Box::new(Ty::Prim(Prim::U8)))))
            }
            Callee::StrSplit => {
                let (s, st) = self.expr(&args[0], None);
                if let Some(t) =
                    st.as_ref().filter(|t| !matches!(self.norm(t), Ty::Prim(Prim::String | Prim::Str) | Ty::Never))
                {
                    self.error(
                        Reason::TypeMismatch,
                        format!("`split` takes a `String` or `&str`, found `{}`", show(t)),
                    );
                }
                let (sep, sept) = self.expr(&args[1], Some(&Ty::Prim(Prim::Char)));
                if let Some(t) = sept.as_ref().filter(|t| !matches!(self.norm(t), Ty::Prim(Prim::Char) | Ty::Never)) {
                    self.error(Reason::TypeMismatch, format!(
                        "`split` takes a `char` separator in v0, found `{}` (a `&str` separator may be empty, where Rust and JS split differently)",
                        show(t)
                    ));
                }
                (vec![s, sep], Some(Ty::Vec(Box::new(Ty::Prim(Prim::Str)))))
            }
            Callee::StringFrom => (typed_args(self, vec![Ty::Prim(Prim::Str)]), Some(Ty::Prim(Prim::String))),
            Callee::StringNew => (Vec::new(), Some(Ty::Prim(Prim::String))),
            Callee::StrWellFormed => (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::Prim(Prim::Unit))),
            // Written only by `method_call` and `collect`, typed.
            Callee::StrConcat | Callee::StrFromChars => {
                (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::Prim(Prim::String)))
            }
            Callee::Slice { start, end, .. } => {
                let (base, bt) = self.expr(&args[0], None);
                let (of, ret) = match bt.as_ref().map(|t| self.norm(t)) {
                    Some(Ty::Prim(Prim::String | Prim::Str)) => (Some(SliceOf::Str), Some(Ty::Prim(Prim::Str))),
                    Some(t @ Ty::Vec(_)) => (Some(SliceOf::Items), Some(t)),
                    Some(Ty::Never) | None => (None, None),
                    Some(other) => {
                        self.error(Reason::Index, format!("cannot slice `{}`", show(&other)));
                        (None, None)
                    }
                };
                let mut typed = vec![base];
                for bound in &args[1..] {
                    typed.push(self.expr(bound, Some(&Ty::Prim(Prim::Usize))).0);
                }
                let e = Expr::Call { callee: Callee::Slice { of, start: *start, end: *end }, args: typed };
                return (e, self.expect(want, ret));
            }
            Callee::Str(m) => (
                args.iter().map(|a| self.expr(a, None).0).collect(),
                Some(match m {
                    StrMethod::Len => Ty::Prim(Prim::Usize),
                    StrMethod::AsStr => Ty::Prim(Prim::Str),
                    StrMethod::StripPrefix | StrMethod::StripSuffix => Ty::Option(Box::new(Ty::Prim(Prim::Str))),
                    StrMethod::SplitOnce => super::prim::split_once_ty(),
                    _ => Ty::bool(),
                }),
            ),
            Callee::IntFrom { to, .. } => return self.int_from(*to, args, want),
            Callee::IntCast { from, to } => {
                (typed_args(self, vec![Ty::Prim((*from).into())]), Some(Ty::Prim((*to).into())))
            }
            Callee::CharCode(to) => (typed_args(self, vec![Ty::Prim(Prim::Char)]), Some(Ty::Prim((*to).into()))),
            Callee::CharFromU8 => (typed_args(self, vec![Ty::Prim(Prim::U8)]), Some(Ty::Prim(Prim::Char))),
            Callee::CharFromU32 => {
                (typed_args(self, vec![Ty::Prim(Prim::U32)]), Some(Ty::option(Ty::Prim(Prim::Char))))
            }
            Callee::Char(m) => {
                let (params, ret) = char_sig(*m);
                let mut all = vec![Ty::Prim(Prim::Char)];
                all.extend(params);
                (typed_args(self, all), Some(ret))
            }
            Callee::UuidParse => (
                typed_args(self, vec![Ty::Prim(Prim::Str)]),
                Some(Ty::result(Ty::Prim(Prim::Uuid), Ty::Prim(Prim::UuidError))),
            ),
            Callee::UuidNil => (Vec::new(), Some(Ty::Prim(Prim::Uuid))),
            Callee::StrParse(t) => (
                typed_args(self, vec![Ty::Prim(Prim::Str)]),
                Some(Ty::result(Ty::Prim((*t).into()), Ty::Prim(Prim::ParseIntError))),
            ),
            Callee::Discriminant { to, .. } => (typed_args(self, Vec::new()), Some(Ty::Prim(Prim::from(*to)))),
        };
        let e = Expr::Call { callee: callee.clone(), args };
        (e, self.expect(want, t))
    }

    pub(super) fn construct_fields(&mut self, ty: &str, variant: Option<&str>, fields: &Fields) -> Fields {
        let declared: Vec<(Option<String>, Ty)> = match variant {
            None => self
                .defs
                .structs
                .get(ty)
                .map(|s| s.fields.iter().map(|f| (Some(f.name.as_str().to_string()), f.ty.clone())).collect())
                .unwrap_or_default(),
            Some(v) => match self
                .defs
                .enums
                .get(ty)
                .and_then(|e| e.variants.iter().find(|x| x.name.as_str() == v))
                .map(|x| &x.fields)
            {
                Some(VariantFields::Tuple(tys)) => tys.iter().map(|t| (None, t.clone())).collect(),
                Some(VariantFields::Struct(fs)) => {
                    fs.iter().map(|f| (Some(f.name.as_str().to_string()), f.ty.clone())).collect()
                }
                _ => Vec::new(),
            },
        };
        match fields {
            Fields::Unit => Fields::Unit,
            Fields::Positional(xs) => Fields::Positional(
                xs.iter()
                    .enumerate()
                    .map(|(i, x)| {
                        let want = declared.get(i).map(|(_, t)| t.clone());
                        self.expr(x, want.as_ref()).0
                    })
                    .collect(),
            ),
            Fields::Named(pairs) => Fields::Named(
                pairs
                    .iter()
                    .map(|(n, x)| {
                        let want =
                            declared.iter().find(|(d, _)| d.as_deref() == Some(n.as_str())).map(|(_, t)| t.clone());
                        (n.clone(), self.expr(x, want.as_ref()).0)
                    })
                    .collect(),
            ),
        }
    }
}

pub(super) fn sig(f: &Fn) -> (Vec<Ty>, Option<Ty>) {
    (f.params.iter().map(|p| p.ty.clone()).collect(), Some(f.ret.clone()))
}
