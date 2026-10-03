//! Method calls: the crate's own methods, the std allow-list, `Option`'s
//! combinators, `char` and `str` methods, widening, and `as`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// `x.m(args)` is `T::m(x, args)` for the crate's own `impl T`. The
    /// receiver is typed once, to find `T`, and the typed receiver is the
    /// call's first argument: typing it again would double the work at every
    /// link of a chain.
    pub(super) fn method_call(&mut self, receiver: &Expr, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        if let Some(typed) = self.consume(receiver, name.as_str(), args, want) {
            return typed;
        }
        if let Some(typed) = self.collect(receiver, name.as_str(), args, want) {
            return typed;
        }
        if let Some(typed) = self.map_unwrap_or(receiver, name.as_str(), args, want) {
            return typed;
        }
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
        if let Some(Ty::Result { ok, err }) = rt.as_ref().map(|t| self.norm(t)) {
            if let Some(typed) = self.result_method(recv.clone(), &ok, &err, name.as_str(), args, want) {
                return typed;
            }
        }
        if let Some(rt) = &rt {
            if name.as_str() == "cmp" {
                if let Some(typed) = self.cmp_method(recv.clone(), rt, args, want) {
                    return typed;
                }
            }
            if self.is_ordering(rt) {
                if let Some(typed) = self.ordering_method(recv.clone(), name.as_str(), args, want) {
                    return typed;
                }
            }
        }
        if name.as_str() == "parse" && args.is_empty() {
            if let Some(rt) = &rt {
                if matches!(self.norm(rt), Ty::Prim(Prim::String | Prim::Str)) {
                    return self.parse(recv, want);
                }
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
        if let Some(m) = IntMethod::from_name(name.as_str()) {
            if let Some(it) = rt.as_ref().and_then(|t| match self.norm(t) {
                Ty::Prim(p) => p.int(),
                _ => None,
            }) {
                return self.int_method(it, m, recv, args, want);
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
                let listed = if self.is_ordering(&rt) { Some(ordering_methods()) } else { std_methods(&rt) };
                let message = match listed {
                    None if name.as_str() == "cmp" => format!(
                        "`.cmp()` on `{}` is not in v0: `Ord` on the crate's own types is not modeled; compare the fields with `cmp` and chain them with `then` or `then_with`",
                        show(&rt)
                    ),
                    _ if name.as_str() == "partial_cmp" => format!(
                        "`.partial_cmp()` on `{}` is not in v0: use `cmp` on integers, `char`, `bool`, `String`/`&str`, or `Uuid` (floats are not `Ord`)",
                        show(&rt)
                    ),
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
                            Some(value) => Expr::Lit(Lit::Int { value, ty: Some(it), byte: false }),
                            None => Expr::Call {
                                callee: Callee::Discriminant { to: it, table, of: en.name.clone() },
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

    /// `all`, `any`, `position`, `count`, and `sum` on `s.chars()`,
    /// `s.bytes()`, or `xs.iter()` / `xs.into_iter()`: the loop std's
    /// default methods run, with the source bound once and the predicate
    /// inlined into the body (so it may not use `?` or `return`, which would
    /// leave the enclosing function). Each stops where std's does: `all` at
    /// the first `false`, `any` and `position` at the first `true`. `sum`
    /// adds from zero in order, panicking on overflow as a debug build does;
    /// on integers only, since float `Sum` starts from `-0.0`.
    fn consume(&mut self, receiver: &Expr, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let takes = match name {
            "all" | "any" | "position" => 1,
            "count" | "sum" => 0,
            _ => return None,
        };
        let Expr::MethodCall { receiver: source, name: adaptor, args: adaptor_args } = receiver.unpositioned() else {
            return None;
        };
        let (over, source) = match (adaptor.as_str(), adaptor_args.as_slice()) {
            ("chars", []) => (Over::Chars, (**source).clone()),
            ("bytes", []) => (Over::Bytes, (**source).clone()),
            ("iter" | "into_iter", []) => (Over::Items, (**source).clone()),
            // `s.split(c)` as a `for` gives it: the pieces, a `char` apart.
            ("split", [sep]) => (
                Over::Items,
                Expr::Call { callee: Callee::StrSplit, args: vec![(**source).clone(), sep.clone()] },
            ),
            _ => return None,
        };
        let failed = (Expr::Lit(Lit::Unit), None);
        if args.len() != takes {
            self.error(Reason::ConstructShape, format!("`{name}` takes {takes} argument(s), got {}", args.len()));
            return Some(failed);
        }
        let before = self.out.len();
        let (source, st) = self.expr(&source, None);
        let item = match (over, st.as_ref().map(|t| self.norm(t))) {
            (Over::Chars, Some(Ty::Prim(Prim::String | Prim::Str))) => Ty::Prim(Prim::Char),
            (Over::Bytes, Some(Ty::Prim(Prim::String | Prim::Str))) => Ty::Prim(Prim::U8),
            (Over::Items, Some(Ty::Vec(t))) => *t,
            (_, found) => {
                if self.out.len() == before {
                    let found = found.as_ref().map(show).unwrap_or_else(|| "?".into());
                    self.error(Reason::TypeMismatch, format!(
                        "`.{}().{name}()` takes a {}, found `{found}`",
                        adaptor.as_str(),
                        if over == Over::Items { "`Vec` or slice" } else { "`String` or `&str`" }
                    ));
                }
                return Some(failed);
            }
        };
        let pred = match args.first() {
            None => None,
            Some(arg) => match self.one_param_fn(name, arg) {
                Some(f) => Some(f),
                None => return Some(failed),
            },
        };
        let method = match name {
            "all" => Consume::All,
            "any" => Consume::Any,
            "position" => Consume::Position,
            "count" => Consume::Count,
            _ => match self.norm(&item) {
                Ty::Prim(p) if p.int().is_some() => Consume::Sum(p.int().expect("checked")),
                _ => {
                    self.error(Reason::TypeMismatch, format!(
                        "`sum` adds integers in v0, found `{}` (a float sum starts from `-0.0`)",
                        show(&item)
                    ));
                    return Some(failed);
                }
            },
        };
        let mut call_args = vec![source];
        if let Some((param, ty, body)) = pred {
            if let Some(written) = &ty {
                if self.norm(written) != self.norm(&item) {
                    self.error(Reason::TypeMismatch, format!(
                        "`{name}`'s closure takes `{}`, the items are `{}`",
                        show(written),
                        show(&item)
                    ));
                    return Some(failed);
                }
            }
            let closure = Expr::Closure {
                params: vec![ClosureParam { name: param, ty: Some(item.clone()) }],
                ret: Some(Ty::bool()),
                body: Box::new(body),
            };
            let (closure, _) = self.expr(&closure, None);
            call_args.push(closure);
        }
        let ty = match method {
            Consume::All | Consume::Any => Ty::bool(),
            Consume::Position => Ty::option(Ty::Prim(Prim::Usize)),
            Consume::Count => Ty::Prim(Prim::Usize),
            Consume::Sum(_) => item.clone(),
        };
        let e = Expr::Call { callee: Callee::Consume { method, over }, args: call_args };
        Some((e, self.expect(want, Some(ty))))
    }

    /// What an iterator method takes as its function: a closure of one
    /// parameter, or a function name as `|$x| f($x)`. The parameter, its
    /// written type, and the body; `None` once reported. The closure is
    /// inlined into a loop, so it may not use `?` or `return`, which would
    /// leave the enclosing function.
    /// `E::V` written where a function goes, `V` a one-field tuple variant:
    /// what builds `E::V(x)` from `x`.
    fn variant_fn(&self, e: &Expr) -> Option<impl std::ops::Fn(Expr) -> Expr> {
        let Expr::Construct { ty, variant: Some(variant), fields: Fields::Unit, base: None } = e.unpositioned() else {
            return None;
        };
        crate::resolve::one_field_variant(self.defs, ty, variant).then(|| {
            let (ty, variant) = (ty.clone(), variant.clone());
            move |x: Expr| Expr::Construct {
                ty: ty.clone(),
                variant: Some(variant.clone()),
                fields: Fields::Positional(vec![x]),
                base: None,
            }
        })
    }

    fn one_param_fn(&mut self, method: &str, arg: &Expr) -> Option<(Name, Option<Ty>, Expr)> {
        match arg.unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 1 => {
                if leaves(body) {
                    self.error(Reason::Closure, format!("a closure passed to `{method}` may not use `?` or `return` in v0; write the loop"));
                    return None;
                }
                let p = &params[0];
                let param = if p.name.as_str() == "_" { self.fresh_name("x") } else { p.name.clone() };
                Some((param, p.ty.clone(), (**body).clone()))
            }
            Expr::Var(f) => {
                let x = self.fresh_name("x");
                Some((x.clone(), None, Expr::Call { callee: Callee::Fn(f.clone()), args: vec![Expr::Var(x)] }))
            }
            _ => {
                self.error(Reason::Closure, format!("`{method}` takes a closure `|x| ..` or a function name in v0"));
                None
            }
        }
    }

    /// `s.split(c).collect()` and `s.split(c).map(f).collect()` into the
    /// `Vec<T>` or `Result<Vec<T>, E>` the context names (a turbofish, a
    /// `let` type, or the return type). The one way to build a `Vec` whose
    /// length is not in the source: once, from text, so the length is the
    /// input's. Collecting a `Vec` or a state is how a sequence grows, and
    /// stays out (design/07 §6). `None` when `name` is not `collect`.
    fn collect(&mut self, receiver: &Expr, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        if name != "collect" {
            return None;
        }
        let failed = (Expr::Lit(Lit::Unit), None);
        if !args.is_empty() {
            self.error(Reason::ConstructShape, format!("`collect` takes no arguments, got {}", args.len()));
            return Some(failed);
        }
        let (pieces, map) = match receiver.unpositioned() {
            Expr::MethodCall { receiver: inner, name: m, args: map_args } if m.as_str() == "map" && map_args.len() == 1 => {
                (inner.unpositioned(), Some(&map_args[0]))
            }
            other => (other, None),
        };
        let source = match pieces {
            Expr::MethodCall { receiver: s, name: m, args: sep } if m.as_str() == "split" && sep.len() == 1 => {
                Expr::Call { callee: Callee::StrSplit, args: vec![(**s).clone(), sep[0].clone()] }
            }
            _ => {
                self.error(Reason::MethodCall, "`collect` builds a `Vec` only from `s.split(c)` or `s.split(c).map(f)` in v0: a list read once from text. A sequence that grows with the state is a recursive enum (design/02 §3)".to_string());
                return Some(failed);
            }
        };
        let target = want.map(|t| self.norm(t));
        let (result, elem, err) = match target {
            Some(Ty::Vec(t)) => (false, *t, None),
            Some(Ty::Result { ok, err }) if matches!(self.norm(&ok), Ty::Vec(_)) => {
                let Ty::Vec(t) = self.norm(&ok) else { unreachable!("matched above") };
                (true, *t, Some(*err))
            }
            Some(Ty::Never) => return Some(failed),
            None => {
                self.error(Reason::NeedsAnnotation, "`collect` needs its target: `collect::<Vec<T>>()`, `collect::<Result<Vec<T>, E>>()`, or a typed `let`".to_string());
                return Some(failed);
            }
            Some(other) => {
                self.error(Reason::TypeMismatch, format!("`collect` builds a `Vec<T>` or a `Result<Vec<T>, E>` in v0, not `{}`", show(&other)));
                return Some(failed);
            }
        };
        let (source, _) = self.expr(&source, None);
        let item = Ty::Prim(Prim::Str);
        let Some(map) = map else {
            if result {
                self.error(Reason::TypeMismatch, "collecting into a `Result` takes `.map(f)` with `f` returning a `Result`".to_string());
                return Some(failed);
            }
            let e = Expr::Call { callee: Callee::Collect { result }, args: vec![source] };
            return Some((e, self.expect(want, Some(Ty::Vec(Box::new(item))))));
        };
        let Some((param, written, body)) = self.one_param_fn("map", map) else {
            return Some(failed);
        };
        if let Some(written) = &written {
            if self.norm(written) != item {
                self.error(Reason::TypeMismatch, format!("`map`'s closure takes `{}`, the pieces are `&str`", show(written)));
                return Some(failed);
            }
        }
        let ret = match &err {
            Some(err) => Ty::Result { ok: Box::new(elem.clone()), err: Box::new(err.clone()) },
            None => elem.clone(),
        };
        let closure = Expr::Closure {
            params: vec![ClosureParam { name: param, ty: Some(item) }],
            ret: Some(ret),
            body: Box::new(body),
        };
        let (closure, _) = self.expr(&closure, None);
        let e = Expr::Call { callee: Callee::Collect { result }, args: vec![source, closure] };
        Some((e, want.cloned()))
    }

    pub(super) fn fresh_name(&mut self, what: &str) -> Name {
        self.fresh += 1;
        Name::new(format!("${what}{}", self.fresh))
    }

    /// `o.map(f).unwrap_or(d)` as one `match o { Some(x) => f(x), None => d }`,
    /// with no `Option` in between, where `d` is a name or a literal: it has
    /// no effect, so running it only on `None` keeps Rust's behavior.
    fn map_unwrap_or(&mut self, receiver: &Expr, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let [default] = args else { return None };
        if name != "unwrap_or" || !default.is_inlinable() {
            return None;
        }
        let Expr::MethodCall { receiver: option, name: map, args: map_args } = receiver.unpositioned() else {
            return None;
        };
        let [f] = map_args.as_slice() else { return None };
        if map.as_str() != "map" {
            return None;
        }
        let (pattern, body) = match f.unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 1 && !leaves(body) => {
                (Pattern::Var(params[0].name.clone()), (**body).clone())
            }
            Expr::Var(f) => {
                let some = self.fresh_name("some");
                let call = Expr::Call { callee: Callee::Fn(f.clone()), args: vec![Expr::Var(some.clone())] };
                (Pattern::Var(some), call)
            }
            _ => return None,
        };
        // Only on an `Option`: a `Result` has a `map` and an `unwrap_or` too.
        let before = self.out.len();
        let (_, ty) = self.expr(option, None);
        self.out.truncate(before);
        if !matches!(ty.map(|t| self.norm(&t)), Some(Ty::Option(_))) {
            return None;
        }
        let fused = Expr::Match {
            scrutinee: option.clone(),
            arms: vec![
                Arm { guard: None, pattern: Pattern::OptionSome(Box::new(pattern)), body },
                Arm { guard: None, pattern: Pattern::OptionNone, body: default.clone() },
            ],
        };
        Some(self.expr(&fused, want))
    }

    /// `unwrap_or`, `ok_or`, and `map` on an `Option`, as the `match` std
    /// writes them. The typed receiver is bound to a fresh name first, so it
    /// is typed once however long the chain. `unwrap_or(d)` and `ok_or(e)`
    /// evaluate their argument whether or not the option is `Some`, as Rust
    /// does; `map(f)` calls `f` only on `Some`. `None` when `name` is none of
    /// these.
    pub(super) fn option_method(&mut self, recv: Expr, inner: &Ty, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
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
                    e if self.variant_fn(e).is_some() => {
                        let build = self.variant_fn(e)?;
                        (Pattern::Var(some.clone()), build(v(&some)))
                    }
                    _ => {
                        self.error(Reason::Closure, "`Option::map` takes a closure `|x| ..`, a function name, or a one-field tuple variant in v0".into());
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

    /// The `f` of `Result::map(f)` or `map_err(f)` as an arm's pattern and
    /// body: a closure of one parameter (`|_| ..` binds nothing), a function
    /// name, or a one-field tuple variant. `None` after reporting anything
    /// else.
    pub(super) fn arm_fn(&mut self, method: &str, f: &Expr) -> Option<(Pattern, Expr)> {
        match f.unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 1 => {
                if leaves(body) {
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
    pub(super) fn result_method(&mut self, recv: Expr, ok: &Ty, err: &Ty, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
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
                            Arm { guard: None, pattern: Pattern::ResultOk(Box::new(pattern)), body: Expr::Call { callee: Callee::ResultOk, args: vec![body] } },
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
        let e = Expr::Let {
            name: r,
            mutable: false,
            ty: Some(rt),
            value: Box::new(recv),
            then: Box::new(typed),
        };
        Some((e, t))
    }

    /// `s.parse()` into the `Result<T, ParseIntError>` the context names, `T`
    /// an integer type. Floats, `bool`, `char`, and the crate's `FromStr`
    /// impls are refused: their grammars are larger than the reason to
    /// carry them.
    fn parse(&mut self, recv: Expr, want: Option<&Ty>) -> Typed {
        let failed = (Expr::Lit(Lit::Unit), None);
        let it = match want.map(|t| self.norm(t)) {
            Some(Ty::Result { ok, err }) => match (self.norm(&ok), self.norm(&err)) {
                (Ty::Prim(p), Ty::Prim(Prim::ParseIntError)) if p.int().is_some() => p.int(),
                (ok, _) => {
                    self.error(Reason::MethodCall, format!(
                        "`parse` reads an integer type in v0 (`s.parse::<u64>()`), not `{}`",
                        show(&ok)
                    ));
                    return failed;
                }
            },
            Some(Ty::Never) => return failed,
            _ => {
                self.error(Reason::NeedsAnnotation, "`parse` needs its target: `s.parse::<u64>()`, or a typed `let` of `Result<u64, ParseIntError>`".to_string());
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
    pub(super) fn int_method(&mut self, it: IntTy, m: IntMethod, recv: Expr, args: &[Expr], want: Option<&Ty>) -> Typed {
        if m == IntMethod::Abs && !it.is_signed() {
            self.error(Reason::MethodCall, format!("`{}` has no `abs`; it is never negative", it.as_str()));
        }
        if args.len() + 1 != m.arity() {
            self.error(Reason::ConstructShape, format!(
                "`{}::{}` takes {} argument(s) after the receiver, got {}",
                it.as_str(),
                m.name(),
                m.arity() - 1,
                args.len()
            ));
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
    /// string, or for `split_once` a `char`: closure patterns are not in the
    /// allow-list.
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
                Some(Ty::Prim(Prim::Char)) if m == StrMethod::SplitOnce => {}
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
            StrMethod::StripPrefix | StrMethod::StripSuffix => Ty::Option(Box::new(Ty::Prim(Prim::Str))),
            StrMethod::SplitOnce => split_once_ty(),
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
        Ty::Prim(Prim::String) => StrMethod::ALL.iter().map(|m| m.name()).chain(["as_bytes", "cmp", "parse", "slicing `s[a..b]`"]).collect(),
        Ty::Prim(Prim::Str) => StrMethod::ALL
            .iter()
            .filter(|m| **m != StrMethod::AsStr)
            .map(|m| m.name())
            .chain(["as_bytes", "cmp", "parse", "slicing `s[a..b]`"])
            .collect(),
        Ty::Prim(Prim::Char) => CharMethod::ALL.iter().map(|m| m.name()).chain(["cmp"]).collect(),
        Ty::Prim(Prim::Bool | Prim::Uuid) => vec!["cmp"],
        Ty::Vec(_) => vec!["len", "is_empty", "indexing `xs[i]`", "slicing `xs[a..b]`"],
        Ty::Option(_) => vec!["is_some", "is_none", "unwrap_or", "ok_or", "map"],
        Ty::Result { .. } => vec!["ok", "map", "map_err"],
        Ty::Prim(p) if p.int().is_some() => IntMethod::ALL
            .iter()
            .filter(|m| **m != IntMethod::Abs || p.int().is_some_and(IntTy::is_signed))
            .map(|m| m.name())
            .chain(["cmp"])
            .collect(),
        _ => vec![],
    };
    Some(match (ty, names.is_empty()) {
        (_, true) => "none; use operators, `match`, or `T::from`".into(),
        (_, false) => {
            let list = names.iter().map(|n| if n.contains(' ') { n.to_string() } else { format!("`{n}`") }).collect::<Vec<_>>().join(", ");
            match ty {
                Ty::Prim(Prim::U8) => format!("{list}; for `u8::is_ascii_*` use `matches!(b, b'0'..=b'9')` or `char::from(b)`"),
                _ => list,
            }
        }
    })
}

/// `Ordering`'s allow-list, for a rejection message.
pub(super) fn ordering_methods() -> String {
    ORDERING_METHODS.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
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
