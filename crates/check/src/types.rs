//! Local type inference, then a rewrite of numeric code into forms whose JS
//! meaning equals the Rust debug-build meaning:
//!
//! - integer arithmetic becomes `Callee::Int` (truncating division, a throw
//!   where Rust panics);
//! - `f32` arithmetic is wrapped in `Callee::Fround`;
//! - every numeric literal gets its type, so 64-bit ones print as `bigint`.
//!
//! Inference is bidirectional and local to one expression tree. Where rustc
//! would fall back to `i32`/`f64` or look at later uses, this pass asks for a
//! suffix or an annotation instead of guessing.

use purecrate_ir::{
    Reason,
    Arm, BinOp, Callee, CharMethod, ClosureParam, Crate, Expr, Fields, FloatTy, Fn, IntOp, IntTy, Item, Lit, Name, Pattern, Prim, StrMethod, TryOn,
    Ty, UnOp, VariantBind, VariantFields, NEWTYPE_FIELD,
};

use crate::defs::Defs;
use crate::Diagnostic;

pub fn elaborate(krate: &Crate) -> Result<Crate, Vec<Diagnostic>> {
    let defs = Defs::of(krate);
    let mut out = Vec::new();
    let items = krate
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| match item {
            Item::Fn(f) => Item::Fn(Typer::new(&defs, i, &mut out).func(f)),
            other => other.clone(),
        })
        .collect();
    if out.is_empty() {
        Ok(Crate::new(krate.name.as_str(), items))
    } else {
        Err(out)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Num {
    Int(IntTy),
    Float(FloatTy),
}

struct Typer<'d, 'a> {
    defs: &'d Defs<'a>,
    item: usize,
    out: &'d mut Vec<Diagnostic>,
    ret: Ty,
    scopes: Vec<(String, Option<Ty>)>,
}

type Typed = (Expr, Option<Ty>);

impl<'d, 'a> Typer<'d, 'a> {
    fn new(defs: &'d Defs<'a>, item: usize, out: &'d mut Vec<Diagnostic>) -> Self {
        Typer {
            defs,
            item,
            out,
            ret: Ty::Prim(Prim::Unit),
            scopes: Vec::new(),
        }
    }

    fn error(&mut self, reason: Reason, message: String) {
        self.out.push(Diagnostic::at(self.item, reason, message));
    }

    /// `x.m(args)` is `T::m(x, args)` for the crate's own `impl T`. The
    /// receiver is typed once, to find `T`, and the typed receiver is the
    /// call's first argument: typing it again would double the work at every
    /// link of a chain.
    fn method_call(&mut self, receiver: &Expr, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
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
                self.out.push(
                    Diagnostic::at(self.item, Reason::MethodCall, format!(
                        "`.{}()` on `{}` is not in v0: only methods of the crate's own inherent impls",
                        name.as_str(),
                        show(&rt)
                    ))
                    .about(name.as_str()),
                );
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

    fn func(mut self, f: &Fn) -> Fn {
        self.ret = f.ret.clone();
        self.scopes = f
            .params
            .iter()
            .map(|p| (p.name.as_str().to_string(), Some(p.ty.clone())))
            .collect();
        let ret = f.ret.clone();
        let (body, _) = self.expr(&f.body, Some(&ret));
        Fn {
            body,
            ..f.clone()
        }
    }

    /// Expands aliases. Alias cycles are cut after a fixed depth.
    fn norm(&self, ty: &Ty) -> Ty {
        let mut t = ty.peel().clone();
        for _ in 0..32 {
            match &t {
                Ty::Named(n) => match self.defs.aliases.get(n.as_str()) {
                    Some(a) => t = a.ty.clone(),
                    None => return t,
                },
                _ => return t,
            }
        }
        t
    }

    fn num(&self, ty: &Ty) -> Option<Num> {
        match self.norm(ty) {
            Ty::Prim(p) => p
                .int()
                .map(Num::Int)
                .or_else(|| p.float().map(Num::Float)),
            _ => None,
        }
    }

    fn same(&self, a: &Ty, b: &Ty) -> bool {
        norm_deep(self, a) == norm_deep(self, b)
    }

    /// Reports a mismatch between a known result and a known expectation.
    /// A `String` may stand where `&str` is wanted: borrows are erased, and
    /// `&String` derefs to `&str`. The reverse is a rustc error.
    fn expect(&mut self, want: Option<&Ty>, got: Option<Ty>) -> Option<Ty> {
        if let (Some(w), Some(g)) = (want, &got) {
            let (wn, gn) = (self.norm(w), self.norm(g));
            let deref = wn == Ty::Prim(Prim::Str) && gn == Ty::Prim(Prim::String);
            if *g != Ty::Never && *w != Ty::Never && !deref && !self.same(w, g) {
                let hint = if wn == Ty::Prim(Prim::String) && gn == Ty::Prim(Prim::Str) {
                    "; a string literal is `&str`, write `String::from(\"..\")`"
                } else {
                    ""
                };
                self.error(Reason::TypeMismatch, format!("expected `{}`, found `{}`{hint}", show(w), show(g)));
            }
        }
        got
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        self.scopes
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .and_then(|(_, t)| t.clone())
    }

    fn expr(&mut self, expr: &Expr, want: Option<&Ty>) -> Typed {
        match expr {
            // The position ends here: the typed body has no `At`.
            Expr::At { at, expr } => {
                let before = self.out.len();
                let typed = self.expr(expr, want);
                crate::locate(&mut self.out[before..], *at);
                typed
            }
            Expr::Ignored { wrapper, expr } => {
                let inner_want = want.map(Ty::peel);
                let (expr, ty) = self.expr(expr, inner_want);
                (
                    Expr::Ignored {
                        wrapper: *wrapper,
                        expr: Box::new(expr),
                    },
                    ty,
                )
            }
            Expr::Lit(lit) => self.lit(lit, false, want),
            Expr::Var(n) => {
                let t = self.lookup(n.as_str());
                (expr.clone(), self.expect(want, t))
            }
            Expr::Let {
                name,
                mutable,
                ty,
                value,
                then,
            } => {
                let before = self.out.len();
                let (value, vt) = self.expr(value, ty.as_ref());
                let bound = ty.clone().or(vt);
                if bound.is_none() && self.out.len() == before {
                    let (kw, first) = if *mutable { ("let mut", " from its first value") } else { ("let", "") };
                    self.error(Reason::NeedsAnnotation, format!(
                        "the type of `{kw} {n}` is not known{first}; write `{kw} {n}: T`",
                        n = name.as_str(),
                    ));
                }
                // TS widens an unannotated binding (`kind: "Walk"` becomes
                // `kind: string`, `null` stays `null`), so the type is always
                // written out when known.
                let annotation = ty.clone().or_else(|| bound.clone().filter(|t| *t != Ty::Never));
                self.scopes.push((name.as_str().to_string(), bound));
                let (then, tt) = self.expr(then, want);
                self.scopes.pop();
                (
                    Expr::Let {
                        name: name.clone(),
                        mutable: *mutable,
                        ty: annotation,
                        value: Box::new(value),
                        then: Box::new(then),
                    },
                    tt,
                )
            }
            Expr::For { var, start, end, body, .. } => {
                // `0..n`: an unsuffixed start takes the end's type. Only the
                // typing order changes; the start still runs first.
                let before = self.out.len();
                let ((s, st), (e, et)) = if is_bare_int(start) {
                    let (e, et) = self.expr(end, None);
                    (self.expr(start, et.as_ref()), (e, et))
                } else {
                    let (s, st) = self.expr(start, None);
                    let e = self.expr(end, st.as_ref());
                    ((s, st), e)
                };
                let bound = st.clone().or(et);
                let int = bound.as_ref().and_then(|t| match self.norm(t) {
                    Ty::Prim(p) => p.int(),
                    _ => None,
                });
                if int.is_none() && self.out.len() == before {
                    let found = bound.as_ref().map(show).unwrap_or_else(|| "?".into());
                    self.error(Reason::TypeMismatch, format!(
                        "`for` takes a range `a..b` of one integer type, found `{found}`"
                    ));
                }
                self.scopes.push((var.as_str().to_string(), bound));
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                self.scopes.pop();
                let e = Expr::For {
                    var: var.clone(),
                    ty: int,
                    start: Box::new(s),
                    end: Box::new(e),
                    body: Box::new(b),
                };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            Expr::ForChars { var, string, body } => {
                let before = self.out.len();
                let (s, st) = self.expr(string, None);
                let is_string = st
                    .as_ref()
                    .is_some_and(|t| matches!(self.norm(t), Ty::Prim(Prim::String | Prim::Str)));
                if !is_string && self.out.len() == before {
                    let found = st.as_ref().map(show).unwrap_or_else(|| "?".into());
                    self.error(Reason::TypeMismatch, format!(
                        "`for c in s.chars()` takes a `String` or `&str`, found `{found}`"
                    ));
                }
                self.scopes.push((var.as_str().to_string(), Some(Ty::Prim(Prim::Char))));
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                self.scopes.pop();
                let e = Expr::ForChars {
                    var: var.clone(),
                    string: Box::new(s),
                    body: Box::new(b),
                };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            Expr::Assign { name, value } => {
                let target = self.lookup(name.as_str());
                let (value, _) = self.expr(value, target.as_ref());
                let e = Expr::Assign {
                    name: name.clone(),
                    value: Box::new(value),
                };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            Expr::Seq { first, then } => {
                let (first, ft) = self.expr(first, None);
                // `{ return x; }`: the block's `()` is never produced.
                let (then, tt) = if ft == Some(Ty::Never) && **then == Expr::Lit(Lit::Unit) {
                    ((**then).clone(), Some(Ty::Never))
                } else {
                    self.expr(then, want)
                };
                let e = Expr::Seq {
                    first: Box::new(first),
                    then: Box::new(then),
                };
                (e, tt)
            }
            Expr::If { cond, then, else_ } => {
                let (cond, _) = self.expr(cond, Some(&Ty::bool()));
                let (then, tt, else_, et) = self.pair(then, else_, want);
                (
                    Expr::If {
                        cond: Box::new(cond),
                        then: Box::new(then),
                        else_: Box::new(else_),
                    },
                    join(tt, et),
                )
            }
            Expr::Match { scrutinee, arms } => self.match_(scrutinee, arms, want),
            Expr::Call { callee, args } => self.call(callee, args, want),
            Expr::MethodCall { receiver, name, args } => self.method_call(receiver, name, args, want),
            Expr::Closure { params, ret, body } => self.closure(params, ret.as_ref(), body, want),
            Expr::Construct {
                ty,
                variant,
                fields,
                base,
            } => {
                let fields = self.construct_fields(ty.as_str(), variant.as_ref().map(|v| v.as_str()), fields);
                let want_ty = Ty::Named(ty.clone());
                let base = match base {
                    Some(b) => {
                        let (b, _) = self.expr(b, Some(&want_ty));
                        Some(Box::new(b))
                    }
                    None => None,
                };
                let e = Expr::Construct {
                    ty: ty.clone(),
                    variant: variant.clone(),
                    fields,
                    base,
                };
                (e, self.expect(want, Some(want_ty)))
            }
            Expr::Index { base, index } => {
                let (base, bt) = self.expr(base, None);
                let elem = match bt.as_ref().map(|t| self.norm(t)) {
                    Some(Ty::Vec(t)) => Some(*t),
                    Some(other) => {
                        self.error(Reason::Index, format!("cannot index `{}`", show(&other)));
                        None
                    }
                    None => None,
                };
                let (index, _) = self.expr(index, Some(&Ty::Prim(Prim::Usize)));
                (
                    Expr::Index {
                        base: Box::new(base),
                        index: Box::new(index),
                    },
                    self.expect(want, elem),
                )
            }
            Expr::Field { base, name } => {
                let (base, bt) = self.expr(base, None);
                let t = bt.as_ref().and_then(|bt| match self.norm(bt) {
                    Ty::Named(s) => self
                        .defs
                        .structs
                        .get(s.as_str())
                        .and_then(|st| st.fields.iter().find(|f| f.name == *name))
                        .map(|f| f.ty.clone()),
                    _ => None,
                });
                if let (None, Some(bt)) = (&t, &bt) {
                    if name.as_str() == NEWTYPE_FIELD {
                        self.error(Reason::TupleField, format!(
                            "`.0` is only in v0 on a one-field tuple struct, not on `{}`",
                            show(bt)
                        ));
                    }
                }
                let e = Expr::Field {
                    base: Box::new(base),
                    name: name.clone(),
                };
                (e, self.expect(want, t))
            }
            Expr::Tuple(xs) => {
                let wants: Vec<Option<Ty>> = match want.map(|w| self.norm(w)) {
                    Some(Ty::Tuple(ts)) if ts.len() == xs.len() => ts.into_iter().map(Some).collect(),
                    _ => vec![None; xs.len()],
                };
                let (elems, tys): (Vec<Expr>, Vec<Option<Ty>>) = xs
                    .iter()
                    .zip(&wants)
                    .map(|(x, w)| self.expr(x, w.as_ref()))
                    .unzip();
                let t = tys.into_iter().collect::<Option<Vec<Ty>>>().map(Ty::Tuple);
                (Expr::Tuple(elems), self.expect(want, t))
            }
            Expr::Array(xs) => {
                let mut elem = match want.map(|w| self.norm(w)) {
                    Some(Ty::Vec(t)) => Some(*t),
                    _ => None,
                };
                if elem.is_none() {
                    if let Some(first) = xs.iter().find(|x| !needs_context(x)) {
                        elem = self.expr(first, None).1;
                    }
                }
                let elems = xs.iter().map(|x| self.expr(x, elem.as_ref()).0).collect();
                let t = elem.map(|t| Ty::Vec(Box::new(t)));
                (Expr::Array(elems), self.expect(want, t))
            }
            Expr::Binary { op, left, right } => self.binary(*op, left, right, want),
            Expr::Unary { op, expr: inner } => self.unary(*op, inner, want),
            Expr::Return(e) => {
                let ret = self.ret.clone();
                let (e, _) = self.expr(e, Some(&ret));
                (Expr::Return(Box::new(e)), Some(Ty::Never))
            }
            Expr::Try { expr: inner, .. } => self.try_(inner, want),
            Expr::Unreachable => (Expr::Unreachable, Some(Ty::Never)),
        }
    }

    /// Rust's `?` also converts the error with `From`; v0 has no traits, so
    /// the error type must already be the function's.
    fn try_(&mut self, inner: &Expr, want: Option<&Ty>) -> Typed {
        let (inner, it) = self.expr(inner, None);
        let ret = self.norm(&self.ret);
        let (on, t) = match (it.map(|t| self.norm(&t)), &ret) {
            (Some(Ty::Result { ok, err }), Ty::Result { err: ret_err, .. }) => {
                if !self.same(&err, ret_err) {
                    self.error(Reason::TryConversion, format!(
                        "`?` on an error of type `{}` in a function returning `{}`; \
                         v0 has no `From` conversion, so the error types must match",
                        show(&err),
                        show(&ret)
                    ));
                }
                (Some(TryOn::Result), Some(*ok))
            }
            (Some(Ty::Option(inner_ty)), Ty::Option(_)) => (Some(TryOn::Option), Some(*inner_ty)),
            (Some(t @ (Ty::Result { .. } | Ty::Option(_))), _) => {
                self.error(Reason::TryConversion, format!(
                    "`?` on `{}` in a function returning `{}`",
                    show(&t),
                    show(&ret)
                ));
                (None, None)
            }
            (Some(t), _) => {
                self.error(Reason::TypeMismatch, format!("`?` needs a `Result` or `Option`, found `{}`", show(&t)));
                (None, None)
            }
            (None, _) => (None, None),
        };
        let e = Expr::Try {
            expr: Box::new(inner),
            on,
        };
        (e, self.expect(want, t))
    }

    /// Types two expressions that must agree, e.g. `if` branches or the
    /// operands of `+`. An untyped literal borrows the other side's type.
    fn pair(&mut self, a: &Expr, b: &Expr, want: Option<&Ty>) -> (Expr, Option<Ty>, Expr, Option<Ty>) {
        if want.is_none() && needs_context(a) && !needs_context(b) {
            let (b, bt) = self.expr(b, None);
            let hint = bt.clone().filter(|t| *t != Ty::Never);
            let (a, at) = self.expr(a, hint.as_ref());
            (a, at, b, bt)
        } else {
            let (a, at) = self.expr(a, want);
            let hint = want.cloned().or_else(|| at.clone().filter(|t| *t != Ty::Never));
            let (b, bt) = self.expr(b, hint.as_ref());
            (a, at, b, bt)
        }
    }

    /// Operands of a comparison. Rust compares `String` and `&str` in either
    /// order, so a string side only asks the other side for `&str`.
    fn compared(&mut self, a: &Expr, b: &Expr) -> (Expr, Option<Ty>, Expr, Option<Ty>) {
        if needs_context(a) && !needs_context(b) {
            return self.pair(a, b, None);
        }
        let (a, at) = self.expr(a, None);
        let hint = at.clone().filter(|t| *t != Ty::Never).map(|t| match self.norm(&t) {
            Ty::Prim(Prim::String) => Ty::Prim(Prim::Str),
            _ => t,
        });
        let (b, bt) = self.expr(b, hint.as_ref());
        (a, at, b, bt)
    }

    fn lit(&mut self, lit: &Lit, negated: bool, want: Option<&Ty>) -> Typed {
        let wanted = want.and_then(|w| self.num(w));
        match lit {
            Lit::Int { value, ty } => {
                let value = if negated { -value } else { *value };
                let shown = value.to_string();
                let ty = match (ty, wanted) {
                    (Some(t), _) => Some(*t),
                    (None, Some(Num::Int(t))) => Some(t),
                    (None, Some(Num::Float(_))) => {
                        self.error(Reason::TypeMismatch, format!("integer literal `{shown}` where a float is expected; write `{shown}.0`"));
                        return (Expr::Lit(lit.clone()), None);
                    }
                    (None, None) if want.is_some() => {
                        let w = show(want.expect("checked"));
                        self.error(Reason::TypeMismatch, format!("expected `{w}`, found integer literal `{shown}`"));
                        return (Expr::Lit(lit.clone()), None);
                    }
                    (None, None) => {
                        self.error(Reason::NeedsAnnotation, format!(
                            "cannot tell the integer type of `{shown}`; add a suffix like `{shown}i32` or annotate the binding"
                        ));
                        return (Expr::Lit(lit.clone()), None);
                    }
                };
                let t = ty.expect("set above");
                let (lo, hi) = t.bounds();
                if value < lo || value > hi {
                    self.error(Reason::TypeMismatch, format!("literal `{shown}` does not fit in `{}`", t.as_str()));
                }
                let e = Expr::Lit(Lit::Int { value, ty: Some(t) });
                (e, self.expect(want, Some(Ty::Prim(t.into()))))
            }
            Lit::Float { digits, ty } => {
                let ty = match (ty, wanted) {
                    (Some(t), _) => *t,
                    (None, Some(Num::Float(t))) => t,
                    (None, _) if want.is_some() => {
                        let w = show(want.expect("checked"));
                        self.error(Reason::TypeMismatch, format!("expected `{w}`, found float literal `{digits}`"));
                        return (Expr::Lit(lit.clone()), None);
                    }
                    (None, _) => {
                        self.error(Reason::NeedsAnnotation, format!(
                            "cannot tell the float type of `{digits}`; add a suffix like `{digits}f64` or annotate the binding"
                        ));
                        return (Expr::Lit(lit.clone()), None);
                    }
                };
                let e = Expr::Lit(Lit::Float {
                    digits: digits.clone(),
                    ty: Some(ty),
                });
                let e = if negated {
                    Expr::Call {
                        callee: Callee::AsFloat(ty),
                        args: vec![Expr::Unary {
                            op: UnOp::Neg,
                            expr: Box::new(e),
                        }],
                    }
                } else {
                    e
                };
                (e, self.expect(want, Some(Ty::Prim(ty.into()))))
            }
            Lit::Bool(_) => (Expr::Lit(lit.clone()), self.expect(want, Some(Ty::bool()))),
            Lit::Str(_) => (
                Expr::Lit(lit.clone()),
                self.expect(want, Some(Ty::Prim(Prim::Str))),
            ),
            Lit::Char(_) => (
                Expr::Lit(lit.clone()),
                self.expect(want, Some(Ty::Prim(Prim::Char))),
            ),
            Lit::Unit => (
                Expr::Lit(lit.clone()),
                self.expect(want, Some(Ty::Prim(Prim::Unit))),
            ),
            Lit::Null => (Expr::Lit(lit.clone()), want.cloned()),
        }
    }

    /// An operator whose operand types stayed unknown would print as the raw
    /// JS operator. Not reported when this item already failed: the unknown
    /// type most likely comes from a binding reported there.
    fn unknown(&mut self, what: &str) {
        if !self.out.iter().any(|d| d.item == self.item) {
            self.error(Reason::NeedsAnnotation, format!(
                "the operand types of this {what} are not known here; annotate the binding they come from"
            ));
        }
    }

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, want: Option<&Ty>) -> Typed {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                let (l, lt, r, rt) = self.pair(left, right, want);
                let t = join(lt, rt);
                let Some(t) = t.filter(|t| *t != Ty::Never) else {
                    self.unknown("arithmetic");
                    return (rebuild(op, l, r), None);
                };
                let e = match self.num(&t) {
                    Some(Num::Int(it)) => Expr::Call {
                        callee: Callee::Int {
                            ty: it,
                            op: int_op(op),
                        },
                        args: vec![l, r],
                    },
                    Some(Num::Float(FloatTy::F32)) => Expr::Call {
                        callee: Callee::Fround,
                        args: vec![rebuild(op, l, r)],
                    },
                    Some(Num::Float(FloatTy::F64)) => Expr::Call {
                        callee: Callee::AsFloat(FloatTy::F64),
                        args: vec![rebuild(op, l, r)],
                    },
                    None => {
                        self.error(Reason::NumericOp, format!("arithmetic on `{}` is not in v0", show(&t)));
                        rebuild(op, l, r)
                    }
                };
                (e, Some(t))
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let (l, lt, r, rt) = self.compared(left, right);
                let t = join(lt, rt).filter(|t| *t != Ty::Never);
                if t.is_none() {
                    self.unknown("comparison");
                }
                let ordered = !matches!(op, BinOp::Eq | BinOp::Ne);
                if let Some(t) = &t {
                    let ok = match self.norm(t) {
                        _ if self.num(t).is_some() => true,
                        // The canonical form orders as the 16 bytes do.
                        Ty::Prim(Prim::Char | Prim::Uuid) => true,
                        Ty::Prim(Prim::Bool | Prim::String | Prim::Str) => !ordered,
                        _ => false,
                    };
                    if !ok {
                        let what = if ordered { "ordering" } else { "equality" };
                        self.error(Reason::NumericOp, format!(
                            "{what} on `{}` is not in v0; JS compares it differently",
                            show(t)
                        ));
                    }
                }
                // JS orders strings by UTF-16 unit; `char` orders by code point.
                if ordered && t.is_some_and(|t| self.norm(&t) == Ty::Prim(Prim::Char)) {
                    let code = |e| Expr::Call { callee: Callee::CharCode(IntTy::U32), args: vec![e] };
                    return (rebuild(op, code(l), code(r)), self.expect(want, Some(Ty::bool())));
                }
                (rebuild(op, l, r), self.expect(want, Some(Ty::bool())))
            }
            BinOp::And | BinOp::Or => {
                let (l, _) = self.expr(left, Some(&Ty::bool()));
                let (r, _) = self.expr(right, Some(&Ty::bool()));
                (rebuild(op, l, r), self.expect(want, Some(Ty::bool())))
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => {
                let (l, lt, r, rt) = self.pair(left, right, want);
                match join(lt, rt).filter(|t| *t != Ty::Never) {
                    Some(t) => self.bit_call(op, t, vec![l, r]),
                    None => {
                        self.unknown("bitwise operator");
                        (rebuild(op, l, r), None)
                    }
                }
            }
            // Rust types the amount on its own: an unsuffixed literal there is
            // `i32`, whatever the left side is.
            BinOp::Shl | BinOp::Shr => {
                let (l, lt) = self.expr(left, want);
                let amount_hint = is_bare_int(right).then(|| Ty::Prim(Prim::I32));
                let (r, rt) = self.expr(right, amount_hint.as_ref());
                if let Some(rt) = rt.as_ref().filter(|t| **t != Ty::Never) {
                    if !matches!(self.num(rt), Some(Num::Int(_))) {
                        self.error(Reason::NumericOp, format!("a shift amount is an integer, found `{}`", show(rt)));
                    }
                }
                match lt.filter(|t| *t != Ty::Never) {
                    Some(t) => self.bit_call(op, t, vec![l, r]),
                    None => {
                        self.unknown("shift");
                        (rebuild(op, l, r), None)
                    }
                }
            }
        }
    }

    /// `& | ^ << >>` and `!` on an integer of type `t`, as a `Callee::Int`
    /// call. `usize` is refused: Rust gives it 64 bits, which its `number`
    /// cannot hold, so `!x` or `x << 20` would have no TS value.
    fn bit_call(&mut self, op: BinOp, t: Ty, args: Vec<Expr>) -> Typed {
        let int_op = int_op(op);
        let what = match op {
            BinOp::BitAnd => "`&`",
            BinOp::BitOr => "`|`",
            BinOp::BitXor => "`^`",
            BinOp::Shl => "`<<`",
            BinOp::Shr => "`>>`",
            _ => "`!`",
        };
        match self.num(&t) {
            Some(Num::Int(IntTy::Usize)) => {
                self.error(Reason::NumericOp, format!(
                    "{what} on `usize` is not in v0: `usize` is a JS number checked to 2^53, not 64 bits; use `u32` or `u64`"
                ));
            }
            Some(Num::Int(it)) => {
                let e = Expr::Call { callee: Callee::Int { ty: it, op: int_op }, args };
                return (e, Some(t));
            }
            _ if self.norm(&t) == Ty::bool() && op != BinOp::Shl && op != BinOp::Shr => {
                let hint = match op {
                    BinOp::BitAnd => "`&&`",
                    BinOp::BitOr => "`||`",
                    _ => "`!=`",
                };
                self.error(Reason::NumericOp, format!(
                    "{what} on `bool` is not in v0; write {hint} (both sides are evaluated with {what}, so bind the right side first if it has an effect)"
                ));
            }
            _ => self.error(Reason::NumericOp, format!("{what} on `{}` is not in v0", show(&t))),
        }
        let mut args = args.into_iter();
        let (l, r) = (args.next().expect("two operands"), args.next().expect("two operands"));
        (rebuild(op, l, r), Some(t))
    }

    fn unary(&mut self, op: UnOp, inner: &Expr, want: Option<&Ty>) -> Typed {
        match op {
            UnOp::Neg => {
                if let Expr::Lit(lit @ (Lit::Int { .. } | Lit::Float { .. })) = inner {
                    return self.lit(lit, true, want);
                }
                let (e, t) = self.expr(inner, want);
                let Some(t) = t.filter(|t| *t != Ty::Never) else {
                    self.unknown("negation");
                    return (neg(e), None);
                };
                let e = match self.num(&t) {
                    Some(Num::Int(it)) if it.is_signed() => Expr::Call {
                        callee: Callee::Int {
                            ty: it,
                            op: IntOp::Neg,
                        },
                        args: vec![e],
                    },
                    Some(Num::Float(ft)) => Expr::Call {
                        callee: Callee::AsFloat(ft),
                        args: vec![neg(e)],
                    },
                    _ => {
                        self.error(Reason::NumericOp, format!("cannot negate `{}`", show(&t)));
                        neg(e)
                    }
                };
                (e, Some(t))
            }
            // Logical on `bool`, bitwise on an integer: the operand decides.
            UnOp::Not => {
                let (e, t) = self.expr(inner, want);
                let int = t.as_ref().filter(|t| **t != Ty::Never).and_then(|t| match self.num(t) {
                    Some(Num::Int(it)) => Some(it),
                    _ => None,
                });
                match (int, t) {
                    (Some(IntTy::Usize), Some(t)) => {
                        self.error(Reason::NumericOp,
                            "`!` on `usize` is not in v0: `usize` is a JS number checked to 2^53, not 64 bits; use `u32` or `u64`".to_string());
                        (Expr::Unary { op, expr: Box::new(e) }, Some(t))
                    }
                    (Some(it), t) => {
                        let e = Expr::Call { callee: Callee::Int { ty: it, op: IntOp::Not }, args: vec![e] };
                        (e, t)
                    }
                    (None, Some(t)) if t != Ty::Never && self.norm(&t) != Ty::bool() => {
                        self.error(Reason::NumericOp, format!("cannot apply `!` to `{}`", show(&t)));
                        (Expr::Unary { op, expr: Box::new(e) }, None)
                    }
                    (None, t) => (Expr::Unary { op, expr: Box::new(e) }, t.map(|_| Ty::bool())),
                }
            }
        }
    }

    fn match_(&mut self, scrutinee: &Expr, arms: &[Arm], want: Option<&Ty>) -> Typed {
        let (scrutinee, st) = self.expr(scrutinee, None);
        // The arms' bindings would have no type, and the printed temporary
        // would be `Result<T, unknown>` in TS (`Ok::<T, E>(x)` is read as
        // `Ok(x)`). Not reported when a binding of this item already was.
        if st.is_none() && !self.out.iter().any(|d| d.item == self.item) {
            self.error(Reason::NeedsAnnotation,
                "the type of the matched value is not known here; bind it first with `let x: T = ..`".to_string());
        }
        let st = st.map(|t| self.norm(&t));
        let mut result: Option<Ty> = None;
        let arms = arms
            .iter()
            .map(|arm| {
                let depth = self.scopes.len();
                self.bind(&arm.pattern, st.as_ref());
                let hint = want.cloned().or_else(|| result.clone());
                let (body, t) = self.expr(&arm.body, hint.as_ref());
                self.scopes.truncate(depth);
                result = join(result.take(), t);
                Arm {
                    pattern: self.lit_pattern(&arm.pattern, st.as_ref()),
                    body,
                }
            })
            .collect();
        (
            Expr::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            result,
        )
    }

    /// An integer arm with every literal given the scrutinee's type, so the
    /// printer knows `1` from `1n`; a string arm checked against a `&str`.
    /// Other patterns are returned as they are.
    fn lit_pattern(&mut self, pattern: &Pattern, scrutinee: Option<&Ty>) -> Pattern {
        if pattern.is_char_case() {
            match scrutinee {
                Some(Ty::Prim(Prim::Char) | Ty::Never) | None => {}
                Some(t) => self.error(Reason::TypeMismatch, format!(
                    "`char` patterns do not match a value of type `{}`",
                    show(t)
                )),
            }
            return pattern.clone();
        }
        if pattern.is_str_case() {
            match scrutinee {
                Some(Ty::Prim(Prim::Str) | Ty::Never) | None => {}
                Some(Ty::Prim(Prim::String)) => self.error(Reason::TypeMismatch,
                    "string patterns match a `&str`, found `String`; match on `s.as_str()`".to_string()),
                Some(t) => self.error(Reason::TypeMismatch, format!(
                    "string patterns do not match a value of type `{}`",
                    show(t)
                )),
            }
            return pattern.clone();
        }
        if !pattern.is_int_case() {
            return pattern.clone();
        }
        let ty = match scrutinee.and_then(|t| self.num(t)) {
            Some(Num::Int(t)) => Some(t),
            _ => {
                if let Some(t) = scrutinee {
                    self.error(Reason::TypeMismatch, format!(
                        "integer patterns do not match a value of type `{}`",
                        show(t)
                    ));
                }
                None
            }
        };
        self.fill_int(pattern, ty)
    }

    fn fill_int(&mut self, pattern: &Pattern, ty: Option<IntTy>) -> Pattern {
        match pattern {
            Pattern::Lit(lit) => Pattern::Lit(self.int_lit(lit, ty)),
            Pattern::Range { lo, hi, inclusive } => Pattern::Range {
                lo: self.int_lit(lo, ty),
                hi: self.int_lit(hi, ty),
                inclusive: *inclusive,
            },
            Pattern::Or(alts) => Pattern::Or(alts.iter().map(|a| self.fill_int(a, ty)).collect()),
            other => other.clone(),
        }
    }

    fn int_lit(&mut self, lit: &Lit, ty: Option<IntTy>) -> Lit {
        match lit {
            Lit::Int { value, ty: written } => {
                if let (Some(w), Some(t)) = (written, ty) {
                    if *w != t {
                        self.error(Reason::TypeMismatch, format!(
                            "pattern `{value}{}` does not match a value of type `{}`",
                            w.as_str(),
                            t.as_str()
                        ));
                    }
                }
                Lit::Int { value: *value, ty: ty.or(*written) }
            }
            other => other.clone(),
        }
    }

    /// `scrutinee` is the normalized type of the matched value, when known.
    fn bind(&mut self, pattern: &Pattern, scrutinee: Option<&Ty>) {
        let inner = |p: &Pattern| match p {
            Pattern::Var(n) => Some(n.as_str().to_string()),
            _ => None,
        };
        let (name, ty) = match (pattern, scrutinee) {
            (Pattern::Var(n), t) => (Some(n.as_str().to_string()), t.cloned()),
            (Pattern::OptionSome(p), Some(Ty::Option(t))) => (inner(p), Some((**t).clone())),
            (Pattern::ResultOk(p), Some(Ty::Result { ok, .. })) => (inner(p), Some((**ok).clone())),
            (Pattern::ResultErr(p), Some(Ty::Result { err, .. })) => {
                (inner(p), Some((**err).clone()))
            }
            (Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p), other) => {
                if let Some(t) = other {
                    self.error(Reason::TypeMismatch, format!(
                        "pattern `{}` does not match a value of type `{}`",
                        describe(pattern),
                        show(t)
                    ));
                }
                (inner(p), None)
            }
            (Pattern::OptionNone, Some(t)) if !matches!(t, Ty::Option(_)) => {
                self.error(Reason::TypeMismatch, format!("pattern `None` does not match a value of type `{}`", show(t)));
                (None, None)
            }
            (Pattern::OptionNone | Pattern::Wildcard | Pattern::Lit(_) | Pattern::Range { .. }, _) => (None, None),
            (Pattern::Variant { .. }, _) => return self.bind_variant(pattern, scrutinee),
            // The alternatives bind nothing (checked by the parser).
            (Pattern::Or(alts), _) => {
                alts.iter().for_each(|alt| self.bind(alt, scrutinee));
                return;
            }
        };
        if let Some(n) = name {
            self.scopes.push((n, ty));
        }
    }

    fn bind_variant(&mut self, pattern: &Pattern, scrutinee: Option<&Ty>) {
        let Pattern::Variant { ty, variant, bind } = pattern else {
            return;
        };
        if let Some(t) = scrutinee.filter(|t| **t != Ty::Named(ty.clone())) {
            self.error(Reason::TypeMismatch, format!(
                "pattern `{}::{}` does not match a value of type `{}`",
                ty.as_str(),
                variant.as_str(),
                show(t)
            ));
        }
        let fields = self
            .defs
            .enums
            .get(ty.as_str())
            .and_then(|e| e.variants.iter().find(|v| v.name == *variant))
            .map(|v| &v.fields);
        let names: Vec<(String, Option<Ty>)> = match (bind, fields) {
            (VariantBind::Tuple(ps), Some(VariantFields::Tuple(tys))) => ps
                .iter()
                .zip(tys)
                .filter_map(|(p, t)| match p {
                    Pattern::Var(n) => Some((n.as_str().to_string(), Some(t.clone()))),
                    _ => None,
                })
                .collect(),
            (VariantBind::Struct(ps), Some(VariantFields::Struct(fs))) => ps
                .iter()
                .filter_map(|(field, p)| match p {
                    Pattern::Var(n) => Some((
                        n.as_str().to_string(),
                        fs.iter().find(|f| f.name == *field).map(|f| f.ty.clone()),
                    )),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        self.scopes.extend(names);
    }

    fn is_local(&self, name: &str) -> bool {
        self.scopes.iter().any(|(n, _)| n == name)
    }

    /// Parameter types come from annotations or, failing that, from the
    /// closure type the context expects. The body is typed with the closure's
    /// return type in place of the function's, since `?` and `return` there
    /// leave the closure.
    fn closure(&mut self, params: &[ClosureParam], ret: Option<&Ty>, body: &Expr, want: Option<&Ty>) -> Typed {
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
                self.error(Reason::NeedsAnnotation, format!(
                    "the type of closure parameter `{0}` is not known here; write `|{0}: T|`",
                    p.name.as_str()
                ));
            }
        }
        let ret = ret.cloned().or_else(|| expected.map(|(_, r)| r));
        if ret.is_none() && exits(body) {
            self.error(Reason::NeedsAnnotation,
                "a closure with `?` or `return` needs its return type; write `|..| -> T { .. }`".to_string());
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
            params: params
                .iter()
                .zip(param_tys)
                .map(|(p, ty)| ClosureParam { name: p.name.clone(), ty })
                .collect(),
            ret,
            body: Box::new(body),
        };
        (e, self.expect(want, t))
    }

    fn local_call(&mut self, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        let (params, ret) = match self.lookup(name.as_str()).map(|t| self.norm(&t)) {
            Some(Ty::Fn { params, ret }) => {
                if params.len() != args.len() {
                    self.error(Reason::ConstructShape, format!(
                        "closure `{}` takes {} argument(s), got {}",
                        name.as_str(),
                        params.len(),
                        args.len()
                    ));
                }
                (params, Some(*ret))
            }
            Some(other) => {
                self.error(Reason::TypeMismatch, format!(
                    "`{}` is a `{}`, not a closure",
                    name.as_str(),
                    show(&other)
                ));
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

    fn call(&mut self, callee: &Callee, args: &[Expr], want: Option<&Ty>) -> Typed {
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
                let sig = self
                    .defs
                    .methods
                    .get(&(ty.as_str(), name.as_str()))
                    .map(|f| sig(f));
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
                let inner = expected.as_ref().map(|(ok, err)| {
                    if matches!(callee, Callee::ResultOk) {
                        ok.clone()
                    } else {
                        err.clone()
                    }
                });
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
                            self.error(Reason::NestedOption, format!(
                                "`Some` of `{}` is `Option<Option<_>>`, and both `None`s are `null` in TS; use an enum",
                                show(inner.as_ref().expect("checked"))
                            ));
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
            Callee::Int { ty, op } => {
                let t = Ty::Prim((*ty).into());
                // A shift amount keeps its own type.
                let n = if op.is_shift() { 1 } else { op.arity() };
                (typed_args(self, vec![t.clone(); n]), Some(t))
            }
            Callee::Fround => (
                typed_args(self, vec![Ty::Prim(Prim::F64)]),
                Some(Ty::Prim(Prim::F32)),
            ),
            Callee::AsFloat(ft) => {
                let t = Ty::Prim((*ft).into());
                (typed_args(self, vec![t.clone()]), Some(t))
            }
            Callee::VecLen => (
                args.iter().map(|a| self.expr(a, None).0).collect(),
                Some(Ty::Prim(Prim::Usize)),
            ),
            Callee::VecIsEmpty | Callee::OptionIsSome | Callee::OptionIsNone => {
                (args.iter().map(|a| self.expr(a, None).0).collect(), Some(Ty::bool()))
            }
            Callee::StrBytes => (
                args.iter().map(|a| self.expr(a, None).0).collect(),
                Some(Ty::Vec(Box::new(Ty::Prim(Prim::U8)))),
            ),
            Callee::StringFrom => (
                typed_args(self, vec![Ty::Prim(Prim::Str)]),
                Some(Ty::Prim(Prim::String)),
            ),
            Callee::Str(m) => (
                args.iter().map(|a| self.expr(a, None).0).collect(),
                Some(match m {
                    StrMethod::Len => Ty::Prim(Prim::Usize),
                    StrMethod::AsStr => Ty::Prim(Prim::Str),
                    _ => Ty::bool(),
                }),
            ),
            Callee::IntFrom { to, .. } => return self.int_from(*to, args, want),
            Callee::CharCode(to) => (typed_args(self, vec![Ty::Prim(Prim::Char)]), Some(Ty::Prim((*to).into()))),
            Callee::CharFromU8 => (typed_args(self, vec![Ty::Prim(Prim::U8)]), Some(Ty::Prim(Prim::Char))),
            Callee::CharFromU32 => (
                typed_args(self, vec![Ty::Prim(Prim::U32)]),
                Some(Ty::option(Ty::Prim(Prim::Char))),
            ),
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
        };
        let e = Expr::Call {
            callee: callee.clone(),
            args,
        };
        (e, self.expect(want, t))
    }

    /// `c.m(args)` for an allow-listed `char` method.
    fn char_method(&mut self, m: CharMethod, recv: Expr, args: &[Expr], want: Option<&Ty>) -> Typed {
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
    fn str_method(&mut self, m: StrMethod, recv: Expr, args: &[Expr], want: Option<&Ty>) -> Typed {
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
    fn int_from(&mut self, to: IntTy, args: &[Expr], want: Option<&Ty>) -> Typed {
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

    fn construct_fields(&mut self, ty: &str, variant: Option<&str>, fields: &Fields) -> Fields {
        let declared: Vec<(Option<String>, Ty)> = match variant {
            None => self
                .defs
                .structs
                .get(ty)
                .map(|s| {
                    s.fields
                        .iter()
                        .map(|f| (Some(f.name.as_str().to_string()), f.ty.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            Some(v) => match self
                .defs
                .enums
                .get(ty)
                .and_then(|e| e.variants.iter().find(|x| x.name.as_str() == v))
                .map(|x| &x.fields)
            {
                Some(VariantFields::Tuple(tys)) => tys.iter().map(|t| (None, t.clone())).collect(),
                Some(VariantFields::Struct(fs)) => fs
                    .iter()
                    .map(|f| (Some(f.name.as_str().to_string()), f.ty.clone()))
                    .collect(),
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
                        let want = declared
                            .iter()
                            .find(|(d, _)| d.as_deref() == Some(n.as_str()))
                            .map(|(_, t)| t.clone());
                        (n.clone(), self.expr(x, want.as_ref()).0)
                    })
                    .collect(),
            ),
        }
    }
}

fn sig(f: &Fn) -> (Vec<Ty>, Option<Ty>) {
    (
        f.params.iter().map(|p| p.ty.clone()).collect(),
        Some(f.ret.clone()),
    )
}

/// Expressions whose numeric type comes only from their surroundings.
fn needs_context(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(Lit::Int { ty: None, .. } | Lit::Float { ty: None, .. }) => true,
        Expr::Unary {
            op: UnOp::Neg,
            expr,
        } => needs_context(expr),
        Expr::Binary {
            op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem,
            left,
            right,
        } => needs_context(left) && needs_context(right),
        Expr::Binary { op, left, right } if op.is_bitwise() => needs_context(left) && needs_context(right),
        Expr::Binary { op, left, .. } if op.is_shift() => needs_context(left),
        Expr::Unary { op: UnOp::Not, expr } => needs_context(expr),
        Expr::If { then, else_, .. } => needs_context(then) && needs_context(else_),
        Expr::Let { then, .. } | Expr::Seq { then, .. } => needs_context(then),
        _ => false,
    }
}

/// The type of two branches; a diverging branch takes the other's type.
fn join(a: Option<Ty>, b: Option<Ty>) -> Option<Ty> {
    match (a, b) {
        (Some(Ty::Never), other) | (other, Some(Ty::Never)) => other.or(Some(Ty::Never)),
        (Some(a), _) => Some(a),
        (None, b) => b,
    }
}

fn norm_deep(t: &Typer, ty: &Ty) -> Ty {
    match t.norm(ty) {
        Ty::Option(inner) => Ty::option(norm_deep(t, &inner)),
        Ty::Vec(inner) => Ty::Vec(Box::new(norm_deep(t, &inner))),
        Ty::Result { ok, err } => Ty::result(norm_deep(t, &ok), norm_deep(t, &err)),
        Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|x| norm_deep(t, x)).collect()),
        other => other,
    }
}

fn int_op(op: BinOp) -> IntOp {
    match op {
        BinOp::Add => IntOp::Add,
        BinOp::Sub => IntOp::Sub,
        BinOp::Mul => IntOp::Mul,
        BinOp::Div => IntOp::Div,
        BinOp::Rem => IntOp::Rem,
        BinOp::BitAnd => IntOp::And,
        BinOp::BitOr => IntOp::Or,
        BinOp::BitXor => IntOp::Xor,
        BinOp::Shl => IntOp::Shl,
        BinOp::Shr => IntOp::Shr,
        other => unreachable!("{other:?} is not arithmetic"),
    }
}

fn rebuild(op: BinOp, left: Expr, right: Expr) -> Expr {
    Expr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}

fn neg(e: Expr) -> Expr {
    Expr::Unary {
        op: UnOp::Neg,
        expr: Box::new(e),
    }
}

fn describe(pattern: &Pattern) -> &'static str {
    match pattern {
        Pattern::OptionSome(_) => "Some(..)",
        Pattern::OptionNone => "None",
        Pattern::ResultOk(_) => "Ok(..)",
        Pattern::ResultErr(_) => "Err(..)",
        _ => "..",
    }
}

pub(crate) fn show(ty: &Ty) -> String {
    match ty {
        Ty::Prim(p) => match p {
            Prim::Bool => "bool".into(),
            Prim::String => "String".into(),
            Prim::Str => "&str".into(),
            Prim::Char => "char".into(),
            Prim::Uuid => "Uuid".into(),
            Prim::UuidError => "uuid::Error".into(),
            Prim::Unit => "()".into(),
            Prim::F32 => "f32".into(),
            Prim::F64 => "f64".into(),
            other => other.int().map(|t| t.as_str()).unwrap_or("?").into(),
        },
        Ty::Option(t) => format!("Option<{}>", show(t)),
        Ty::Result { ok, err } => format!("Result<{}, {}>", show(ok), show(err)),
        Ty::Vec(t) => format!("Vec<{}>", show(t)),
        Ty::Tuple(ts) => format!(
            "({})",
            ts.iter().map(show).collect::<Vec<_>>().join(", ")
        ),
        Ty::Named(n) => n.as_str().to_string(),
        Ty::Fn { params, ret } => format!(
            "impl Fn({}) -> {}",
            params.iter().map(show).collect::<Vec<_>>().join(", "),
            show(ret)
        ),
        Ty::Ignored { wrapper, inner } => format!("{}<{}>", wrapper.rust_name(), show(inner)),
        Ty::Never => "!".into(),
    }
}

/// The parameters after the receiver, and the result, of a `char` method.
fn char_sig(m: CharMethod) -> (Vec<Ty>, Ty) {
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

/// Contains a `?` or `return` that leaves this closure body.
fn exits(body: &Expr) -> bool {
    match body {
        Expr::Try { .. } | Expr::Return(_) => true,
        Expr::Closure { .. } => false,
        other => other.children().into_iter().any(exits),
    }
}

/// An integer literal without a suffix, possibly negated.
fn is_bare_int(expr: &Expr) -> bool {
    match expr.unpositioned() {
        Expr::Lit(Lit::Int { ty: None, .. }) => true,
        Expr::Unary { expr, .. } => is_bare_int(expr),
        _ => false,
    }
}
