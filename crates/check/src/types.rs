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
    Arm, BinOp, Callee, CharMethod, ClosureParam, IntMethod, Const, Crate, Expr, Over, Fields, FloatTy, Fn, IntOp, IntTy, Item, Lit, Name, Pattern, Prim, SliceOf, StrMethod, TryOn,
    Ty, UnOp, VariantBind, VariantFields, NEWTYPE_FIELD,
};

use crate::defs::Defs;

mod calls;
mod methods;
mod ops;
mod ordering;
mod patterns;

use calls::*;
use methods::*;
use ops::*;
use ordering::*;
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
            Item::Const(c) => match crate::consts::fold(&defs, &c.ty, &c.value) {
                Ok(lit) => Item::Const(Const { value: Expr::Lit(lit), ..c.clone() }),
                Err(m) => {
                    out.push(Diagnostic::at(i, Reason::ConstExpr, format!("const `{}`: {m}", c.name.as_str())));
                    item.clone()
                }
            },
            Item::Enum(e) if e.variants.iter().any(|v| v.discriminant.is_some()) || e.repr.is_some() => {
                match crate::consts::discriminants(&defs, e) {
                    Ok(table) => {
                        let it = e.repr.unwrap_or(IntTy::I64);
                        let mut folded = e.clone();
                        for (v, (_, d)) in folded.variants.iter_mut().zip(table) {
                            if v.discriminant.is_some() {
                                v.discriminant = Some(Expr::Lit(Lit::Int { value: d, ty: Some(it) }));
                            }
                        }
                        Item::Enum(folded)
                    }
                    Err(m) => {
                        out.push(Diagnostic::at(i, Reason::ConstExpr, m));
                        item.clone()
                    }
                }
            }
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
    /// Names made up by `tuple::lower` so far in this function.
    fresh: usize,
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
            fresh: 0,
        }
    }

    fn error(&mut self, reason: Reason, message: String) {
        self.out.push(Diagnostic::at(self.item, reason, message));
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
                let t = self.lookup(n.as_str()).or_else(|| {
                    let bound = self.scopes.iter().any(|(s, _)| s == n.as_str());
                    (!bound).then(|| self.defs.consts.get(n.as_str()).map(|c| c.ty.clone())).flatten()
                });
                (expr.clone(), self.expect(want, t))
            }
            Expr::Cast { expr: inner, to } => self.cast(inner, to, want),
            Expr::While { cond, body } => {
                let (c, _) = self.expr(cond, Some(&Ty::bool()));
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                let e = Expr::While { cond: Box::new(c), body: Box::new(b) };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            Expr::Break | Expr::Continue => (expr.clone(), Some(Ty::Never)),
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
            Expr::ForEach { var, over, source: string, body } => {
                let before = self.out.len();
                let (s, st) = self.expr(string, None);
                let norm = st.as_ref().map(|t| self.norm(t));
                let item = match (over, &norm) {
                    (Over::Chars, Some(Ty::Prim(Prim::String | Prim::Str))) => Some(Ty::Prim(Prim::Char)),
                    (Over::Bytes, Some(Ty::Prim(Prim::String | Prim::Str))) => Some(Ty::Prim(Prim::U8)),
                    (Over::Items, Some(Ty::Vec(t))) => Some((**t).clone()),
                    _ => None,
                };
                if item.is_none() && self.out.len() == before {
                    let found = st.as_ref().map(show).unwrap_or_else(|| "?".into());
                    let what = match over {
                        Over::Chars => "`for c in s.chars()` takes a `String` or `&str`",
                        Over::Bytes => "`for b in s.bytes()` takes a `String` or `&str`",
                        Over::Items => "`for x in xs` takes a range `a..b`, a `Vec` or slice (`xs`, `&xs`, `xs.iter()`), `s.chars()`, or `s.bytes()`",
                    };
                    self.error(Reason::TypeMismatch, format!("{what}, found `{found}`"));
                }
                self.scopes.push((var.as_str().to_string(), item));
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                self.scopes.pop();
                let e = Expr::ForEach {
                    var: var.clone(),
                    over: *over,
                    source: Box::new(s),
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
        Expr::Array(xs) => xs.iter().all(needs_context),
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
