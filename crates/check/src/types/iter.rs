//! Iterator chains: `sum`, `count`, `all`, `fold`, `collect`, `map(..).unwrap_or(..)`,
//! and the sequences they consume.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// `all`, `any`, `position`, `count`, and `sum` on `s.chars()`,
    /// `s.bytes()`, or `xs.iter()` / `xs.into_iter()`: the loop std's
    /// default methods run, with the source bound once and the predicate
    /// inlined into the body (so it may not use `?` or `return`, which would
    /// leave the enclosing function). Each stops where std's does: `all` at
    /// the first `false`, `any` and `position` at the first `true`. `sum`
    /// adds from zero in order, panicking on overflow as a debug build does;
    /// on integers only, since float `Sum` starts from `-0.0`.
    pub(super) fn consume(&mut self, receiver: &Expr, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let takes = match name {
            "fold" => 2,
            "all" | "any" | "position" | "find" | "max_by_key" | "min_by_key" | "max_by" | "min_by" => 1,
            "count" | "sum" | "max" | "min" => 0,
            _ => return None,
        };
        let failed = (Expr::Lit(Lit::Unit), None);
        if !is_sequence(receiver) {
            return None;
        }
        if args.len() != takes {
            self.error(Reason::ConstructShape, format!("`{name}` takes {takes} argument(s), got {}", args.len()));
            return Some(failed);
        }
        let Some((source, over, item)) = self.sequence(receiver, name) else {
            return Some(failed);
        };
        if matches!(name, "find" | "max" | "min" | "max_by_key" | "min_by_key" | "max_by" | "min_by")
            && matches!(self.norm(&item), Ty::Option(_))
        {
            self.error(
                Reason::NestedOption,
                format!(
                    "`{name}` over `{}` items is `Option<Option<_>>`, and both `None`s are `null` in TS",
                    show(&item)
                ),
            );
            return Some(failed);
        }
        if name == "fold" {
            return Some(self.fold(source, over, item, args, want));
        }
        let mut call_args = vec![source];
        let method = match name {
            "max_by" | "min_by" => {
                let ordering = Ty::named(purecrate_ir::ORDERING);
                let f = Ty::Fn { params: vec![item.clone(), item.clone()], ret: Box::new(ordering) };
                if !matches!(args[0].unpositioned(), Expr::Closure { params, .. } if params.len() == 2) {
                    self.error(Reason::Closure, format!("`{name}` takes a closure `|a, b| ..` in v0"));
                    return Some(failed);
                }
                // The closure stays an arrow (`Iter.maxBy(xs, (a, b) => ..)`),
                // so a `return` in it leaves it, as in Rust.
                let (closure, _) = self.expr(&args[0], Some(&f));
                call_args.push(closure);
                if name == "max_by" {
                    Consume::MaxBy
                } else {
                    Consume::MinBy
                }
            }
            "max" | "min" => match self.ordered_by_any(&item, name) {
                Some(Ok(text)) if name == "max" => Consume::Max { text },
                Some(Ok(text)) => Consume::Min { text },
                // `Iter.maxBy(xs, by)`, as `max_by` with the comparator.
                Some(Err(by)) => {
                    call_args.push(self.comparator_value(by));
                    if name == "max" {
                        Consume::MaxBy
                    } else {
                        Consume::MinBy
                    }
                }
                None => return Some(failed),
            },
            _ => {
                let pred = match args.first() {
                    None => None,
                    Some(arg) => match self.one_param_fn(name, arg) {
                        Some(f) => Some(f),
                        None => return Some(failed),
                    },
                };
                let keyed = matches!(name, "max_by_key" | "min_by_key");
                let mut key = None;
                if let Some((param, ty, body)) = pred {
                    if !self.param_fits(name, &ty, &item) {
                        return Some(failed);
                    }
                    let closure = Expr::Closure {
                        params: vec![ClosureParam { name: param, ty: Some(item.clone()) }],
                        ret: if keyed { None } else { Some(Ty::bool()) },
                        body: Box::new(body),
                    };
                    let (closure, ct) = self.expr(&closure, None);
                    key = match ct.map(|t| self.norm(&t)) {
                        Some(Ty::Fn { ret, .. }) => Some(*ret),
                        _ => None,
                    };
                    call_args.push(closure);
                }
                match name {
                    "all" => Consume::All,
                    "any" => Consume::Any,
                    "position" => Consume::Position,
                    "find" => Consume::Find,
                    "count" => Consume::Count,
                    "max_by_key" | "min_by_key" => {
                        let Some(key) = key else { return Some(failed) };
                        let text = match self.ordered_by_any(&key, name) {
                            Some(Ok(text)) => text,
                            // The comparator follows the key.
                            Some(Err(by)) => {
                                call_args.push(self.comparator_value(by));
                                false
                            }
                            None => return Some(failed),
                        };
                        if name == "max_by_key" {
                            Consume::MaxByKey { text }
                        } else {
                            Consume::MinByKey { text }
                        }
                    }
                    _ => match int_of(&self.norm(&item)) {
                        Some(it) => Consume::Sum(it),
                        None => {
                            self.error(
                                Reason::TypeMismatch,
                                format!(
                                    "`sum` adds integers in v0, found `{}` (a float sum starts from `-0.0`)",
                                    show(&item)
                                ),
                            );
                            return Some(failed);
                        }
                    },
                }
            }
        };
        let ty = match method {
            Consume::All | Consume::Any => Ty::bool(),
            Consume::Position => Ty::option(Ty::Prim(Prim::Usize)),
            Consume::Count => Ty::Prim(Prim::Usize),
            Consume::Sum(_) => item.clone(),
            _ => Ty::option(item.clone()),
        };
        let e = Expr::Call { callee: Callee::Consume { method, over }, args: call_args };
        Some((e, self.expect(want, Some(ty))))
    }

    /// `fold(init, f)` on a typed sequence: `init` runs once the source is
    /// made, before any item; then `f(acc, x)` for each item in order. The
    /// accumulator's type is `init`'s, which an unsuffixed number does not
    /// give without a type from the context: Rust infers it from `f`.
    fn fold(&mut self, source: Expr, over: Over, item: Ty, args: &[Expr], want: Option<&Ty>) -> Typed {
        let failed = (Expr::Lit(Lit::Unit), None);
        let unsuffixed = |e: &Expr| {
            let e = match e.unpositioned() {
                Expr::Unary { op: purecrate_ir::UnOp::Neg, expr } => expr.unpositioned(),
                e => e,
            };
            matches!(e, Expr::Lit(Lit::Int { ty: None, .. } | Lit::Float { ty: None, .. }))
        };
        if want.is_none() && unsuffixed(&args[0]) {
            self.error(
                Reason::NeedsAnnotation,
                "`fold`'s start is an unsuffixed number, whose type Rust infers from the closure: write its type (`0u64`) or type the result".to_string(),
            );
            return failed;
        }
        let (init, acc) = self.expr(&args[0], want);
        let Some(acc) = acc else { return failed };
        let f = match args[1].unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 2 => {
                if self.exits_reported("`fold`", body, "write the loop") {
                    return failed;
                }
                let mut named = |p: &ClosureParam| {
                    if p.name.as_str() == "_" {
                        self.fresh_name("x")
                    } else {
                        p.name.clone()
                    }
                };
                let (a, x) = (named(&params[0]), named(&params[1]));
                Expr::Closure {
                    params: vec![
                        ClosureParam { name: a, ty: Some(acc.clone()) },
                        ClosureParam { name: x, ty: Some(item.clone()) },
                    ],
                    ret: Some(acc.clone()),
                    body: body.clone(),
                }
            }
            _ => {
                self.error(Reason::Closure, "`fold` takes a closure `|acc, x| ..` in v0".to_string());
                return failed;
            }
        };
        let (f, _) = self.expr(&f, None);
        let e = Expr::Call { callee: Callee::Consume { method: Consume::Fold, over }, args: vec![source, init, f] };
        (e, self.expect(want, Some(acc)))
    }

    /// What an iterator method takes as its function: a closure of one
    /// parameter, or a function name as `|$x| f($x)`. The parameter, its
    /// written type, and the body; `None` once reported. The closure is
    /// inlined into a loop, so it may not use `?` or `return`, which would
    /// leave the enclosing function.
    /// `E::V` written where a function goes, `V` a one-field tuple variant:
    /// what builds `E::V(x)` from `x`.
    pub(super) fn variant_fn(&self, e: &Expr) -> Option<impl std::ops::Fn(Expr) -> Expr> {
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

    pub(super) fn one_param_fn(&mut self, method: &str, arg: &Expr) -> Option<(Name, Option<Ty>, Expr)> {
        match arg.unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 1 => {
                if self.exits_reported(&format!("`{method}`"), body, "write the loop") {
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

    /// Whether the type written on the parameter of `stage`'s closure, if
    /// any, is the items' type, aliases followed; reported when not.
    fn param_fits(&mut self, stage: &str, written: &Option<Ty>, item: &Ty) -> bool {
        match written {
            Some(w) if !self.same(w, item) => {
                self.error(
                    Reason::TypeMismatch,
                    format!("`{stage}`'s closure takes `{}`, the items are `{}`", show(w), show(item)),
                );
                false
            }
            _ => true,
        }
    }

    /// `s.split(c).collect()` and `s.split(c).map(f).collect()` into the
    /// `Vec<T>` or `Result<Vec<T>, E>` the context names (a turbofish, a
    /// `let` type, or the return type). The one way to build a `Vec` whose
    /// length is not in the source: once, from text, so the length is the
    /// input's. Collecting a `Vec` or a state is how a sequence grows, and
    /// stays out (design/07 §6). `None` when `name` is not `collect`.
    pub(super) fn collect(&mut self, receiver: &Expr, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        if name != "collect" {
            return None;
        }
        let failed = (Expr::Lit(Lit::Unit), None);
        if !args.is_empty() {
            self.error(Reason::ConstructShape, format!("`collect` takes no arguments, got {}", args.len()));
            return Some(failed);
        }
        // The last `map(f)` stays apart: into a `Result`, `f` is what may fail.
        let (pieces, map) = match receiver.unpositioned() {
            Expr::MethodCall { receiver: inner, name: m, args: map_args }
                if m.as_str() == "map" && map_args.len() == 1 && is_sequence(inner) =>
            {
                (inner.unpositioned(), Some(&map_args[0]))
            }
            other => (other, None),
        };
        if !is_sequence(pieces) {
            self.error(Reason::MethodCall, "`collect` builds a `Vec` from `xs.iter()`, `s.chars()`, `s.bytes()`, `s.split(p)`, `s.split_whitespace()`, or `s.lines()`, through any `map(f)` and `filter(p)`, in v0".to_string());
            return Some(failed);
        }
        let target = want.map(|t| self.norm(t));
        // Into a `String`: the chars collected, then joined.
        let into_string = matches!(target, Some(Ty::Prim(Prim::String)));
        // Into a `BTreeMap` (from pairs, a later key's value replacing an
        // earlier one's) or a `BTreeSet`: the items collected, then built.
        let into_tree = match &target {
            Some(Ty::Map { .. }) => Some(TreeOp::FromPairs),
            Some(Ty::Set(_)) => Some(TreeOp::FromKeys),
            _ => None,
        };
        let (result, elem, err) = match target {
            Some(Ty::Vec(t)) => (false, *t, None),
            Some(Ty::Prim(Prim::String)) => (false, Ty::Prim(Prim::Char), None),
            Some(Ty::Map { key, value }) => (false, Ty::Tuple(vec![*key, *value]), None),
            Some(Ty::Set(key)) => (false, *key, None),
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
                self.error(
                    Reason::TypeMismatch,
                    format!("`collect` builds a `Vec<T>` or a `Result<Vec<T>, E>` in v0, not `{}`", show(&other)),
                );
                return Some(failed);
            }
        };
        let Some((source, over, item)) = self.sequence(pieces, "collect") else {
            return Some(failed);
        };
        let hole = |t: &Ty| matches!(t, Ty::Named(n) if n.as_str() == "_");
        let Some(map) = map else {
            if err.as_ref().is_some_and(hole) || result {
                self.error(
                    Reason::TypeMismatch,
                    "collecting into a `Result` takes `.map(f)` with `f` returning a `Result`".to_string(),
                );
                return Some(failed);
            }
            let e = Expr::Call { callee: Callee::Collect { result, over }, args: vec![source] };
            if into_string {
                return Some(self.joined_chars(e, &item));
            }
            if let Some(op) = into_tree {
                return Some(self.tree_from(op, e, &item, want));
            }
            // The items are what the sequence gives, whatever the target says.
            let pieces = Ty::Vec(Box::new(item));
            return Some((e, self.expect(want.filter(|t| !super::has_hole(t)), Some(pieces))));
        };
        let Some((param, written, body)) = self.one_param_fn("map", map) else {
            return Some(failed);
        };
        if !self.param_fits("map", &written, &item) {
            return Some(failed);
        }
        // A hole is what `f` returns: its `Ok` and `Err` for a `Result`.
        let (elem, err) = if hole(&elem) || err.as_ref().is_some_and(hole) {
            let returned = self.returns_of(&body, &param, &item)?;
            match (&err, self.norm(&returned)) {
                (Some(e), Ty::Result { ok, err: re }) => {
                    let ok = if hole(&elem) { *ok } else { elem };
                    let e = if hole(e) { *re } else { e.clone() };
                    (ok, Some(e))
                }
                (Some(_), other) => {
                    self.error(
                        Reason::TypeMismatch,
                        format!(
                            "collecting into a `Result` takes `.map(f)` with `f` returning a `Result`, not `{}`",
                            show(&other)
                        ),
                    );
                    return Some(failed);
                }
                (None, other) => (other, None),
            }
        } else {
            (elem, err)
        };
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
        let e = Expr::Call { callee: Callee::Collect { result, over }, args: vec![source, closure] };
        if into_string {
            return Some(self.joined_chars(e, &elem));
        }
        if let Some(op) = into_tree {
            return Some(self.tree_from(op, e, &elem, want));
        }
        let filled = if result {
            Ty::Result { ok: Box::new(Ty::Vec(Box::new(elem))), err: Box::new(err.expect("a Result has its error")) }
        } else {
            Ty::Vec(Box::new(elem))
        };
        Some((e, Some(filled)))
    }

    /// Collected `items` as a map (`item` a `(String, V)` pair) or a set
    /// (`item` a `String`), checked against the target the context names.
    fn tree_from(&mut self, op: TreeOp, items: Expr, item: &Ty, want: Option<&Ty>) -> Typed {
        let got = match (op, self.norm(item)) {
            (TreeOp::FromPairs, Ty::Tuple(kv)) if kv.len() == 2 => {
                Some(Ty::Map { key: Box::new(kv[0].clone()), value: Box::new(kv[1].clone()) })
            }
            (TreeOp::FromKeys, k) => Some(Ty::Set(Box::new(k))),
            _ => None,
        };
        let Some(got) = got else {
            self.error(
                Reason::TypeMismatch,
                format!("`collect` builds a `BTreeMap` from `(key, value)` pairs, found `{}`", show(item)),
            );
            return (Expr::Lit(Lit::Unit), None);
        };
        // The keys `resolve` takes in a written type (design/01 §7.26).
        let key_ok = |t: &Ty| matches!(t, Ty::Map { key, .. } | Ty::Set(key) if crate::keys::key(self.defs, key).is_ok());
        if !key_ok(&got) {
            self.error(
                Reason::TypeMismatch,
                format!("`collect` into a map or set takes keys of an integer, `char`, `bool`, `Uuid`, `String`, or a newtype of one, found `{}`", show(&got)),
            );
            return (Expr::Lit(Lit::Unit), None);
        }
        let e = Expr::Call { callee: Callee::Tree(op), args: vec![items] };
        (e, self.expect(want, Some(got)))
    }

    /// `chars` (a `Vec<char>`) as one `String`; `item` must be `char`.
    fn joined_chars(&mut self, chars: Expr, item: &Ty) -> Typed {
        if self.norm(item) != Ty::Prim(Prim::Char) {
            self.error(
                Reason::TypeMismatch,
                format!("`collect` builds a `String` from `char`s in v0, not `{}`", show(item)),
            );
            return (Expr::Lit(Lit::Unit), None);
        }
        (Expr::Call { callee: Callee::StrFromChars, args: vec![chars] }, Some(Ty::Prim(Prim::String)))
    }

    /// What a consumer or `collect` walks: `s.chars()`, `s.bytes()`,
    /// `xs.iter()` / `xs.into_iter()`, or `s.split(c)`, through any
    /// `.map(f)`, `.filter(p)`, `.copied()`, and `.cloned()` (design/01
    /// §7.13). Typed: the source, what it walks, and its items. `None` after
    /// an error; `expr` is one by `is_sequence`.
    pub(super) fn sequence(&mut self, expr: &Expr, consumer: &str) -> Option<(Expr, Over, Ty)> {
        if let Expr::Call { callee: Callee::Range { inclusive, .. }, args } = expr.unpositioned() {
            return self.range(*inclusive, args);
        }
        let Expr::MethodCall { receiver, name, args } = expr.unpositioned() else { unreachable!("is_sequence") };
        match (name.as_str(), args.as_slice()) {
            ("copied" | "cloned", []) => self.sequence(receiver, consumer),
            // A range's items, reversed or stepped as they are walked:
            // computed, never collected (design/01 §7.22).
            ("rev", []) | ("step_by", [_]) if is_range_based(receiver) => {
                let (source, over, item) = self.sequence(receiver, consumer)?;
                let e = match args.first() {
                    None => Expr::Call { callee: Callee::RangeRev, args: vec![source] },
                    Some(k) => {
                        let (k, _) = self.expr(k, Some(&Ty::Prim(Prim::Usize)));
                        Expr::Call { callee: Callee::RangeStep, args: vec![source, k] }
                    }
                };
                Some((e, over, item))
            }
            ("step_by", [_]) => {
                self.error(
                    Reason::MethodCall,
                    "`step_by` steps a range in v0 (`(a..b).step_by(k)`, `(a..=b).rev().step_by(k)`), not a list or a string's items".to_string(),
                );
                None
            }
            // On a source only, so no stage runs out of Rust's order: the
            // items reversed, or paired up to the shorter side.
            ("rev" | "zip", _) => {
                let (source, over, item) = self.sequence(receiver, consumer)?;
                if name.as_str() == "zip" && typed_range(&source) {
                    self.error(Reason::MethodCall, ZIP_RANGE.to_string());
                    return None;
                }
                let source = items(source, over);
                if name.as_str() == "rev" {
                    return Some((Expr::Call { callee: Callee::VecRev, args: vec![source] }, Over::Items, item));
                }
                let other = &args[0];
                let (other, other_item) = if is_sequence(other) {
                    let (o, over, t) = self.sequence(other, consumer)?;
                    if typed_range(&o) {
                        self.error(Reason::MethodCall, ZIP_RANGE.to_string());
                        return None;
                    }
                    (items(o, over), t)
                } else {
                    let before = self.out.len();
                    let (o, t) = self.expr(other, None);
                    match t.map(|t| self.norm(&t)) {
                        Some(Ty::Vec(t)) => (o, *t),
                        found => {
                            if self.out.len() == before {
                                let found = found.as_ref().map(show).unwrap_or_else(|| "?".into());
                                self.error(
                                    Reason::TypeMismatch,
                                    format!("`zip` takes a `Vec`, a slice, or `xs.iter()`, `s.chars()`, or `s.bytes()`, found `{found}`"),
                                );
                            }
                            return None;
                        }
                    }
                };
                let pair = Ty::Tuple(vec![item, other_item]);
                Some((Expr::Call { callee: Callee::VecZip, args: vec![source, other] }, Over::Items, pair))
            }
            (stage @ ("map" | "filter"), [f]) => {
                let (source, over, item) = self.sequence(receiver, consumer)?;
                let (param, written, body) = self.one_param_fn(stage, f)?;
                if !self.param_fits(stage, &written, &item) {
                    return None;
                }
                let ret = if stage == "map" { self.returns_of(&body, &param, &item)? } else { Ty::bool() };
                let closure = Expr::Closure {
                    params: vec![ClosureParam { name: param, ty: Some(item.clone()) }],
                    ret: Some(ret.clone()),
                    body: Box::new(body),
                };
                let (closure, _) = self.expr(&closure, None);
                let (callee, out) =
                    if stage == "map" { (Callee::IterMap { over }, ret) } else { (Callee::IterFilter { over }, item) };
                Some((Expr::Call { callee, args: vec![source, closure] }, Over::Items, out))
            }
            // `f` returns an `Option` (`filter_map`, `flat_map`) or a `Vec`
            // (`flat_map`): the `Some`s' values, or each `Vec`'s elements.
            (stage @ ("filter_map" | "flat_map"), [f]) => {
                let (source, over, item) = self.sequence(receiver, consumer)?;
                let (param, written, body) = self.one_param_fn(stage, f)?;
                if !self.param_fits(stage, &written, &item) {
                    return None;
                }
                let ret = self.stage_returns(stage, &body, &param, &item)?;
                let out = self.flattened(stage, &ret)?;
                let closure = Expr::Closure {
                    params: vec![ClosureParam { name: param, ty: Some(item) }],
                    ret: Some(ret.clone()),
                    body: Box::new(body),
                };
                let (closure, _) = self.expr(&closure, None);
                let e = match self.norm(&ret) {
                    Ty::Option(_) => Expr::Call { callee: Callee::IterFilterMap { over }, args: vec![source, closure] },
                    _ => {
                        let mapped = Expr::Call { callee: Callee::IterMap { over }, args: vec![source, closure] };
                        Expr::Call { callee: Callee::IterFlatten { over: Over::Items }, args: vec![mapped] }
                    }
                };
                Some((e, Over::Items, out))
            }
            // Over `Vec` items, their elements; over `Option` items, the
            // `Some`s' values, as `filter_map` of the item itself.
            ("flatten", []) => {
                let (source, over, item) = self.sequence(receiver, consumer)?;
                let out = self.flattened("flatten", &item)?;
                let e = match self.norm(&item) {
                    Ty::Option(_) => {
                        let x = self.fresh_name("x");
                        let closure = Expr::Closure {
                            params: vec![ClosureParam { name: x.clone(), ty: Some(item.clone()) }],
                            ret: Some(item),
                            body: Box::new(Expr::Var(x)),
                        };
                        let (closure, _) = self.expr(&closure, None);
                        Expr::Call { callee: Callee::IterFilterMap { over }, args: vec![source, closure] }
                    }
                    _ => Expr::Call { callee: Callee::IterFlatten { over }, args: vec![source] },
                };
                Some((e, Over::Items, out))
            }
            // The slices of a `Vec`, each `n` long: overlapping (`windows`)
            // or one after another, the last maybe shorter (`chunks`).
            (adaptor @ ("windows" | "chunks"), [n]) => {
                let before = self.out.len();
                let (xs, xt) = self.expr(receiver, None);
                let item = match xt.as_ref().map(|t| self.norm(t)) {
                    Some(Ty::Vec(t)) => Ty::Vec(t),
                    found => {
                        if self.out.len() == before {
                            let found = found.as_ref().map(show).unwrap_or_else(|| "?".into());
                            self.error(
                                Reason::TypeMismatch,
                                format!("`{adaptor}` takes a `Vec` or slice, found `{found}`"),
                            );
                        }
                        return None;
                    }
                };
                let (n, _) = self.expr(n, Some(&Ty::Prim(Prim::Usize)));
                let callee = Callee::VecWindows { chunks: adaptor == "chunks" };
                Some((Expr::Call { callee, args: vec![xs, n] }, Over::Items, item))
            }
            (adaptor, _) => {
                let (over, source) = match (adaptor, args.as_slice()) {
                    ("chars", []) => (Over::Chars, (**receiver).clone()),
                    ("bytes", []) => (Over::Bytes, (**receiver).clone()),
                    ("iter" | "into_iter" | "keys" | "values", []) => (Over::Items, (**receiver).clone()),
                    // `s.split_whitespace()` and `s.lines()`: the pieces, a new array.
                    ("split_whitespace" | "lines", []) => (Over::Items, expr.clone()),
                    // `s.split(c)` as a `for` gives it: the pieces, a `char` apart.
                    ("split", [sep]) => (
                        Over::Items,
                        Expr::Call { callee: Callee::StrSplit, args: vec![(**receiver).clone(), sep.clone()] },
                    ),
                    _ => unreachable!("is_sequence"),
                };
                let before = self.out.len();
                let (source, st) = self.expr(&source, None);
                // A map's or a set's items in key order, as a new array.
                let (source, st) = match (adaptor, st.as_ref().map(|t| self.norm(t))) {
                    ("iter" | "into_iter", Some(t @ Ty::Map { .. })) => self.tree_items(TreeOp::Entries, source, t),
                    ("iter" | "into_iter" | "keys", Some(t @ (Ty::Set(_) | Ty::Map { .. }))) => {
                        self.tree_items(TreeOp::Keys, source, t)
                    }
                    ("values", Some(t @ Ty::Map { .. })) => self.tree_items(TreeOp::Values, source, t),
                    ("keys" | "values", _) => (source, None),
                    _ => (source, st),
                };
                let item = match st.as_ref().and_then(|t| self.walked(over, t)) {
                    Some(item) => item,
                    None => {
                        if self.out.len() == before {
                            let found = st.as_ref().map(|t| show(&self.norm(t))).unwrap_or_else(|| "?".into());
                            self.error(
                                Reason::TypeMismatch,
                                format!(
                                    "`.{adaptor}().{consumer}()` takes a {}, found `{found}`",
                                    if over == Over::Items { "`Vec` or slice" } else { "`String` or `&str`" }
                                ),
                            );
                        }
                        return None;
                    }
                };
                Some((source, over, item))
            }
        }
    }

    /// Whether `expr` puts `rev`, `zip`, or `step_by` after a stage that
    /// runs code, reported.
    pub(super) fn reorder_reported(&mut self, expr: &Expr) -> bool {
        let Some(m) = stage_before_reorder(expr) else { return false };
        let message = if m == "step_by" {
            "`step_by` comes right after the range in v0 (`(a..b).step_by(k).map(f)`), before any `map` or `filter`, so each runs in Rust's order".to_string()
        } else {
            format!("`{m}` comes right after the source in v0 (`xs.iter().{m}(..).map(f)`), before any `map` or `filter`, so each runs in Rust's order")
        };
        self.error(Reason::MethodCall, message);
        true
    }

    /// What `flatten` gives of `t`, an item or `f`'s result for `filter_map`
    /// and `flat_map`: a `Vec`'s elements or an `Option`'s value (only an
    /// `Option` for `filter_map`). Rust takes any `IntoIterator`; v0 these
    /// two. `None` once reported.
    fn flattened(&mut self, stage: &str, t: &Ty) -> Option<Ty> {
        match self.norm(t) {
            Ty::Option(u) => Some(*u),
            Ty::Vec(u) if stage != "filter_map" => Some(*u),
            Ty::Never => None,
            other => {
                let what = match stage {
                    "filter_map" => "`filter_map`'s function returns an `Option`".to_string(),
                    "flat_map" => "`flat_map`'s function returns a `Vec` or an `Option` in v0".to_string(),
                    _ => "`flatten` takes items that are `Vec`s or `Option`s in v0".to_string(),
                };
                self.error(Reason::TypeMismatch, format!("{what}, found `{}`", show(&other)));
                None
            }
        }
    }

    /// `a..b` or `a..=b` as a sequence: the bounds of one integer type, an
    /// unsuffixed start taking the end's, as in a `for`. Each is evaluated
    /// once, `a` first.
    fn range(&mut self, inclusive: bool, args: &[Expr]) -> Option<(Expr, Over, Ty)> {
        let [start, end] = args else { unreachable!("`syntax` writes both bounds") };
        let before = self.out.len();
        let ((s, st), (e, et)) = if is_bare_int(start) {
            let (e, et) = self.expr(end, None);
            (self.expr(start, et.as_ref()), (e, et))
        } else {
            let (s, st) = self.expr(start, None);
            let e = self.expr(end, st.as_ref());
            ((s, st), e)
        };
        let bound = st.or(et);
        let Some(it) = bound.as_ref().and_then(|t| int_of(&self.norm(t))) else {
            if self.out.len() == before {
                let found = bound.as_ref().map(show).unwrap_or_else(|| "?".into());
                self.error(Reason::TypeMismatch, format!("a range `a..b` is of one integer type, found `{found}`"));
            }
            return None;
        };
        let range = Expr::Call { callee: Callee::Range { inclusive, ty: Some(it) }, args: vec![s, e] };
        Some((range, Over::Items, bound.expect("an integer type")))
    }

    /// The type `body` has with `param` bound to `item`, for a hole that
    /// `map(f)` fills; its diagnostics are left to the typing that follows.
    fn returns_of(&mut self, body: &Expr, param: &Name, item: &Ty) -> Option<Ty> {
        let ty = self.returned(body, param, item);
        if ty.is_none() {
            self.error(
                Reason::NeedsAnnotation,
                "the `_` in `collect::<..>()` is not known from `map`'s function; name the type".to_string(),
            );
        }
        ty
    }

    /// `returns_of` for `stage`'s function, which returns an `Option` or a
    /// `Vec`: an iterator there is named as such.
    fn stage_returns(&mut self, stage: &str, body: &Expr, param: &Name, item: &Ty) -> Option<Ty> {
        let ty = self.returned(body, param, item);
        if ty.is_none() {
            let what = if stage == "filter_map" { "an `Option`" } else { "a `Vec` or an `Option` in v0" };
            let not = if is_sequence(body) {
                ", not an iterator: collect it into a `Vec`"
            } else {
                "; its type is not known here"
            };
            self.error(Reason::TypeMismatch, format!("`{stage}`'s function returns {what}{not}"));
        }
        ty
    }

    /// The type `body` has with `param` bound to `item`, what it reports
    /// dropped.
    fn returned(&mut self, body: &Expr, param: &Name, item: &Ty) -> Option<Ty> {
        let before = self.out.len();
        self.scopes.push((param.as_str().to_string(), Some(item.clone())));
        let (_, ty) = self.expr(body, None);
        self.scopes.pop();
        self.out.truncate(before);
        ty
    }

    /// `o.map(f).unwrap_or(d)` as one `match o { Some(x) => f(x), None => d }`,
    /// with no `Option` in between, where `d` is a name or a literal: it has
    /// no effect, so running it only on `None` keeps Rust's behavior.
    pub(super) fn map_unwrap_or(
        &mut self,
        receiver: &Expr,
        name: &str,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
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
            Expr::Closure { params, body, .. } if params.len() == 1 && !body.exits() => {
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
}

/// `zip` on a range, or with one: a range is computed, not an array.
const ZIP_RANGE: &str = "`zip` pairs a `Vec`'s, a slice's, or a string's items in v0, not a range's; index the list with the range's value in a `for`";

/// Whether `expr` is a sequence `TypeCx::sequence` types: a source (with
/// `rev()`, `zip(..)`, or on a range `step_by(k)` on it), then any `map`,
/// `filter`, `filter_map`, `flat_map`, `flatten`, `copied`, and `cloned`.
pub(super) fn is_sequence(expr: &Expr) -> bool {
    if is_range(expr) {
        return true;
    }
    let Expr::MethodCall { receiver, name, args } = expr.unpositioned() else { return false };
    match (name.as_str(), args.len()) {
        ("copied" | "cloned" | "flatten", 0) | ("map" | "filter" | "filter_map" | "flat_map", 1) => {
            is_sequence(receiver)
        }
        ("rev", 0) | ("zip" | "step_by", 1) => is_source(receiver),
        ("chars" | "bytes" | "iter" | "into_iter" | "keys" | "values" | "split_whitespace" | "lines", 0)
        | ("split" | "windows" | "chunks", 1) => true,
        _ => false,
    }
}

/// What an iterator chain may end in (`TypeCx::consume`, `collect`), and the
/// stages before it.
pub(super) const CONSUMERS: &str = "`all`, `any`, `position`, `find`, `count`, `sum`, `max`, `min`, `max_by_key`, `min_by_key`, `max_by`, `min_by`, `fold`, and `collect`, after any `map`, `filter`, `filter_map`, `flat_map`, `flatten`, `copied`, `cloned`, `rev`, `zip`, and `step_by`; or a `for` over it";

/// A method that starts a sequence (`is_sequence`).
pub(super) fn is_source_name(name: &str) -> bool {
    matches!(
        name,
        "chars"
            | "bytes"
            | "iter"
            | "into_iter"
            | "keys"
            | "values"
            | "split_whitespace"
            | "lines"
            | "split"
            | "windows"
            | "chunks"
    )
}

/// A sequence with no stage that runs code: a source, `copied`, `cloned`,
/// `rev`, `zip`, and `step_by`.
fn is_source(expr: &Expr) -> bool {
    if is_range(expr) {
        return true;
    }
    let Expr::MethodCall { receiver, name, args } = expr.unpositioned() else { return false };
    match (name.as_str(), args.len()) {
        ("copied" | "cloned" | "rev", 0) | ("zip" | "step_by", 1) => is_source(receiver),
        ("chars" | "bytes" | "iter" | "into_iter" | "keys" | "values" | "split_whitespace" | "lines", 0)
        | ("split" | "windows" | "chunks", 1) => true,
        _ => false,
    }
}

/// `(a..b)` or `(a..=b)` as `syntax` writes a method's receiver.
fn is_range(expr: &Expr) -> bool {
    matches!(expr.unpositioned(), Expr::Call { callee: Callee::Range { .. }, .. })
}

/// A range, then any `rev()` and `step_by(k)`.
fn is_range_based(expr: &Expr) -> bool {
    match expr.unpositioned() {
        Expr::MethodCall { receiver, name, args } => {
            matches!((name.as_str(), args.len()), ("rev", 0) | ("step_by", 1)) && is_range_based(receiver)
        }
        e => is_range(e),
    }
}

/// Whether a typed sequence is a range's (`Iter.range`, then any `rev` and
/// `step_by`): computed as it is walked, not an array.
pub(super) fn typed_range(expr: &Expr) -> bool {
    matches!(expr, Expr::Call { callee: Callee::Range { .. } | Callee::RangeRev | Callee::RangeStep, .. })
}

/// Whether `expr` is `rev()`, `zip(..)`, or `step_by(..)` where a stage that
/// runs code comes before it, which v0 refuses.
pub(super) fn stage_before_reorder(expr: &Expr) -> Option<&'static str> {
    let Expr::MethodCall { receiver, name, args } = expr.unpositioned() else { return None };
    match (name.as_str(), args.len()) {
        ("rev", 0) if !is_source(receiver) && is_sequence(receiver) => Some("rev"),
        ("zip", 1) if !is_source(receiver) && is_sequence(receiver) => Some("zip"),
        ("step_by", 1) if !is_source(receiver) && is_sequence(receiver) => Some("step_by"),
        _ => stage_before_reorder(receiver),
    }
}

/// A typed source as an array: `chars()` and `bytes()` of a string
/// collected, a `Vec` or the pieces of `split` as they are.
fn items(source: Expr, over: Over) -> Expr {
    match over {
        Over::Items => source,
        Over::Chars => Expr::Call { callee: Callee::Collect { result: false, over: Over::Chars }, args: vec![source] },
        Over::Bytes => Expr::Call { callee: Callee::StrBytes, args: vec![source] },
    }
}
