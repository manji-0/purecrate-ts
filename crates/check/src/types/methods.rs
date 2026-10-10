//! Method calls: the crate's own methods and the std allow-list, each
//! handed to `iter`, `option_result`, or `prim`; and `as`.

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
        // `clone` is a new array for a `Vec`, which a local may push to, and
        // the value itself for anything else, which nothing writes; `as_ref`
        // and `as_deref` on an `Option` are the option (design/01 §7.14).
        if args.is_empty() && matches!(name.as_str(), "clone" | "as_ref" | "as_deref" | "cloned" | "copied") {
            if let Some(rt) = &rt {
                match (name.as_str(), self.norm(rt)) {
                    ("clone", Ty::Vec(_)) => {
                        let e = Expr::Call {
                            callee: Callee::Collect { result: false, over: Over::Items },
                            args: vec![recv],
                        };
                        return (e, self.expect(want, Some(rt.clone())));
                    }
                    // `Option<&T>` from `find`, `max`, and the others is
                    // `Option<T>` here, as every reference is the value.
                    ("clone", _) | ("as_ref" | "cloned" | "copied", Ty::Option(_)) => {
                        return (recv, self.expect(want, Some(rt.clone())))
                    }
                    ("as_deref", Ty::Option(inner)) if matches!(self.norm(&inner), Ty::Prim(Prim::String)) => {
                        return (recv, self.expect(want, Some(Ty::option(Ty::Prim(Prim::Str)))));
                    }
                    _ => {}
                }
            }
        }
        if name.as_str() == "push" {
            if let Some(Ty::Vec(item)) = rt.as_ref().map(|t| self.norm(t)) {
                let failed = (Expr::Lit(Lit::Unit), None);
                let [arg] = args else {
                    self.error(Reason::ConstructShape, format!("`push` takes 1 argument, got {}", args.len()));
                    return failed;
                };
                if !matches!(recv.unpositioned(), Expr::Var(_)) {
                    self.error(Reason::MethodCall, "`push` grows a local `let mut v: Vec<T>` in v0, not a field or an element: build the new `Vec` and put it in a new value (design/02 §3.1)".to_string());
                    return failed;
                }
                let (arg, _) = self.expr(arg, Some(&item));
                let e = Expr::Call { callee: Callee::VecPush, args: vec![recv, arg] };
                return (e, self.expect(want, Some(Ty::Prim(Prim::Unit))));
            }
        }
        if name.as_str() == "insert" {
            if let Some(Ty::Vec(item)) = rt.as_ref().map(|t| self.norm(t)) {
                let failed = (Expr::Lit(Lit::Unit), None);
                let [at, arg] = args else {
                    self.error(Reason::ConstructShape, format!("`insert` takes 2 arguments, got {}", args.len()));
                    return failed;
                };
                if !matches!(recv.unpositioned(), Expr::Var(_)) {
                    self.error(Reason::MethodCall, "`insert` grows a local `let mut v: Vec<T>` in v0, not a field or an element: build the new `Vec` and put it in a new value (design/02 §3.1)".to_string());
                    return failed;
                }
                let (at, _) = self.expr(at, Some(&Ty::Prim(Prim::Usize)));
                let (arg, _) = self.expr(arg, Some(&item));
                let e = Expr::Call { callee: Callee::VecInsert, args: vec![recv, at, arg] };
                return (e, self.expect(want, Some(Ty::Prim(Prim::Unit))));
            }
        }
        if matches!(name.as_str(), "sort" | "sort_by" | "sort_by_key") {
            if let Some(Ty::Vec(item)) = rt.as_ref().map(|t| self.norm(t)) {
                let failed = (Expr::Lit(Lit::Unit), None);
                let method = name.as_str();
                let takes = usize::from(method != "sort");
                if args.len() != takes {
                    self.error(
                        Reason::ConstructShape,
                        format!("`{method}` takes {takes} argument(s), got {}", args.len()),
                    );
                    return failed;
                }
                if !matches!(recv.unpositioned(), Expr::Var(_)) {
                    self.error(Reason::MethodCall, format!("`{method}` sorts a local `let mut v: Vec<T>` in v0, not a field or an element: build the new `Vec` and put it in a new value (design/02 §3.1)"));
                    return failed;
                }
                if args.first().is_some_and(|a| !matches!(a.unpositioned(), Expr::Closure { .. }) || a.exits()) {
                    self.error(Reason::Closure, format!("`{method}` takes a closure without `?` or `return` in v0"));
                    return failed;
                }
                let (sort, args) = match method {
                    "sort" => {
                        let Some(text) = self.ordered_by(&item, method) else { return failed };
                        (Sort::Natural { text }, vec![recv])
                    }
                    "sort_by" => {
                        let ordering = Ty::named(purecrate_ir::ORDERING);
                        let f = Ty::Fn { params: vec![(*item).clone(), (*item).clone()], ret: Box::new(ordering) };
                        let (closure, _) = self.expr(&args[0], Some(&f));
                        (Sort::By, vec![recv, closure])
                    }
                    _ => {
                        // The parameter takes the item where its type is not written.
                        let Expr::Closure { params, body, ret } = args[0].unpositioned().clone() else {
                            unreachable!("checked above")
                        };
                        let params = params
                            .into_iter()
                            .map(|p| ClosureParam { ty: p.ty.or(Some((*item).clone())), ..p })
                            .collect();
                        let (closure, ct) = self.expr(&Expr::Closure { params, body, ret }, None);
                        let key = match ct.map(|t| self.norm(&t)) {
                            Some(Ty::Fn { ret, .. }) => *ret,
                            _ => return failed,
                        };
                        let Some(text) = self.ordered_by(&key, method) else { return failed };
                        (Sort::ByKey { text }, vec![recv, closure])
                    }
                };
                let e = Expr::Call { callee: Callee::VecSort(sort), args };
                return (e, self.expect(want, Some(Ty::Prim(Prim::Unit))));
            }
        }
        if name.as_str() == "remove" {
            if let Some(Ty::Vec(item)) = rt.as_ref().map(|t| self.norm(t)) {
                let failed = (Expr::Lit(Lit::Unit), None);
                let [at] = args else {
                    self.error(Reason::ConstructShape, format!("`remove` takes 1 argument, got {}", args.len()));
                    return failed;
                };
                if !matches!(recv.unpositioned(), Expr::Var(_)) {
                    self.error(Reason::MethodCall, "`remove` takes from a local `let mut v: Vec<T>` in v0, not a field or an element: build the new `Vec` and put it in a new value (design/02 §3.1)".to_string());
                    return failed;
                }
                let (at, _) = self.expr(at, Some(&Ty::Prim(Prim::Usize)));
                let e = Expr::Call { callee: Callee::VecRemove, args: vec![recv, at] };
                return (e, self.expect(want, Some(*item)));
            }
        }
        // `s.push(c)` / `s.push_str(t)` on a local `let mut s: String`: the
        // assignment `s = s + c`, so every pass sees a write it knows.
        if matches!(name.as_str(), "push" | "push_str")
            && matches!(rt.as_ref().map(|t| self.norm(t)), Some(Ty::Prim(Prim::String)))
        {
            let failed = (Expr::Lit(Lit::Unit), None);
            let [arg] = args else {
                self.error(Reason::ConstructShape, format!("`{}` takes 1 argument, got {}", name.as_str(), args.len()));
                return failed;
            };
            let Expr::Var(target) = recv.unpositioned() else {
                self.error(Reason::MethodCall, format!("`{}` grows a local `let mut s: String` in v0, not a field or an element: build the new `String` and put it in a new value (design/02 §3.1)", name.as_str()));
                return failed;
            };
            let piece = if name.as_str() == "push" { Ty::Prim(Prim::Char) } else { Ty::Prim(Prim::Str) };
            let (arg, at) = self.expr(arg, None);
            let fits = match at.map(|t| self.norm(&t)) {
                Some(Ty::Prim(Prim::String | Prim::Str)) => piece == Ty::Prim(Prim::Str),
                Some(Ty::Prim(Prim::Char)) => piece == Ty::Prim(Prim::Char),
                _ => true,
            };
            if !fits {
                let want = if piece == Ty::Prim(Prim::Char) { "a `char`" } else { "a `&str`" };
                self.error(Reason::TypeMismatch, format!("`String::{}` takes {want}", name.as_str()));
                return failed;
            }
            let value = Expr::Call { callee: Callee::StrConcat, args: vec![Expr::Var(target.clone()), arg] };
            let e = Expr::Assign { name: target.clone(), value: Box::new(value) };
            return (e, self.expect(want, Some(Ty::Prim(Prim::Unit))));
        }
        if name.as_str() == "len" && args.is_empty() {
            if let Some(rt) = &rt {
                if matches!(self.norm(rt), Ty::Vec(_)) {
                    let e = Expr::Call { callee: Callee::VecLen, args: vec![recv] };
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
                    let e = Expr::Call { callee: Callee::StrBytes, args: vec![recv] };
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
        if let Some(m) = FloatMethod::ALL.into_iter().find(|m| m.name() == name.as_str()) {
            if let Some(ft) = rt.as_ref().and_then(|t| match self.norm(t) {
                Ty::Prim(p) => p.float(),
                _ => None,
            }) {
                if !args.is_empty() {
                    self.error(
                        Reason::ConstructShape,
                        format!("`{}::{}` takes no argument, got {}", ft.as_str(), m.name(), args.len()),
                    );
                }
                let t = Ty::Prim(ft.into());
                let e = Expr::Call { callee: Callee::Float { ty: ft, m }, args: vec![recv] };
                return (e, self.expect(want, Some(if m.is_test() { Ty::bool() } else { t })));
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
                    self.error(
                        Reason::ConstructShape,
                        format!(
                            "`{}.{}` takes {} argument(s) after the receiver, got {}",
                            ty.as_str(),
                            name.as_str(),
                            params.saturating_sub(1),
                            args.len()
                        ),
                    );
                }
                let (param_tys, ret) =
                    self.defs.methods.get(&(ty.as_str(), name.as_str())).map(|f| sig(f)).unwrap_or_default();
                self.expect(param_tys.first(), Some(rt));
                let mut typed = vec![recv];
                for (a, p) in args.iter().zip(param_tys.iter().skip(1).map(Some).chain(std::iter::repeat(None))) {
                    typed.push(self.expr(a, p).0);
                }
                let e = Expr::Call { callee: Callee::Method { ty, name: name.clone() }, args: typed };
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
                        Diagnostic::at(
                            self.item,
                            Reason::NeedsAnnotation,
                            format!(
                                "the receiver type of `.{}()` is not known here; annotate the binding",
                                name.as_str()
                            ),
                        )
                        .about(name.as_str()),
                    );
                }
                (receiver.clone(), None)
            }
        }
    }

    /// `e as T`: a fieldless enum to an integer type that holds every
    /// discriminant, looked up in a table; an unsigned integer of 32 bits or
    /// fewer to `usize`, which holds every value on every target (std has no
    /// `usize::from(u32)`, so `as` is how Rust writes it); or one integer type
    /// to another that std does not widen to with `From`, wrapping as Rust
    /// does. A widening `From` takes is written `T::from(x)`, its one
    /// spelling. Everything else keeps its old rejection.
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
        let source = t.as_ref().and_then(|t| match self.norm(t) {
            Ty::Prim(p) => p.int(),
            _ => None,
        });
        if let (Some(from @ (IntTy::U8 | IntTy::U16 | IntTy::U32)), Some(IntTy::Usize)) = (source, target) {
            let e = Expr::Call { callee: Callee::IntFrom { from: Some(from), to: IntTy::Usize }, args: vec![e] };
            return (e, self.expect(want, Some(to.clone())));
        }
        // With a float on either side: toward zero and saturating into an
        // integer, to the nearest into a float; what std's `From` takes is
        // written `T::from(x)`, as between integers.
        let to_num = match to {
            Ty::Prim(p) => p.int().map(Num::Int).or(p.float().map(Num::Float)),
            _ => None,
        };
        let from_num = t.as_ref().and_then(|t| self.num(t));
        let floats = (matches!(from_num, Some(Num::Float(_))), matches!(to_num, Some(Num::Float(_))));
        if let (Some(from), Some(into), (true, _) | (_, true)) = (from_num, to_num, floats) {
            let callee = match (from, into) {
                (Num::Float(f), Num::Int(i)) => Some(Callee::FloatToInt { from: f, to: i }),
                (Num::Int(i), Num::Float(f)) if super::prim::float_widens(i, f) => None,
                (Num::Int(i), Num::Float(f)) => Some(Callee::IntToFloat { from: i, to: f }),
                (Num::Float(a), Num::Float(b)) if a == b => return (e, self.expect(want, Some(to.clone()))),
                (Num::Float(_), Num::Float(FloatTy::F32)) => Some(Callee::FloatToFloat { to: FloatTy::F32 }),
                _ => None,
            };
            return match callee {
                Some(callee) => (Expr::Call { callee, args: vec![e] }, self.expect(want, Some(to.clone()))),
                None => {
                    let (fs, ts) = (num_name(from), num_name(into));
                    self.error(
                        Reason::Cast,
                        format!("`{fs} as {ts}` widens, which v0 writes `{ts}::from(x)`; `as` is for what `from` does not take"),
                    );
                    (e, None)
                }
            };
        }
        // Between integers: what std widens is written `T::from(x)`, the one
        // spelling of a lossless conversion; everything else wraps as Rust's
        // `as` does.
        if let (Some(from), Some(it)) = (source, target) {
            if from == it {
                return (e, self.expect(want, Some(to.clone())));
            }
            if from.widens_to(it) {
                self.error(
                    Reason::Cast,
                    format!(
                        "`{} as {}` widens, which v0 writes `{}::from(x)`; `as` between integers is for what `from` does not take",
                        from.as_str(),
                        it.as_str(),
                        it.as_str()
                    ),
                );
                return (e, None);
            }
            let e = Expr::Call { callee: Callee::IntCast { from, to: it }, args: vec![e] };
            return (e, self.expect(want, Some(to.clone())));
        }
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
                            Expr::Construct { variant: Some(v), .. } => {
                                table.iter().find(|(n, _)| n == v).map(|(_, d)| *d)
                            }
                            _ => None,
                        };
                        let folded = match known {
                            Some(value) => Expr::Lit(Lit::Int { value, ty: Some(it), byte: false, hex: false }),
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
                    "`{what} as {}` is not in v0: `as` reads a fieldless enum's discriminant, or converts between numeric types; nothing else is",
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
}

