//! Calls, closures, struct and variant construction, and `?`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// The items `over` walks in a value of type `ty`.
    pub(super) fn walked(&self, over: Over, ty: &Ty) -> Option<Ty> {
        match (over, self.norm(ty)) {
            (Over::Chars, Ty::Prim(Prim::String | Prim::Str)) => Some(Ty::Prim(Prim::Char)),
            (Over::Bytes, Ty::Prim(Prim::String | Prim::Str)) => Some(Ty::Prim(Prim::U8)),
            (Over::Items, Ty::Vec(t)) => Some(*t),
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
        let (pattern, body) = self.arm_fn("`Result::map_err`", f)?;
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
        let lets = vec![(r, Some(Ty::result(*ok.clone(), *err)), recv)];
        let (typed, t) = self.scoped(&lets, &rest, want.or(Some(&*ok)));
        Some((wrap_lets(lets, typed), t))
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
            Callee::Fn(_) | Callee::Method { .. } => match self.callee_fn(callee) {
                Some((f, what)) if !f.generics.is_empty() => {
                    let (params, ret) = sig(f);
                    let ret = ret.unwrap_or(Ty::Prim(Prim::Unit));
                    self.generic_args(&what, &f.generics, Vec::new(), &params, args, &ret, want)
                }
                f => {
                    let (params, ret) = f.map(|(f, _)| sig(f)).unwrap_or_default();
                    (typed_args(self, params), ret)
                }
            },
            // A newtype's constructor, `Wrap(x)`: built as `Wrap { 0: x }`
            // is, so a generic one takes its argument from the field.
            Callee::StructNew(n) => match self.construct(n, None, &Fields::Positional(args.to_vec()), want) {
                (Fields::Positional(args), t) => (args, t),
                _ => unreachable!("construct keeps the fields' shape"),
            },
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
                        let (args, tys) = self.typed_all(args);
                        let inner = tys.into_iter().next().flatten();
                        if inner.as_ref().is_some_and(|t| matches!(self.norm(t), Ty::Option(_))) {
                            self.error(
                                Reason::NestedOption,
                                format!(
                                "`Some` of `{}` is `Option<Option<_>>`, and both `None`s are `null` in TS; use an enum",
                                show(inner.as_ref().expect("checked"))
                            ),
                            );
                        }
                        (args, inner.map(Ty::option))
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
            Callee::VecLen => (typed_args(self, Vec::new()), Some(Ty::Prim(Prim::Usize))),
            // Written only by `binary` and `method_call`, typed.
            Callee::DeepEq | Callee::VecContains | Callee::VecIsEmpty | Callee::OptionIsSome | Callee::OptionIsNone => {
                (typed_args(self, Vec::new()), Some(Ty::bool()))
            }
            // Written only by `method_call` and `syntax`, typed.
            Callee::VecSort(_) | Callee::VecPush | Callee::VecInsert | Callee::StrWellFormed => {
                (typed_args(self, Vec::new()), Some(Ty::Prim(Prim::Unit)))
            }
            // Written only by `method_call` and `collect`, typed.
            Callee::StrConcat | Callee::StrFromChars => (typed_args(self, Vec::new()), Some(Ty::Prim(Prim::String))),
            Callee::VecRev | Callee::VecZip => {
                let (args, tys) = self.typed_all(args);
                let mut items = tys.iter().map(|t| self.vec_item(t));
                let t = match callee {
                    Callee::VecRev => items.next().flatten().map(|t| Ty::Vec(Box::new(t))),
                    _ => match (items.next().flatten(), items.next().flatten()) {
                        (Some(a), Some(b)) => Some(Ty::Vec(Box::new(Ty::Tuple(vec![a, b])))),
                        _ => None,
                    },
                };
                (args, t)
            }
            // Written only by `tuple`, after typing.
            Callee::SliceElem { .. } | Callee::SliceRest { .. } => {
                let (args, tys) = self.typed_all(args);
                let t = match callee {
                    Callee::SliceElem { .. } => self.vec_item(&tys[0]),
                    _ => tys[0].clone(),
                };
                (args, t)
            }
            Callee::VecGet | Callee::VecLast => {
                let (args, tys) = self.typed_all(args);
                (args, self.vec_item(&tys[0]).map(Ty::option))
            }
            Callee::VecRemove => {
                let (args, tys) = self.typed_all(args);
                (args, self.vec_item(&tys[0]))
            }
            // Written by `syntax` for `v[i] = x`: only a local's own array.
            Callee::VecSet => {
                let (v, at, value, compound) = match args {
                    [v, at, value] => (v, at, value, false),
                    [v, at, value, _] => (v, at, value, true),
                    _ => unreachable!("`syntax` writes three arguments, or four for `op=`"),
                };
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
                // Rust evaluates the value before the place, and in `v[i] op=
                // x` on a primitive the `x` before `v[i]` (`v[i] = v[i] op x`
                // written out reads `v[i]` first, as part of the value): that
                // part is bound first, so every later pass (a lifted block
                // included) keeps the order. `emit` puts a name or literal
                // back in place.
                let reads_place = |e: &Expr| matches!(e.unpositioned(), Expr::Index { base, .. } if base.unpositioned() == v.unpositioned());
                let name = self.fresh_name("value");
                let placeholder = Box::new(Expr::Var(name.clone()));
                let (bound, set_value, ty) = match value {
                    Expr::Binary { op, left, right } if compound && reads_place(&left) => {
                        (*right, Expr::Binary { op, left, right: placeholder }, None)
                    }
                    Expr::Call { callee: callee @ Callee::Int { .. }, args }
                        if compound && args.len() == 2 && reads_place(&args[0]) =>
                    {
                        let [read, right]: [Expr; 2] = args.try_into().expect("two arguments");
                        (right, Expr::Call { callee, args: vec![read, *placeholder] }, None)
                    }
                    value => (value, *placeholder, item.clone()),
                };
                let set = Expr::Call { callee: Callee::VecSet, args: vec![v, at, set_value] };
                let e = Expr::Let { name, mutable: false, ty, value: Box::new(bound), then: Box::new(set) };
                return (e, self.expect(want, Some(Ty::Prim(Prim::Unit))));
            }
            // Written only by `binary`, on strings already typed.
            Callee::StrCmp => {
                (typed_args(self, vec![Ty::Prim(Prim::Str), Ty::Prim(Prim::Str)]), Some(Ty::Prim(Prim::I32)))
            }
            // Written only by `sequence`, typed: a `Vec` of what the stage gives.
            Callee::IterMap { .. } | Callee::IterFilter { .. } => {
                let (args, tys) = self.typed_all(args);
                let item = match (callee, tys.get(1).cloned().flatten()) {
                    (Callee::IterMap { .. }, Some(Ty::Fn { ret, .. })) => Some(*ret),
                    (Callee::IterFilter { over }, _) => tys[0].as_ref().and_then(|t| self.walked(*over, t)),
                    _ => None,
                };
                (args, item.map(|t| Ty::Vec(Box::new(t))))
            }
            // Written only by `consume`, typed.
            Callee::Consume { method, over } => {
                let (args, tys) = self.typed_all(args);
                let item = tys[0].as_ref().and_then(|t| self.walked(*over, t));
                let t = match method {
                    purecrate_ir::Consume::All | purecrate_ir::Consume::Any => Some(Ty::bool()),
                    purecrate_ir::Consume::Position => Some(Ty::option(Ty::Prim(Prim::Usize))),
                    purecrate_ir::Consume::Count => Some(Ty::Prim(Prim::Usize)),
                    purecrate_ir::Consume::Sum(int) => Some(Ty::Prim(Prim::from(*int))),
                    _ => item.map(Ty::option),
                };
                (args, t)
            }
            // Written only by `collect`, typed: the source, then `f` if mapped.
            Callee::Collect { result, over } => {
                let (args, tys) = self.typed_all(args);
                let item = match tys.get(1).cloned().flatten() {
                    Some(Ty::Fn { ret, .. }) => Some(*ret),
                    _ if tys.len() == 1 => tys[0].as_ref().and_then(|t| self.walked(*over, t)),
                    _ => None,
                };
                let t = match (item, *result) {
                    (Some(Ty::Result { ok, err }), true) => Some(Ty::Result { ok: Box::new(Ty::Vec(ok)), err }),
                    (Some(item), false) => Some(Ty::Vec(Box::new(item))),
                    _ => None,
                };
                (args, t)
            }
            // Written only by `cmp_method` and `ordering_method`, typed.
            Callee::OrdCmp { .. } | Callee::OrdCmpList { .. } | Callee::OrdThen => {
                (typed_args(self, Vec::new()), Some(Ty::named(purecrate_ir::ORDERING)))
            }
            Callee::StrBytes => (typed_args(self, Vec::new()), Some(Ty::Vec(Box::new(Ty::Prim(Prim::U8))))),
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
                // A closure on a `char` (or a function's name) says where to split.
                let named = self.named_fn(&args[1], Ty::Prim(Prim::Char));
                let sep_arg = named.as_ref().unwrap_or(&args[1]);
                if matches!(sep_arg.unpositioned(), Expr::Closure { .. }) {
                    let f = Ty::Fn { params: vec![Ty::Prim(Prim::Char)], ret: Box::new(Ty::bool()) };
                    let (sep, _) = self.expr(sep_arg, Some(&f));
                    return (
                        Expr::Call { callee: callee.clone(), args: vec![s, sep] },
                        self.expect(want, Some(Ty::Vec(Box::new(Ty::Prim(Prim::Str))))),
                    );
                }
                let (sep, sept) = self.expr(&args[1], None);
                let pieces = Some(Ty::Vec(Box::new(Ty::Prim(Prim::Str))));
                match sept.as_ref().map(|t| self.norm(t)) {
                    Some(Ty::Prim(Prim::Char) | Ty::Never) | None => {}
                    // A literal prints as JS `split`, which cuts at the same
                    // matches; any other text through `Str.splitStr`, which
                    // splits at char boundaries on an empty one, as Rust does.
                    Some(Ty::Prim(Prim::String | Prim::Str)) => match sep.unpositioned() {
                        Expr::Lit(Lit::Str(t)) if t.is_empty() => self.error(
                            Reason::MethodCall,
                            "`split(\"\")` is not in v0: Rust gives an empty piece at each end (`\"ab\".split(\"\")` is `[\"\", \"a\", \"b\", \"\"]`); iterate `s.chars()`".to_string(),
                        ),
                        Expr::Lit(Lit::Str(_)) => {}
                        _ => {
                            let e = Expr::Call { callee: Callee::StrSplitStr, args: vec![s, sep] };
                            return (e, self.expect(want, pieces));
                        }
                    },
                    Some(t) => self.error(
                        Reason::TypeMismatch,
                        format!("`split` takes a `char`, a `&str`, or a closure on a `char` in v0, found `{}`", show(&t)),
                    ),
                }
                (vec![s, sep], pieces)
            }
            // Written only by `split` and `str_method`, typed.
            Callee::StrSplitStr => (typed_args(self, Vec::new()), Some(Ty::Vec(Box::new(Ty::Prim(Prim::Str))))),
            Callee::StrRepeat | Callee::StrJoin => (typed_args(self, Vec::new()), Some(Ty::Prim(Prim::String))),
            Callee::StringFrom => (typed_args(self, vec![Ty::Prim(Prim::Str)]), Some(Ty::Prim(Prim::String))),
            Callee::Format(pieces) => return self.format(pieces, args, want),
            Callee::Tree(op) => return self.tree_call(*op, args, want),
            // Written by `syntax` with a `&str` or `String` message.
            Callee::Panic => (typed_args(self, Vec::new()), Some(Ty::Never)),
            Callee::StringNew => (Vec::new(), Some(Ty::Prim(Prim::String))),
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
            Callee::Str(m) => (typed_args(self, Vec::new()), Some(str_ret(*m))),
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

    /// The crate function or method a `Callee::Fn` or `Callee::Method`
    /// names, and how a message names it.
    fn callee_fn(&self, callee: &Callee) -> Option<(&'a Fn, String)> {
        match callee {
            Callee::Fn(n) => self.defs.free_fns.get(n.as_str()).map(|f| (*f, format!("`{}`", n.as_str()))),
            Callee::Method { ty, name } => self
                .defs
                .methods
                .get(&(ty.as_str(), name.as_str()))
                .map(|f| (*f, format!("`{}::{}`", ty.as_str(), name.as_str()))),
            _ => None,
        }
    }

    /// `args` typed with no hint, and the type of each.
    fn typed_all(&mut self, args: &[Expr]) -> (Vec<Expr>, Vec<Option<Ty>>) {
        args.iter().map(|a| self.expr(a, None)).unzip()
    }

    /// The item type of `t` when it is a `Vec`.
    fn vec_item(&self, t: &Option<Ty>) -> Option<Ty> {
        match t.as_ref().map(|t| self.norm(t)) {
            Some(Ty::Vec(item)) => Some(*item),
            _ => None,
        }
    }

    /// A struct's or variant's fields typed against their declared types,
    /// and the value's type: `Named`, or for a generic one an `App` whose
    /// arguments come from `want` or else from the fields.
    pub(super) fn construct(
        &mut self,
        ty: &Name,
        variant: Option<&str>,
        fields: &Fields,
        want: Option<&Ty>,
    ) -> (Fields, Option<Ty>) {
        let params = self.defs.type_params(ty.as_str()).unwrap_or_default();
        let mut binds = Vec::new();
        if let Some(w) = want {
            self.unify(
                &Ty::App { name: ty.clone(), args: params.iter().map(|p| Ty::Param(p.clone())).collect() },
                w,
                params,
                &mut binds,
            );
        }
        let declared = self.defs.fields_of(ty.as_str(), variant);
        let fields = match fields {
            Fields::Unit => Fields::Unit,
            Fields::Positional(xs) => Fields::Positional(
                xs.iter()
                    .enumerate()
                    .map(|(i, x)| self.typed_against(declared.get(i).map(|(_, t)| *t), x, params, &mut binds))
                    .collect(),
            ),
            Fields::Named(pairs) => Fields::Named(
                pairs
                    .iter()
                    .map(|(n, x)| {
                        let decl = declared.iter().find(|(d, _)| *d == Some(n)).map(|(_, t)| *t);
                        (n.clone(), self.typed_against(decl, x, params, &mut binds))
                    })
                    .collect(),
            ),
        };
        if let Some(p) = params.iter().find(|p| !binds.iter().any(|(b, _)| b == *p)) {
            self.error(
                Reason::NeedsAnnotation,
                format!(
                    "the type `{}` of this `{}` is not known here; annotate the binding it goes to",
                    p.as_str(),
                    ty.as_str()
                ),
            );
            return (fields, None);
        }
        if !self.instance_ok(&binds, &format!("`{}`", ty.as_str())) {
            return (fields, None);
        }
        (fields, Some(self.applied_ty(ty, params, &binds)))
    }
}

pub(super) fn sig(f: &Fn) -> (Vec<Ty>, Option<Ty>) {
    (f.params.iter().map(|p| p.ty.clone()).collect(), Some(f.ret.clone()))
}
