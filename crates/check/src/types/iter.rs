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
            "all" | "any" | "position" => 1,
            "count" | "sum" => 0,
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
                    self.error(
                        Reason::TypeMismatch,
                        format!("`sum` adds integers in v0, found `{}` (a float sum starts from `-0.0`)", show(&item)),
                    );
                    return Some(failed);
                }
            },
        };
        let mut call_args = vec![source];
        if let Some((param, ty, body)) = pred {
            if let Some(written) = &ty {
                if self.norm(written) != self.norm(&item) {
                    self.error(
                        Reason::TypeMismatch,
                        format!("`{name}`'s closure takes `{}`, the items are `{}`", show(written), show(&item)),
                    );
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

    fn one_param_fn(&mut self, method: &str, arg: &Expr) -> Option<(Name, Option<Ty>, Expr)> {
        match arg.unpositioned() {
            Expr::Closure { params, body, .. } if params.len() == 1 => {
                if body.exits() {
                    self.error(
                        Reason::Closure,
                        format!("a closure passed to `{method}` may not use `?` or `return` in v0; write the loop"),
                    );
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
            self.error(Reason::MethodCall, "`collect` builds a `Vec` from `xs.iter()`, `s.chars()`, `s.bytes()`, or `s.split(c)`, through any `map(f)` and `filter(p)`, in v0".to_string());
            return Some(failed);
        }
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
            // The items are what the sequence gives, whatever the target says.
            let pieces = Ty::Vec(Box::new(item));
            return Some((e, self.expect(want.filter(|t| !super::has_hole(t)), Some(pieces))));
        };
        let Some((param, written, body)) = self.one_param_fn("map", map) else {
            return Some(failed);
        };
        if let Some(written) = &written {
            if self.norm(written) != item {
                self.error(
                    Reason::TypeMismatch,
                    format!("`map`'s closure takes `{}`, the items are `{}`", show(written), show(&item)),
                );
                return Some(failed);
            }
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
        let filled = if result {
            Ty::Result { ok: Box::new(Ty::Vec(Box::new(elem))), err: Box::new(err.expect("a Result has its error")) }
        } else {
            Ty::Vec(Box::new(elem))
        };
        Some((e, Some(filled)))
    }

    /// What a consumer or `collect` walks: `s.chars()`, `s.bytes()`,
    /// `xs.iter()` / `xs.into_iter()`, or `s.split(c)`, through any
    /// `.map(f)`, `.filter(p)`, `.copied()`, and `.cloned()` (design/01
    /// §7.13). Typed: the source, what it walks, and its items. `None` after
    /// an error; `expr` is one by `is_sequence`.
    pub(super) fn sequence(&mut self, expr: &Expr, consumer: &str) -> Option<(Expr, Over, Ty)> {
        let Expr::MethodCall { receiver, name, args } = expr.unpositioned() else { unreachable!("is_sequence") };
        match (name.as_str(), args.as_slice()) {
            ("copied" | "cloned", []) => self.sequence(receiver, consumer),
            (stage @ ("map" | "filter"), [f]) => {
                let (source, over, item) = self.sequence(receiver, consumer)?;
                let (param, written, body) = self.one_param_fn(stage, f)?;
                if let Some(written) = &written {
                    if self.norm(written) != self.norm(&item) {
                        self.error(
                            Reason::TypeMismatch,
                            format!("`{stage}`'s closure takes `{}`, the items are `{}`", show(written), show(&item)),
                        );
                        return None;
                    }
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
            (adaptor, _) => {
                let (over, source) = match (adaptor, args.as_slice()) {
                    ("chars", []) => (Over::Chars, (**receiver).clone()),
                    ("bytes", []) => (Over::Bytes, (**receiver).clone()),
                    ("iter" | "into_iter", []) => (Over::Items, (**receiver).clone()),
                    // `s.split(c)` as a `for` gives it: the pieces, a `char` apart.
                    ("split", [sep]) => (
                        Over::Items,
                        Expr::Call { callee: Callee::StrSplit, args: vec![(**receiver).clone(), sep.clone()] },
                    ),
                    _ => unreachable!("is_sequence"),
                };
                let before = self.out.len();
                let (source, st) = self.expr(&source, None);
                let item = match (over, st.as_ref().map(|t| self.norm(t))) {
                    (Over::Chars, Some(Ty::Prim(Prim::String | Prim::Str))) => Ty::Prim(Prim::Char),
                    (Over::Bytes, Some(Ty::Prim(Prim::String | Prim::Str))) => Ty::Prim(Prim::U8),
                    (Over::Items, Some(Ty::Vec(t))) => *t,
                    (_, found) => {
                        if self.out.len() == before {
                            let found = found.as_ref().map(show).unwrap_or_else(|| "?".into());
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

    /// The type `body` has with `param` bound to `item`, for a hole that
    /// `map(f)` fills; its diagnostics are left to the typing that follows.
    fn returns_of(&mut self, body: &Expr, param: &Name, item: &Ty) -> Option<Ty> {
        let before = self.out.len();
        self.scopes.push((param.as_str().to_string(), Some(item.clone())));
        let (_, ty) = self.expr(body, None);
        self.scopes.pop();
        self.out.truncate(before);
        if ty.is_none() {
            self.error(
                Reason::NeedsAnnotation,
                "the `_` in `collect::<..>()` is not known from `map`'s function; name the type".to_string(),
            );
        }
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

/// Whether `expr` is a sequence `TypeCx::sequence` types: a source, then
/// any `map`, `filter`, `copied`, and `cloned`.
fn is_sequence(expr: &Expr) -> bool {
    let Expr::MethodCall { receiver, name, args } = expr.unpositioned() else { return false };
    match (name.as_str(), args.len()) {
        ("copied" | "cloned", 0) | ("map" | "filter", 1) => is_sequence(receiver),
        ("chars" | "bytes" | "iter" | "into_iter", 0) | ("split", 1) => true,
        _ => false,
    }
}
