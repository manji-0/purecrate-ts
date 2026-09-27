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
    Arm, BinOp, Callee, Crate, Expr, Fields, FloatTy, Fn, IntOp, IntTy, Item, Lit, Pattern, Prim, TryOn,
    Ty, UnOp, VariantBind, VariantFields,
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

    fn error(&mut self, message: String) {
        self.out.push(Diagnostic::at(self.item, message));
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
        let mut t = ty.clone();
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
    fn expect(&mut self, want: Option<&Ty>, got: Option<Ty>) -> Option<Ty> {
        if let (Some(w), Some(g)) = (want, &got) {
            if *g != Ty::Never && *w != Ty::Never && !self.same(w, g) {
                self.error(format!("expected `{}`, found `{}`", show(w), show(g)));
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
                let (value, vt) = self.expr(value, ty.as_ref());
                let bound = ty.clone().or(vt);
                if *mutable && bound.is_none() {
                    self.error(format!(
                        "the type of `let mut {}` is not known from its first value; write `let mut {}: T`",
                        name.as_str(),
                        name.as_str()
                    ));
                }
                // TS infers a `let` from its first value (`null` stays `null`),
                // and a value printed as statements is declared before it is
                // assigned, so both need the type written out.
                let annotation = ty.clone().or_else(|| {
                    bound.clone().filter(|t| {
                        *t != Ty::Never
                            && (*mutable
                                || value.needs_statements() && !matches!(value, Expr::Try { .. }))
                    })
                });
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
            Expr::Construct {
                ty,
                variant,
                fields,
            } => {
                let fields = self.construct_fields(ty.as_str(), variant.as_ref().map(|v| v.as_str()), fields);
                let e = Expr::Construct {
                    ty: ty.clone(),
                    variant: variant.clone(),
                    fields,
                };
                (e, self.expect(want, Some(Ty::Named(ty.clone()))))
            }
            Expr::Field { base, name } => {
                let (base, bt) = self.expr(base, None);
                let t = bt.and_then(|bt| match self.norm(&bt) {
                    Ty::Named(s) => self
                        .defs
                        .structs
                        .get(s.as_str())
                        .and_then(|st| st.fields.iter().find(|f| f.name == *name))
                        .map(|f| f.ty.clone()),
                    _ => None,
                });
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
                    self.error(format!(
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
                self.error(format!(
                    "`?` on `{}` in a function returning `{}`",
                    show(&t),
                    show(&ret)
                ));
                (None, None)
            }
            (Some(t), _) => {
                self.error(format!("`?` needs a `Result` or `Option`, found `{}`", show(&t)));
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
                        self.error(format!("integer literal `{shown}` where a float is expected; write `{shown}.0`"));
                        return (Expr::Lit(lit.clone()), None);
                    }
                    (None, None) if want.is_some() => {
                        let w = show(want.expect("checked"));
                        self.error(format!("expected `{w}`, found integer literal `{shown}`"));
                        return (Expr::Lit(lit.clone()), None);
                    }
                    (None, None) => {
                        self.error(format!(
                            "cannot tell the integer type of `{shown}`; add a suffix like `{shown}i32` or annotate the binding"
                        ));
                        return (Expr::Lit(lit.clone()), None);
                    }
                };
                let t = ty.expect("set above");
                let (lo, hi) = t.bounds();
                if value < lo || value > hi {
                    self.error(format!("literal `{shown}` does not fit in `{}`", t.as_str()));
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
                        self.error(format!("expected `{w}`, found float literal `{digits}`"));
                        return (Expr::Lit(lit.clone()), None);
                    }
                    (None, _) => {
                        self.error(format!(
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
                    Expr::Unary {
                        op: UnOp::Neg,
                        expr: Box::new(e),
                    }
                } else {
                    e
                };
                (e, self.expect(want, Some(Ty::Prim(ty.into()))))
            }
            Lit::Bool(_) => (Expr::Lit(lit.clone()), self.expect(want, Some(Ty::bool()))),
            Lit::Str(_) => (
                Expr::Lit(lit.clone()),
                self.expect(want, Some(Ty::Prim(Prim::String))),
            ),
            Lit::Unit => (
                Expr::Lit(lit.clone()),
                self.expect(want, Some(Ty::Prim(Prim::Unit))),
            ),
            Lit::Null => (Expr::Lit(lit.clone()), want.cloned()),
        }
    }

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, want: Option<&Ty>) -> Typed {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                let (l, lt, r, rt) = self.pair(left, right, want);
                let t = join(lt, rt);
                let Some(t) = t.filter(|t| *t != Ty::Never) else {
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
                    Some(Num::Float(FloatTy::F64)) => rebuild(op, l, r),
                    None => {
                        self.error(format!("arithmetic on `{}` is not in v0", show(&t)));
                        rebuild(op, l, r)
                    }
                };
                (e, Some(t))
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let (l, lt, r, rt) = self.pair(left, right, None);
                if let Some(t) = join(lt, rt).filter(|t| *t != Ty::Never) {
                    let ordered = !matches!(op, BinOp::Eq | BinOp::Ne);
                    let ok = match self.norm(&t) {
                        _ if self.num(&t).is_some() => true,
                        Ty::Prim(Prim::Bool) | Ty::Prim(Prim::String) => !ordered,
                        _ => false,
                    };
                    if !ok {
                        let what = if ordered { "ordering" } else { "equality" };
                        self.error(format!(
                            "{what} on `{}` is not in v0; JS compares it differently",
                            show(&t)
                        ));
                    }
                }
                (rebuild(op, l, r), self.expect(want, Some(Ty::bool())))
            }
            BinOp::And | BinOp::Or => {
                let (l, _) = self.expr(left, Some(&Ty::bool()));
                let (r, _) = self.expr(right, Some(&Ty::bool()));
                (rebuild(op, l, r), self.expect(want, Some(Ty::bool())))
            }
        }
    }

    fn unary(&mut self, op: UnOp, inner: &Expr, want: Option<&Ty>) -> Typed {
        match op {
            UnOp::Neg => {
                if let Expr::Lit(lit @ (Lit::Int { .. } | Lit::Float { .. })) = inner {
                    return self.lit(lit, true, want);
                }
                let (e, t) = self.expr(inner, want);
                let Some(t) = t.filter(|t| *t != Ty::Never) else {
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
                    Some(Num::Float(_)) => neg(e),
                    _ => {
                        self.error(format!("cannot negate `{}`", show(&t)));
                        neg(e)
                    }
                };
                (e, Some(t))
            }
            UnOp::Not => {
                let (e, t) = self.expr(inner, Some(&Ty::bool()));
                let e = Expr::Unary {
                    op,
                    expr: Box::new(e),
                };
                (e, t.map(|_| Ty::bool()))
            }
        }
    }

    fn match_(&mut self, scrutinee: &Expr, arms: &[Arm], want: Option<&Ty>) -> Typed {
        let (scrutinee, st) = self.expr(scrutinee, None);
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
                    pattern: arm.pattern.clone(),
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
                    self.error(format!(
                        "pattern `{}` does not match a value of type `{}`",
                        describe(pattern),
                        show(t)
                    ));
                }
                (inner(p), None)
            }
            (Pattern::OptionNone, Some(t)) if !matches!(t, Ty::Option(_)) => {
                self.error(format!("pattern `None` does not match a value of type `{}`", show(t)));
                (None, None)
            }
            (Pattern::OptionNone | Pattern::Wildcard | Pattern::Lit(_), _) => (None, None),
            (Pattern::Variant { .. }, _) => return self.bind_variant(pattern, scrutinee),
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
            self.error(format!(
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

    fn call(&mut self, callee: &Callee, args: &[Expr], want: Option<&Ty>) -> Typed {
        let typed_args = |me: &mut Self, params: Vec<Ty>| -> Vec<Expr> {
            args.iter()
                .zip(params.iter().map(Some).chain(std::iter::repeat(None)))
                .map(|(a, p)| me.expr(a, p).0)
                .collect()
        };
        let (args, t) = match callee {
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
                        let t = typed.first().and_then(|(_, t)| t.clone()).map(Ty::option);
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
                (typed_args(self, vec![t.clone(); op.arity()]), Some(t))
            }
            Callee::Fround => (
                typed_args(self, vec![Ty::Prim(Prim::F64)]),
                Some(Ty::Prim(Prim::F32)),
            ),
        };
        let e = Expr::Call {
            callee: callee.clone(),
            args,
        };
        (e, self.expect(want, t))
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

fn show(ty: &Ty) -> String {
    match ty {
        Ty::Prim(p) => match p {
            Prim::Bool => "bool".into(),
            Prim::String => "String".into(),
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
        Ty::Never => "!".into(),
    }
}
