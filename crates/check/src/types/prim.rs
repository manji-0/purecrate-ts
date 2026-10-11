//! Methods of a primitive: integers, `char`, `str`, `parse`, and widening
//! with `T::from`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// `s.parse()` into the `Result<T, ParseIntError>` the context names, `T`
    /// an integer type. Floats, `bool`, `char`, and the crate's `FromStr`
    /// impls are refused: their grammars are larger than the reason to
    /// carry them.
    pub(super) fn parse(&mut self, recv: Expr, want: Option<&Ty>) -> Typed {
        let failed = (Expr::Lit(Lit::Unit), None);
        let it = match want.map(|t| self.norm(t)) {
            Some(Ty::Result { ok, err }) => match (self.norm(&ok), self.norm(&err)) {
                (Ty::Prim(p), Ty::Prim(Prim::ParseIntError)) if p.int().is_some() => p.int(),
                (ok, _) => {
                    self.error(
                        Reason::MethodCall,
                        format!("`parse` reads an integer type in v0 (`s.parse::<u64>()`), not `{}`", show(&ok)),
                    );
                    return failed;
                }
            },
            Some(Ty::Never) => return failed,
            _ => {
                self.error(
                    Reason::NeedsAnnotation,
                    "`parse` needs its target: `s.parse::<u64>()`, or a typed `let` of `Result<u64, ParseIntError>`"
                        .to_string(),
                );
                return failed;
            }
        };
        let Some(it) = it else { return failed };
        let e = Expr::Call { callee: Callee::StrParse(it), args: vec![recv] };
        (e, want.cloned())
    }

    /// `c.m(args)` for an allow-listed `char` method.
    /// `x.min(y)`, `x.checked_add(y)`, `x.pow(e)`, ...: a `Callee::Int` on
    /// the receiver's type, the receiver first.
    pub(super) fn int_method(
        &mut self,
        it: IntTy,
        m: IntMethod,
        recv: Expr,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Typed {
        if m == IntMethod::Abs && !it.is_signed() {
            self.error(Reason::MethodCall, format!("`{}` has no `abs`; it is never negative", it.as_str()));
        }
        if args.len() + 1 != m.arity() {
            self.error(
                Reason::ConstructShape,
                format!(
                    "`{}::{}` takes {} argument(s) after the receiver, got {}",
                    it.as_str(),
                    m.name(),
                    m.arity() - 1,
                    args.len()
                ),
            );
        }
        let e = Expr::Call {
            callee: Callee::Int { ty: it, op: IntOp::Method(m) },
            args: std::iter::once(recv).chain(args.iter().cloned()).collect(),
        };
        let (e, t) = self.int_call(it, IntOp::Method(m), e);
        (e, self.expect(want, t))
    }

    /// Types the arguments after the receiver of an integer call already
    /// built, and gives its result type.
    pub(super) fn int_call(&mut self, it: IntTy, op: IntOp, call: Expr) -> (Expr, Option<Ty>) {
        let Expr::Call { callee, args } = call else { unreachable!("an integer call") };
        let t = Ty::Prim(it.into());
        let mut typed = args.into_iter();
        let mut out = Vec::new();
        if let Some(recv) = typed.next() {
            out.push(recv);
        }
        for a in typed {
            let want = match op {
                IntOp::Method(m) if m.takes_exponent() => Ty::Prim(Prim::U32),
                _ => t.clone(),
            };
            out.push(self.expr(&a, Some(&want)).0);
        }
        let ret = match op {
            IntOp::Method(m) if m.is_checked() => Ty::option(t),
            _ => t,
        };
        (Expr::Call { callee, args: out }, Some(ret))
    }

    pub(super) fn char_method(&mut self, m: CharMethod, recv: Expr, args: &[Expr], want: Option<&Ty>) -> Typed {
        if args.len() != m.args() {
            self.error(
                Reason::ConstructShape,
                format!("`char::{}` takes {} argument(s) after the receiver, got {}", m.name(), m.args(), args.len()),
            );
        }
        let (params, ret) = char_sig(m);
        let mut typed = vec![recv];
        for (a, p) in args.iter().zip(&params) {
            typed.push(self.expr(a, Some(p)).0);
        }
        let e = Expr::Call { callee: Callee::Char(m), args: typed };
        (e, self.expect(want, Some(ret)))
    }

    /// `s.m(needles)` for an allow-listed `str` method. A needle must be a
    /// string, or for `split_once` a `char`: closure patterns are not in the
    /// allow-list.
    pub(super) fn str_method(&mut self, m: StrMethod, recv: Expr, args: &[Expr], want: Option<&Ty>) -> Typed {
        if args.len() != m.needles() {
            self.error(
                Reason::ConstructShape,
                format!("`str::{}` takes {} argument(s) after the receiver, got {}", m.name(), m.needles(), args.len()),
            );
        }
        let mut typed = vec![recv];
        for a in args {
            // A closure pattern takes a `char` and says whether it matches; a
            // function's name is that closure.
            let a = &self.named_fn(a, Ty::Prim(Prim::Char)).unwrap_or_else(|| a.clone());
            let f = Ty::Fn { params: vec![Ty::Prim(Prim::Char)], ret: Box::new(Ty::bool()) };
            if m.takes_pattern() && matches!(a.unpositioned(), Expr::Closure { .. }) {
                let (e, _) = self.expr(a, Some(&f));
                typed.push(e);
                continue;
            }
            if m.takes_pattern() {
                let (e, t) = self.expr(a, None);
                if t.as_ref().is_some_and(|t| matches!(self.norm(t), Ty::Fn { .. })) {
                    let (e, _) = self.expr(&e, Some(&f));
                    typed.push(e);
                    continue;
                }
            }
            let (e, t) = self.expr(a, None);
            match t.map(|t| self.norm(&t)) {
                Some(Ty::Prim(Prim::String | Prim::Str)) | Some(Ty::Never) | None => {}
                Some(Ty::Prim(Prim::Char)) if m == StrMethod::SplitOnce || m.takes_pattern() => {}
                Some(other) => self.error(
                    Reason::TypeMismatch,
                    format!("`str::{}` takes a `&str` pattern in v0, found `{}`", m.name(), show(&other)),
                ),
            }
            typed.push(e);
        }
        let ret = match m {
            StrMethod::Len => Ty::Prim(Prim::Usize),
            StrMethod::AsStr => Ty::Prim(Prim::Str),
            StrMethod::StripPrefix | StrMethod::StripSuffix => Ty::Option(Box::new(Ty::Prim(Prim::Str))),
            StrMethod::SplitOnce => split_once_ty(),
            StrMethod::ToAsciiLowercase | StrMethod::ToAsciiUppercase => Ty::Prim(Prim::String),
            StrMethod::Trim
            | StrMethod::TrimStart
            | StrMethod::TrimEnd
            | StrMethod::TrimMatches
            | StrMethod::TrimStartMatches
            | StrMethod::TrimEndMatches => Ty::Prim(Prim::Str),
            StrMethod::Find => Ty::option(Ty::Prim(Prim::Usize)),
            StrMethod::IsEmpty
            | StrMethod::StartsWith
            | StrMethod::EndsWith
            | StrMethod::Contains
            | StrMethod::EqIgnoreAsciiCase => Ty::bool(),
        };
        let e = Expr::Call { callee: Callee::Str(m), args: typed };
        (e, self.expect(want, Some(ret)))
    }

    /// A crate function's name where a function of one `param` goes, as the
    /// closure `|$x| f($x)`; `None` for anything else.
    pub(super) fn named_fn(&mut self, e: &Expr, param: Ty) -> Option<Expr> {
        let Expr::Var(f) = e.unpositioned() else { return None };
        if !self.defs.free_fns.contains_key(f.as_str()) {
            return None;
        }
        let x = self.fresh_name("x");
        Some(Expr::Closure {
            params: vec![ClosureParam { name: x.clone(), ty: Some(param) }],
            ret: None,
            body: Box::new(Expr::Call { callee: Callee::Fn(f.clone()), args: vec![Expr::Var(x)] }),
        })
    }

    /// `f64::from(x)` / `f32::from(x)`: what std's `From` takes, each exact.
    /// An integer becomes `IntToFloat`, an `f32` to `f64` `FloatToFloat`.
    pub(super) fn float_from(&mut self, to: FloatTy, args: &[Expr], want: Option<&Ty>) -> Typed {
        let (arg, t) = self.expr(&args[0], None);
        let want_ty = Ty::Prim(to.into());
        let callee = match t.as_ref().and_then(|t| self.num(t)) {
            Some(Num::Int(from)) if float_widens(from, to) => Some(Callee::IntToFloat { from, to }),
            Some(Num::Float(from)) if from == to => return (arg, self.expect(want, Some(want_ty))),
            Some(Num::Float(FloatTy::F32)) => Some(Callee::FloatToFloat { to }),
            Some(Num::Int(from)) => {
                self.error(
                    Reason::NumericOp,
                    format!(
                        "`{}::from` does not take `{}`: std has no lossless conversion; `x as {}` rounds to the nearest",
                        to.as_str(),
                        from.as_str(),
                        to.as_str()
                    ),
                );
                None
            }
            Some(Num::Float(from)) => {
                self.error(
                    Reason::NumericOp,
                    format!("`{}::from` does not take `{}`: `x as {}` rounds", to.as_str(), from.as_str(), to.as_str()),
                );
                None
            }
            None => {
                if let Some(t) = t.as_ref().filter(|t| **t != Ty::Never) {
                    self.error(
                        Reason::TypeMismatch,
                        format!("`{}::from` takes a number in v0, found `{}`", to.as_str(), show(t)),
                    );
                }
                None
            }
        };
        match callee {
            Some(callee) => (Expr::Call { callee, args: vec![arg] }, self.expect(want, Some(want_ty))),
            None => (arg, None),
        }
    }

    /// `to::from(x)`: the argument is typed on its own, then must widen to
    /// `to` without loss. Narrowing has no `From` in std and stays rejected.
    pub(super) fn int_from(&mut self, to: IntTy, args: &[Expr], want: Option<&Ty>) -> Typed {
        let typed: Vec<Typed> = args.iter().map(|a| self.expr(a, None)).collect();
        let arg = typed.first().and_then(|(_, t)| t.clone()).map(|t| self.norm(&t));
        if arg == Some(Ty::Prim(Prim::Char)) {
            if !matches!(to, IntTy::U32 | IntTy::U64 | IntTy::U128) {
                self.error(
                    Reason::NumericOp,
                    format!(
                        "`{}::from` does not take `char`: std converts a `char` only to `u32`, `u64`, and `u128`",
                        to.as_str()
                    ),
                );
            }
            let e = Expr::Call { callee: Callee::CharCode(to), args: typed.into_iter().map(|(e, _)| e).collect() };
            return (e, self.expect(want, Some(Ty::Prim(to.into()))));
        }
        let from = match typed.first().map(|(_, t)| t.clone()) {
            Some(Some(t)) if t != Ty::Never => match self.num(&t) {
                Some(Num::Int(f)) if f.widens_to(to) => Some(f),
                Some(Num::Int(f)) => {
                    self.error(
                        Reason::NumericOp,
                        format!(
                            "`{}::from` does not take `{}`: std has no lossless conversion between them",
                            to.as_str(),
                            f.as_str()
                        ),
                    );
                    None
                }
                _ => {
                    self.error(
                        Reason::TypeMismatch,
                        format!("`{}::from` takes an integer in v0, found `{}`", to.as_str(), show(&t)),
                    );
                    None
                }
            },
            _ => None,
        };
        let e = Expr::Call { callee: Callee::IntFrom { from, to }, args: typed.into_iter().map(|(e, _)| e).collect() };
        (e, self.expect(want, Some(Ty::Prim(to.into()))))
    }
}

/// std implements `From<from> for to`: the integers a float holds exactly.
pub(super) fn float_widens(from: IntTy, to: FloatTy) -> bool {
    match to {
        FloatTy::F64 => matches!(from, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::U8 | IntTy::U16 | IntTy::U32),
        FloatTy::F32 => matches!(from, IntTy::I8 | IntTy::I16 | IntTy::U8 | IntTy::U16),
    }
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

/// `Option<(&str, &str)>`, what `str::split_once` returns.
pub(super) fn split_once_ty() -> Ty {
    Ty::option(Ty::Tuple(vec![Ty::Prim(Prim::Str), Ty::Prim(Prim::Str)]))
}