/// The allow-listed methods on a std receiver, for a rejection message;
/// `None` for the crate's own types, whose methods are its `impl` blocks.
fn std_methods(ty: &Ty) -> Option<String> {
    let names: Vec<&str> = match ty {
        Ty::Named(_) => return None,
        Ty::Prim(Prim::String) => StrMethod::ALL
            .iter()
            .map(|m| m.name())
            .chain(["as_bytes", "cmp", "parse", "clone", "chars", "bytes", "split", "slicing `s[a..b]`"])
            .collect(),
        Ty::Prim(Prim::Str) => StrMethod::ALL
            .iter()
            .filter(|m| **m != StrMethod::AsStr)
            .map(|m| m.name())
            .chain(["as_bytes", "cmp", "parse", "clone", "chars", "bytes", "split", "slicing `s[a..b]`"])
            .collect(),
        Ty::Prim(Prim::Char) => CharMethod::ALL.iter().map(|m| m.name()).chain(["cmp"]).collect(),
        Ty::Prim(Prim::Bool | Prim::Uuid) => vec!["cmp"],
        Ty::Vec(_) => vec![
            "len",
            "is_empty",
            "cmp",
            "clone",
            "iter",
            "into_iter",
            "push, insert, remove, `xs[i] = x`, sort, sort_by, and sort_by_key (on a `let mut` local)",
            "indexing `xs[i]`",
            "slicing `xs[a..b]`",
        ],
        Ty::Option(_) => {
            vec!["is_some", "is_none", "unwrap_or", "ok_or", "map", "clone", "cloned", "copied", "as_ref", "as_deref"]
        }
        Ty::Result { .. } => vec!["ok", "map", "map_err"],
        Ty::Prim(p) if p.float().is_some() => FloatMethod::ALL.iter().map(|m| m.name()).collect(),
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
            let list = names
                .iter()
                .map(|n| if n.contains(' ') { n.to_string() } else { format!("`{n}`") })
                .collect::<Vec<_>>()
                .join(", ");
            match ty {
                Ty::Prim(Prim::U8) => {
                    format!("{list}; for `u8::is_ascii_*` use `matches!(b, b'0'..=b'9')` or `char::from(b)`")
                }
                _ => list,
            }
        }
    })
}

/// `Ordering`'s allow-list, for a rejection message.
fn ordering_methods() -> String {
    ORDERING_METHODS.iter().map(|n| format!("`{n}`")).collect::<Vec<_>>().join(", ")
}

fn num_name(n: Num) -> &'static str {
    match n {
        Num::Int(t) => t.as_str(),
        Num::Float(t) => t.as_str(),
    }
}
