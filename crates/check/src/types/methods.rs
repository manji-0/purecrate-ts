//! Method calls: the crate's own methods and the std allow-list, each
//! handed to `iter`, `option_result`, or `prim`; and `as`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// `x.m(args)` is `T::m(x, args)` for the crate's own `impl T`. The
    /// receiver is typed once, to find `T`, and the typed receiver is the
    /// call's first argument: typing it again would double the work at every
    /// link of a chain.
    pub(super) fn method_call(&mut self, receiver: &Expr, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        let whole = Expr::MethodCall { receiver: Box::new(receiver.clone()), name: name.clone(), args: args.to_vec() };
        if self.reorder_reported(&whole) {
            return (Expr::Lit(Lit::Unit), None);
        }
        // `xs.iter().rev()` and `a.iter().zip(b)` as a value (a `for`'s
        // source): the new array. A range's is not one.
        if matches!(name.as_str(), "rev" | "zip") && iter::is_sequence(&whole) {
            let Some((source, _, item)) = self.sequence(&whole, name.as_str()) else {
                return (Expr::Lit(Lit::Unit), None);
            };
            if iter::typed_range(&source) {
                self.error(Reason::Range, "a range's `rev()` is a sequence in v0, before `.step_by(k)`, `.map(f)`, a consumer, or `collect`, or in a `for`; it is not a value".to_string());
                return (Expr::Lit(Lit::Unit), None);
            }
            return (source, self.expect(want, Some(Ty::Vec(Box::new(item)))));
        }
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
        // A method no consumer takes, on an iterator chain
        // (`xs.into_iter().partition(f)`): the chain's source alone is no
        // value in v0, and its refusal (`.into_iter()` on `Vec<i32>`) would
        // blame a method the chain is allowed. The method is named instead.
        if self.out.len() > before
            && iter::is_sequence(receiver)
            && !iter::is_sequence(&whole)
            && self.out[before..]
                .iter()
                .all(|d| d.reason == Reason::MethodCall && d.detail.as_deref().is_some_and(iter::is_source_name))
        {
            self.out.truncate(before);
            self.out.push(
                Diagnostic::at(
                    self.item,
                    Reason::MethodCall,
                    format!(
                        "`.{}()` on an iterator is not on the std allow-list; allowed: {}",
                        name.as_str(),
                        iter::CONSUMERS
                    ),
                )
                .about(name.as_str()),
            );
            return (Expr::Lit(Lit::Unit), None);
        }
        self.method_call_typed(receiver, name, args, want, before, recv, rt)
    }

    /// The rest of `method_call`, once the receiver is typed: apart, so
    /// the frame each link of a chain holds while its receiver is typed
    /// (`self.expr` above) is small.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn method_call_typed(
        &mut self,
        receiver: &Expr,
        name: &Name,
        args: &[Expr],
        want: Option<&Ty>,
        before: usize,
        recv: Expr,
        rt: Option<Ty>,
    ) -> Typed {
        // `s.into()`: the `From` member of the type the context wants.
        if name.as_str() == "into" && args.is_empty() {
            return self.convert("`.into()`", None, receiver, Some((recv, rt)), want);
        }
        // The receiver's type with aliases followed, worked out once.
        let nt = rt.as_ref().map(|t| self.norm(t));
        if let Some(t @ (Ty::Map { .. } | Ty::Set(_))) = &nt {
            if let Some(typed) = self.tree_method(recv.clone(), t, name.as_str(), args, want) {
                return typed;
            }
        }
        // `clone` is a new array for a `Vec`, which a local may push to, and
        // the value itself for anything else, which nothing writes; `as_ref`
        // and `as_deref` on an `Option` are the option (design/01 §7.14), and
        // `as_slice` on a `Vec` the array, which nothing writes while the
        // slice is borrowed (§7.24).
        if args.is_empty()
            && matches!(name.as_str(), "clone" | "as_ref" | "as_deref" | "cloned" | "copied" | "as_slice")
        {
            if let (Some(rt), Some(nt)) = (&rt, &nt) {
                match (name.as_str(), nt) {
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
                    ("as_deref", Ty::Option(inner)) if matches!(self.norm(inner), Ty::Prim(Prim::String)) => {
                        return (recv, self.expect(want, Some(Ty::option(Ty::Prim(Prim::Str)))));
                    }
                    // `Option<Vec<T>>` to `Option<&[T]>`, and `Vec<T>` to `&[T]`:
                    // one type here, as every reference is the value.
                    ("as_deref", Ty::Option(inner)) if matches!(self.norm(inner), Ty::Vec(_)) => {
                        return (recv, self.expect(want, Some(rt.clone())));
                    }
                    ("as_slice", Ty::Vec(_)) => return (recv, self.expect(want, Some(rt.clone()))),
                    _ => {}
                }
            }
        }
        if let (
            Some(Ty::Vec(item)),
            "push" | "insert" | "remove" | "sort" | "sort_by" | "sort_by_key" | "dedup" | "dedup_by_key",
        ) = (&nt, name.as_str())
        {
            return self.vec_write(name.as_str(), recv, item, args, want);
        }
        // `s.push(c)` / `s.push_str(t)` on a local `let mut s: String`: the
        // assignment `s = s + c`, so every pass sees a write it knows.
        if matches!(name.as_str(), "push" | "push_str") && matches!(nt, Some(Ty::Prim(Prim::String))) {
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
        if let (Some(Ty::Vec(item)), "join") = (&nt, name.as_str()) {
            return self.join(recv, item, args, want);
        }
        if name.as_str() == "len" && args.is_empty() && matches!(nt, Some(Ty::Vec(_))) {
            let e = Expr::Call { callee: Callee::VecLen, args: vec![recv] };
            return (e, self.expect(want, Some(Ty::Prim(Prim::Usize))));
        }
        if let Some(Ty::Vec(item)) = &nt {
            if let Some(typed) = self.vec_access(recv.clone(), item, name.as_str(), args, want) {
                return typed;
            }
        }
        if let Some(nt) = nt.as_ref().filter(|_| args.is_empty()) {
            let callee = match (nt, name.as_str()) {
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
        if let Some(Ty::Option(inner)) = &nt {
            if let Some(typed) = self.option_method(recv.clone(), inner, name.as_str(), args, want) {
                return typed;
            }
        }
        if let Some(Ty::Result { ok, err }) = &nt {
            if let Some(typed) = self.result_method(recv.clone(), ok, err, name.as_str(), args, want) {
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
            if let Some(typed) = self.pick(recv.clone(), rt, name.as_str(), args, want) {
                return typed;
            }
        }
        let text = matches!(nt, Some(Ty::Prim(Prim::String | Prim::Str)));
        if name.as_str() == "parse" && args.is_empty() && text {
            return self.parse(recv, want);
        }
        if name.as_str() == "as_bytes" && args.is_empty() && text {
            let e = Expr::Call { callee: Callee::StrBytes, args: vec![recv] };
            return (e, self.expect(want, Some(Ty::Vec(Box::new(Ty::Prim(Prim::U8))))));
        }
        if name.as_str() == "repeat" && text {
            let [n] = args else {
                self.error(
                    Reason::ConstructShape,
                    format!("`str::repeat` takes 1 argument after the receiver, got {}", args.len()),
                );
                return (Expr::Lit(Lit::Unit), None);
            };
            let (n, _) = self.expr(n, Some(&Ty::Prim(Prim::Usize)));
            let e = Expr::Call { callee: Callee::StrRepeat, args: vec![recv, n] };
            return (e, self.expect(want, Some(Ty::Prim(Prim::String))));
        }
        if name.as_str() == "to_string" && args.is_empty() {
            if let Some(t @ Ty::Prim(_)) = &nt {
                if let Some(typed) = self.displayed_text(recv.clone(), t, want) {
                    return typed;
                }
            }
        }
        if let Some(m) = IntMethod::from_name(name.as_str()) {
            if let Some(it) = nt.as_ref().and_then(int_of) {
                return self.int_method(it, m, recv, args, want);
            }
        }
        if let Some(m) = FloatMethod::ALL.into_iter().find(|m| m.name() == name.as_str()) {
            if let Some(ft) = nt.as_ref().and_then(float_of) {
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
            if matches!(nt, Some(Ty::Prim(Prim::Char))) {
                return self.char_method(m, recv, args, want);
            }
        }
        if let Some(m) = StrMethod::from_name(name.as_str()) {
            // `str::as_str` is unstable; only `String` has one.
            if matches!(nt, Some(Ty::Prim(Prim::String))) || (text && m != StrMethod::AsStr) {
                return self.str_method(m, recv, args, want);
            }
        }
        let owner = nt.as_ref().and_then(|t| match t.applied() {
            Some((n, _)) => {
                let params = self.defs.methods.get(&(n.as_str(), name.as_str())).map(|f| f.params.len());
                params.map(|p| (n.clone(), p))
            }
            None => None,
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
                let f = self.defs.methods[&(ty.as_str(), name.as_str())];
                if !f.generics.is_empty() {
                    return self.generic_method(f, ty, name, recv, &rt, args, want);
                }
                let (param_tys, ret) = sig(f);
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
                    None if name.as_str() == "to_string" && self.crate_type(&rt).is_some() => {
                        format::no_display(".to_string()", &self.crate_type(&rt).expect("checked"))
                    }
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

    /// `xs.join(sep)` on a `Vec` or slice of `String`s or `&str`s, `sep` a
    /// `&str` or `String`: the items with `sep` between them, a new `String`.
    #[inline(never)]
    fn join(&mut self, recv: Expr, item: &Ty, args: &[Expr], want: Option<&Ty>) -> Typed {
        let failed = (Expr::Lit(Lit::Unit), None);
        let [sep] = args else {
            self.error(Reason::ConstructShape, format!("`join` takes 1 argument, got {}", args.len()));
            return failed;
        };
        let item = self.norm(item);
        if !matches!(item, Ty::Prim(Prim::String | Prim::Str) | Ty::Never) {
            self.error(
                Reason::MethodCall,
                format!(
                    "`join` on `Vec<{}>` is not in v0: it joins `String`s or `&str`s; make the text first (`.iter().map(|x| format!(\"{{x}}\")).collect::<Vec<String>>()`)",
                    show(&item)
                ),
            );
            return failed;
        }
        let (sep, st) = self.expr(sep, None);
        if let Some(t) =
            st.map(|t| self.norm(&t)).filter(|t| !matches!(t, Ty::Prim(Prim::String | Prim::Str) | Ty::Never))
        {
            self.error(Reason::TypeMismatch, format!("`join` takes a `&str` separator in v0, found `{}`", show(&t)));
            return failed;
        }
        let e = Expr::Call { callee: Callee::StrJoin, args: vec![recv, sep] };
        (e, self.expect(want, Some(Ty::Prim(Prim::String))))
    }

    /// `push`, `insert`, `remove`, `sort`, `sort_by`, and `sort_by_key` on a
    /// `Vec` of `item`s: each writes a local `let mut` array. Apart from
    /// `method_call_typed`, whose frame every link of a chain holds.
    #[inline(never)]
    fn vec_write(&mut self, method: &str, recv: Expr, item: &Ty, args: &[Expr], want: Option<&Ty>) -> Typed {
        let failed = (Expr::Lit(Lit::Unit), None);
        let (takes, arity, verb) = match method {
            "push" => (1, "1 argument", "grows"),
            "insert" => (2, "2 arguments", "grows"),
            "remove" => (1, "1 argument", "takes from"),
            "sort" => (0, "0 argument(s)", "sorts"),
            "dedup" => (0, "0 argument(s)", "dedups"),
            "dedup_by_key" => (1, "1 argument(s)", "dedups"),
            _ => (1, "1 argument(s)", "sorts"),
        };
        if args.len() != takes {
            self.error(Reason::ConstructShape, format!("`{method}` takes {arity}, got {}", args.len()));
            return failed;
        }
        if !matches!(recv.unpositioned(), Expr::Var(_)) {
            self.error(Reason::MethodCall, format!("`{method}` {verb} a local `let mut v: Vec<T>` in v0, not a field or an element: build the new `Vec` and put it in a new value (design/02 §3.1)"));
            return failed;
        }
        let usize_ = Ty::Prim(Prim::Usize);
        let (callee, args, ty) = match method {
            "push" => {
                let (arg, _) = self.expr(&args[0], Some(item));
                (Callee::VecPush, vec![recv, arg], Ty::Prim(Prim::Unit))
            }
            "insert" => {
                let (at, _) = self.expr(&args[0], Some(&usize_));
                let (arg, _) = self.expr(&args[1], Some(item));
                (Callee::VecInsert, vec![recv, at, arg], Ty::Prim(Prim::Unit))
            }
            "remove" => {
                let (at, _) = self.expr(&args[0], Some(&usize_));
                (Callee::VecRemove, vec![recv, at], item.clone())
            }
            "dedup" | "dedup_by_key" => {
                let mut call = vec![recv];
                let compared = match args.first() {
                    None => item.clone(),
                    Some(f) => {
                        let Some((param, written, body)) = self.one_param_fn(method, f) else { return failed };
                        let ty = written.or(Some(item.clone()));
                        let closure = Expr::Closure {
                            params: vec![ClosureParam { name: param, ty }],
                            ret: None,
                            body: Box::new(body),
                        };
                        let (closure, ct) = self.expr(&closure, None);
                        call.push(closure);
                        match ct.map(|t| self.norm(&t)) {
                            Some(Ty::Fn { ret, .. }) => *ret,
                            _ => return failed,
                        }
                    }
                };
                if !self.deep_comparable(&compared, &mut Vec::new()) {
                    self.error(
                        Reason::Comparison,
                        format!("`{method}` compares with `==`, and `{}` holds a value JS cannot compare as Rust does (`ParseIntError`, `uuid::Error`, a function)", show(&compared)),
                    );
                    return failed;
                }
                (Callee::VecDedup, call, Ty::Prim(Prim::Unit))
            }
            _ => {
                let named = args.first().and_then(|a| self.named_fn(a, item.clone()));
                let args: &[Expr] = match &named {
                    Some(f) => std::slice::from_ref(f),
                    None => args,
                };
                // The closure stays an arrow (`Slice.sortBy(v, (a, b) => ..)`),
                // so a `return` in it leaves it, as in Rust.
                if args.first().is_some_and(|a| !matches!(a.unpositioned(), Expr::Closure { .. })) {
                    self.error(Reason::Closure, format!("`{method}` takes a closure or a function name in v0"));
                    return failed;
                }
                let (sort, args) = match method {
                    "sort" => match self.ordered_by_any(item, method) {
                        Some(Ok(text)) => (Sort::Natural { text }, vec![recv]),
                        Some(Err(by)) => (Sort::By, vec![recv, comparator_value(by)]),
                        None => return failed,
                    },
                    "sort_by" => {
                        let ordering = Ty::named(purecrate_ir::ORDERING);
                        let f = Ty::Fn { params: vec![item.clone(), item.clone()], ret: Box::new(ordering) };
                        let (closure, _) = self.expr(&args[0], Some(&f));
                        (Sort::By, vec![recv, closure])
                    }
                    _ => {
                        // The parameter takes the item where its type is not written.
                        let Expr::Closure { params, body, ret } = args[0].unpositioned().clone() else {
                            unreachable!("checked above")
                        };
                        let params =
                            params.into_iter().map(|p| ClosureParam { ty: p.ty.or(Some(item.clone())), ..p }).collect();
                        let (closure, ct) = self.expr(&Expr::Closure { params, body, ret }, None);
                        let key = match ct.map(|t| self.norm(&t)) {
                            Some(Ty::Fn { ret, .. }) => *ret,
                            _ => return failed,
                        };
                        match self.ordered_by_any(&key, method) {
                            Some(Ok(text)) => (Sort::ByKey { text }, vec![recv, closure]),
                            // The comparator follows the key.
                            Some(Err(by)) => (Sort::ByKey { text: false }, vec![recv, closure, comparator_value(by)]),
                            None => return failed,
                        }
                    }
                };
                (Callee::VecSort(sort), args, Ty::Prim(Prim::Unit))
            }
        };
        (Expr::Call { callee, args }, self.expect(want, Some(ty)))
    }

    /// A method of `impl<T> Page<T>` called on a typed receiver: the
    /// receiver binds the impl's parameters, the arguments the method's own.
    /// Apart from `method_call`, whose frame every link of a chain holds.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn generic_method(
        &mut self,
        f: &Fn,
        ty: Name,
        name: &Name,
        recv: Expr,
        rt: &Ty,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Typed {
        let (param_tys, ret) = sig(f);
        let mut binds = Vec::new();
        if let Some(first) = param_tys.first() {
            self.unify(first, rt, &f.generics, &mut binds);
        }
        let what = format!("`{}.{}`", ty.as_str(), name.as_str());
        let ret = ret.unwrap_or(Ty::Prim(Prim::Unit));
        let (rest, t) =
            self.generic_args(&what, &f.generics, binds, param_tys.get(1..).unwrap_or(&[]), args, &ret, want);
        let e = Expr::Call {
            callee: Callee::Method { ty, name: name.clone() },
            args: std::iter::once(recv).chain(rest).collect(),
        };
        (e, self.expect(want, t))
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
        let target = int_of(to);
        let enum_def = t.as_ref().and_then(|t| match self.norm(t) {
            Ty::Named(n) => self.defs.enums.get(n.as_str()).copied(),
            _ => None,
        });
        let source = t.as_ref().and_then(|t| int_of(&self.norm(t)));
        if let (Some(from @ (IntTy::U8 | IntTy::U16 | IntTy::U32)), Some(IntTy::Usize)) = (source, target) {
            let e = Expr::Call { callee: Callee::IntFrom { from: Some(from), to: IntTy::Usize }, args: vec![e] };
            return (e, self.expect(want, Some(to.clone())));
        }
        // With a float on either side: toward zero and saturating into an
        // integer, to the nearest into a float; what std's `From` takes is
        // written `T::from(x)`, as between integers.
        let to_num = num_of(to);
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
        // `str::as_str` is unstable; only `String` has one.
        Ty::Prim(p @ (Prim::String | Prim::Str)) => StrMethod::ALL
            .iter()
            .filter(|m| *p == Prim::String || **m != StrMethod::AsStr)
            .map(|m| m.name())
            .chain([
                "as_bytes",
                "cmp",
                "parse",
                "clone",
                "chars",
                "bytes",
                "split",
                "repeat",
                "to_string",
                "slicing `s[a..b]`",
            ])
            .collect(),
        Ty::Prim(Prim::Char) => CharMethod::ALL.iter().map(|m| m.name()).chain(["cmp", "to_string"]).collect(),
        Ty::Prim(Prim::Bool | Prim::Uuid) => vec!["cmp", "to_string"],
        Ty::Vec(_) => vec![
            "len",
            "is_empty",
            "get",
            "first",
            "last",
            "contains",
            "cmp",
            "clone",
            "join (on `String`s or `&str`s)",
            "iter",
            "into_iter",
            "windows and chunks (before a consumer or in a `for`)",
            "push, insert, remove, `xs[i] = x`, sort, sort_by, sort_by_key, dedup, and dedup_by_key (on a `let mut` local)",
            "indexing `xs[i]`",
            "slicing `xs[a..b]`",
            "`as_slice`",
        ],
        Ty::Option(_) => ["is_some", "is_none"]
            .into_iter()
            .chain(option_result::OPTION_METHODS.map(|(n, _)| n))
            .chain(["clone", "cloned", "copied", "as_ref", "as_deref"])
            .collect(),
        Ty::Result { .. } => option_result::RESULT_METHODS.map(|(n, _)| n).to_vec(),
        Ty::Prim(p) if p.float().is_some() => FloatMethod::ALL.iter().map(|m| m.name()).collect(),
        Ty::Prim(p) if p.int().is_some() => IntMethod::ALL
            .iter()
            .filter(|m| **m != IntMethod::Abs || p.int().is_some_and(IntTy::is_signed))
            .map(|m| m.name())
            .chain(["cmp", "to_string"])
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
