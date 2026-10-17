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
    Arm, BinOp, Callee, CharMethod, ClosureParam, Cmp, Const, Consume, Crate, Expr, Fields, FloatMethod, FloatTy,
    FmtAlign, FmtPiece, FmtSpec, Fn, IntMethod, IntOp, IntTy, Item, Lit, Name, Over, Pattern, Pick, Prim, Reason, Same,
    SliceOf, Sort, StrMethod, TraitMethod, TreeOp, TryOn, Ty, UnOp, VariantBind, VariantFields,
};

use crate::defs::{Defs, Peel};

mod access;
mod calls;
mod convert;
mod format;
mod generic;
mod iter;
mod methods;
mod ops;
mod option_result;
mod ordering;
mod patterns;
mod prim;
mod traits;
mod tree;

use crate::Diagnostic;
use calls::*;
use ops::*;
use ordering::*;
use prim::*;

pub fn elaborate(krate: &Crate) -> Result<Crate, Vec<Diagnostic>> {
    let defs = Defs::of(krate);
    let mut out = Vec::new();
    let items = krate
        .items
        .iter()
        .enumerate()
        .map(|(i, item)| match item {
            Item::Fn(f) => Item::Fn(Typer::new(&defs, i, &mut out).func(f)),
            Item::Const(c) if matches!(&c.ty, Ty::Vec(t) if **t == Ty::Prim(Prim::U8)) => {
                match crate::consts::bytes(&c.value) {
                    Some(value) => Item::Const(Const { value, ..c.clone() }),
                    None => {
                        out.push(Diagnostic::at(
                            i,
                            Reason::ConstExpr,
                            format!("const `{}`: a `&[u8]` const is a byte string `b\"..\"` in v0", c.name.as_str()),
                        ));
                        item.clone()
                    }
                }
            }
            // A crate const may also be a tuple, an enum, or an open struct
            // (design/01 §7.29); a generic type is refused there, by name.
            Item::Const(c)
                if c.owner.is_none()
                    && matches!(
                        defs.expand(&c.ty, Peel::Every),
                        Ty::Array { .. } | Ty::Tuple(_) | Ty::Named(_) | Ty::App { .. }
                    ) =>
            {
                match crate::consts::value(&defs, &c.ty, &c.value) {
                    Ok(value) => Item::Const(Const { value, ..c.clone() }),
                    Err(m) => {
                        out.push(Diagnostic::at(i, Reason::ConstExpr, format!("const `{}`: {m}", c.name.as_str())));
                        item.clone()
                    }
                }
            }
            Item::Const(c) if matches!(defs.expand(&c.ty, Peel::Every), Ty::Array { .. }) => {
                match crate::consts::array(&defs, &c.ty, &c.value) {
                    Ok(value) => Item::Const(Const { value, ..c.clone() }),
                    Err(m) => {
                        out.push(Diagnostic::at(i, Reason::ConstExpr, format!("const `{}`: {m}", c.name.as_str())));
                        item.clone()
                    }
                }
            }
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
                                v.discriminant =
                                    Some(Expr::Lit(Lit::Int { value: d, ty: Some(it), byte: false, hex: false }));
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
    /// For each `loop` around the expression, innermost last: the type its
    /// `break`s carry, once known.
    loops: Vec<Option<Ty>>,
    /// `let` bindings of an unsuffixed integer whose type a later use is
    /// looked for in (`Let`): each one's index in `scopes`, and the first
    /// integer type a use asks of it.
    open: Vec<(usize, Option<Ty>)>,
    /// The dictionaries the function takes (design/01 §7.25).
    dicts: Vec<traits::Dict>,
}

type Typed = (Expr, Option<Ty>);

/// A typed value bound to a name: the name, its type, and the value.
type Bound = (Name, Option<Ty>, Expr);

impl<'d, 'a> Typer<'d, 'a> {
    fn new(defs: &'d Defs<'a>, item: usize, out: &'d mut Vec<Diagnostic>) -> Self {
        Typer {
            defs,
            item,
            out,
            ret: Ty::Prim(Prim::Unit),
            scopes: Vec::new(),
            fresh: 0,
            loops: Vec::new(),
            open: Vec::new(),
            dicts: Vec::new(),
        }
    }

    fn error(&mut self, reason: Reason, message: String) {
        self.out.push(Diagnostic::at(self.item, reason, message));
    }

    fn func(mut self, f: &Fn) -> Fn {
        self.ret = f.ret.clone();
        self.dicts = self.dicts_of(f);
        self.scopes = f.params.iter().map(|p| (p.name.as_str().to_string(), Some(p.ty.clone()))).collect();
        let ret = f.ret.clone();
        let (body, _) = self.expr(&f.body, Some(&ret));
        Fn { body, ..f.clone() }
    }

    /// `S { .. }` or a variant: `S { a, ..base }` of a generic `S` takes
    /// its type from the base. Apart from `expr`, whose frame a chain of
    /// calls holds at every link.
    #[inline(never)]
    fn construct_expr(
        &mut self,
        ty: &Name,
        variant: Option<&Name>,
        fields: &Fields,
        base: Option<&Expr>,
        want: Option<&Ty>,
    ) -> Typed {
        // `..Default::default()` is the struct's own, whatever the context.
        let own = match base.map(Expr::unpositioned) {
            Some(Expr::Call { callee: Callee::Default { .. }, .. })
                if self.defs.generics_of(ty.as_str()) == Some(0) =>
            {
                Some(Ty::Named(ty.clone()))
            }
            _ => None,
        };
        let base_typed = base.map(|b| self.expr(b, own.as_ref().or(want)));
        let hint = base_typed.as_ref().and_then(|(_, t)| t.clone()).or_else(|| want.cloned());
        let (fields, t) = self.construct(ty, variant.map(|v| v.as_str()), fields, hint.as_ref());
        let e = Expr::Construct {
            ty: ty.clone(),
            variant: variant.cloned(),
            fields,
            base: base_typed.map(|(b, _)| Box::new(b)),
        };
        (e, self.expect(want, t))
    }

    /// A struct field's type, at the struct's type arguments.
    #[inline(never)]
    fn field_ty(&self, base: &Ty, name: &Name) -> Option<Ty> {
        let base = self.norm(base);
        if let Some(t) = self.dict_field(&base, name) {
            return Some(t);
        }
        let binds = self.bindings_of(&base);
        let (s, _) = base.applied()?;
        let st = self.defs.structs.get(s.as_str())?;
        st.fields.iter().find(|f| f.name == *name).map(|f| f.ty.subst(&binds))
    }

    /// Expands aliases. Alias cycles are cut after a fixed depth. An array
    /// `[T; N]` is read as the slice it derefs to, as every read of one is
    /// a slice's (design/01 §7.27); `array_len` keeps its length.
    fn norm(&self, ty: &Ty) -> Ty {
        match self.defs.expand(ty, Peel::First) {
            Ty::Array { elem, .. } => Ty::Vec(elem.clone()),
            other => other.clone(),
        }
    }

    /// The length of an array `[T; N]`, through aliases.
    fn array_len(&self, ty: &Ty) -> Option<usize> {
        match self.defs.expand(ty, Peel::First) {
            Ty::Array { len, .. } => Some(*len),
            _ => None,
        }
    }

    fn num(&self, ty: &Ty) -> Option<Num> {
        num_of(&self.norm(ty))
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
        self.scopes.iter().rev().find(|(n, _)| n == name).and_then(|(_, t)| t.clone())
    }

    /// The `open` entry `name` reads, while no use has given it a type.
    fn open_slot(&self, name: &str) -> Option<usize> {
        let at = self.scopes.iter().rposition(|(n, _)| n == name)?;
        self.open.iter().position(|(i, t)| *i == at && t.is_none())
    }

    /// Whether `expr`'s numeric type comes only from its surroundings: an
    /// unsuffixed literal, or a binding of one whose type is looked for.
    fn contextual(&self, expr: &Expr) -> bool {
        needs_context_in(expr, &|n| self.open_slot(n.as_str()).is_some())
    }

    /// The integer type the first use of `name` in `then` asks of it, `name`
    /// bound to an unsuffixed integer (`let mut s = 0; .. s += i; ..`), as
    /// rustc infers it; `i32`, rustc's fallback, where no use asks one
    /// (`n += 1; n == 0`). `then` is typed once to find it, then dropped
    /// with what it reported; the caller types it again with the type found,
    /// so every use is checked against it.
    fn first_use_type(&mut self, name: &Name, then: &Expr, want: Option<&Ty>) -> Option<Ty> {
        self.first_use_types(&[(name.clone(), None)], then, want).pop()
    }

    /// `first_use_type` for several names `then` sees at once, each with
    /// its type where it has one (`let (dr, rank, ok) = ..`, `ok` a `bool`):
    /// the open ones' types, in order, those with one as given.
    fn first_use_types(&mut self, names: &[(Name, Option<Ty>)], then: &Expr, want: Option<&Ty>) -> Vec<Ty> {
        let (out, fresh, loops) = (self.out.len(), self.fresh, self.loops.clone());
        let (depth, opened) = (self.scopes.len(), self.open.len());
        for (name, ty) in names {
            self.scopes.push((name.as_str().to_string(), ty.clone()));
            if ty.is_none() {
                self.open.push((self.scopes.len() - 1, None));
            }
        }
        let _ = self.expr(then, want);
        let found = self.open.drain(opened..).map(|(_, t)| t.unwrap_or(Ty::Prim(Prim::I32)));
        let mut found = found.collect::<Vec<_>>().into_iter();
        let tys = names.iter().map(|(_, ty)| ty.clone().unwrap_or_else(|| found.next().expect("one each"))).collect();
        self.scopes.truncate(depth);
        self.out.truncate(out);
        (self.fresh, self.loops) = (fresh, loops);
        tys
    }

    /// The type `expr` has alone, what it reported dropped.
    fn type_alone(&mut self, expr: &Expr) -> Option<Ty> {
        let (out, fresh, loops) = (self.out.len(), self.fresh, self.loops.clone());
        let (_, t) = self.expr(expr, None);
        let clean = self.out.len() == out;
        self.out.truncate(out);
        (self.fresh, self.loops) = (fresh, loops);
        t.filter(|_| clean)
    }

    /// `body` typed with the names `lets` binds in scope.
    fn scoped(&mut self, lets: &[Bound], body: &Expr, want: Option<&Ty>) -> Typed {
        let depth = self.scopes.len();
        self.scopes.extend(lets.iter().map(|(name, ty, _)| (name.as_str().to_string(), ty.clone())));
        let typed = self.expr(body, want);
        self.scopes.truncate(depth);
        typed
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
                (Expr::Ignored { wrapper: *wrapper, expr: Box::new(expr) }, ty)
            }
            Expr::Lit(lit) => self.lit(lit, false, want),
            Expr::Var(n) if !self.is_local(n.as_str()) && purecrate_ir::split_assoc_path(n.as_str()).is_some() => {
                match self.bound_const(n.as_str(), want) {
                    Some(typed) => typed,
                    None => {
                        let t = self.defs.consts.get(n.as_str()).map(|c| c.ty.clone());
                        (expr.clone(), self.expect(want, t))
                    }
                }
            }
            Expr::Var(n) => {
                if let (Some(slot), Some(w)) = (self.open_slot(n.as_str()), want) {
                    if int_of(&self.norm(w)).is_some() {
                        self.open[slot].1 = Some(w.clone());
                    }
                }
                let t = self.lookup(n.as_str()).or_else(|| {
                    let bound = self.scopes.iter().any(|(s, _)| s == n.as_str());
                    (!bound).then(|| self.defs.consts.get(n.as_str()).map(|c| c.ty.clone())).flatten()
                });
                (expr.clone(), self.expect(want, t))
            }
            Expr::Cast { expr: inner, to } => self.cast(inner, to, want),
            // `loop`: of the type its `break`s carry, `()` when they carry
            // nothing, and `!` when nothing breaks out of it.
            Expr::While { cond, body } if **cond == Expr::Lit(Lit::Bool(true)) => {
                self.loops.push(want.filter(|w| **w != Ty::Never).cloned());
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                let carried = self.loops.pop().flatten();
                let ty = match breaks(body) {
                    (true, _) => carried,
                    (false, true) => Some(Ty::Prim(Prim::Unit)),
                    (false, false) => Some(Ty::Never),
                };
                let e = Expr::While { cond: cond.clone(), body: Box::new(b) };
                (e, self.expect(want, ty))
            }
            Expr::While { cond, body } => {
                let (c, _) = self.expr(cond, Some(&Ty::bool()));
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                let e = Expr::While { cond: Box::new(c), body: Box::new(b) };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            Expr::Break(Some(value)) => {
                let slot = self.loops.last().cloned().flatten();
                let (v, t) = self.expr(value, slot.as_ref());
                if let (Some(top), None, Some(t)) = (self.loops.last_mut(), slot, t.filter(|t| *t != Ty::Never)) {
                    *top = Some(t);
                }
                (Expr::Break(Some(Box::new(v))), Some(Ty::Never))
            }
            Expr::Break(None) | Expr::Continue => (expr.clone(), Some(Ty::Never)),
            Expr::Let { name, mutable, ty, value, then } => {
                // `let mut s = 0;`: the type of the first use that asks for
                // one, as rustc infers it, where nothing else gives one.
                // Inside that search, a nested one is not searched again.
                let inferred = match ty {
                    None if is_bare_int_valued(value) && self.open.is_empty() => self.first_use_type(name, then, want),
                    None => match then.unpositioned() {
                        Expr::Match { scrutinee, arms } if matches!(scrutinee.unpositioned(), Expr::Var(n) if n == name) => {
                            self.literal_tuple_type(value, arms, want)
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let before = self.out.len();
                let (value, vt) = self.expr(value, ty.as_ref().or(inferred.as_ref()));
                // A type with holes (`collect::<Vec<_>>()`) is what the value
                // filled them with.
                let ty = &ty.clone().filter(|t| !has_hole(t));
                let bound = ty.clone().or(vt);
                if bound.is_none() && self.out.len() == before {
                    let (kw, first) = if *mutable { ("let mut", " from its first value") } else { ("let", "") };
                    self.error(
                        Reason::NeedsAnnotation,
                        format!("the type of `{kw} {n}` is not known{first}; write `{kw} {n}: T`", n = name.as_str(),),
                    );
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
            Expr::For { var, start, end, inclusive, rev, body, .. } => {
                // `0..n`: an unsuffixed start takes the end's type. Only the
                // typing order changes; the start still runs first.
                let before = self.out.len();
                // `0..3`: both bounds unsuffixed take the type the first use
                // of the variable in the body asks (`v[i]`, a `usize`), as an
                // unsuffixed `let` does (§7.22).
                let inferred = match is_bare_int(start) && is_bare_int(end) && self.open.is_empty() {
                    true => self.first_use_type(var, body, Some(&Ty::Prim(Prim::Unit))),
                    false => None,
                };
                let ((s, st), (e, et)) = if is_bare_int(start) {
                    let (e, et) = self.expr(end, inferred.as_ref());
                    (self.expr(start, et.as_ref()), (e, et))
                } else {
                    let (s, st) = self.expr(start, None);
                    let e = self.expr(end, st.as_ref());
                    ((s, st), e)
                };
                let bound = st.clone().or(et);
                let int = bound.as_ref().and_then(|t| int_of(&self.norm(t)));
                if int.is_none() && self.out.len() == before {
                    let found = bound.as_ref().map(show).unwrap_or_else(|| "?".into());
                    self.error(
                        Reason::TypeMismatch,
                        format!("`for` takes a range `a..b` of one integer type, found `{found}`"),
                    );
                }
                self.scopes.push((var.as_str().to_string(), bound));
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                self.scopes.pop();
                let (start, end, body) = (Box::new(s), Box::new(e), Box::new(b));
                let e = Expr::For { var: var.clone(), ty: int, start, end, inclusive: *inclusive, rev: *rev, body };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            // Over a sequence: its stages run as the loop asks for each item.
            Expr::ForEach { var, over: Over::Items, source, body } if iter::is_sequence(source) => {
                let walked = if self.reorder_reported(source) { None } else { self.sequence(source, "for") };
                let (s, over, item) = match walked {
                    Some((s, over, item)) => (s, over, Some(item)),
                    None => (Expr::Lit(Lit::Unit), Over::Items, None),
                };
                self.scopes.push((var.as_str().to_string(), item));
                let (b, _) = self.expr(body, Some(&Ty::Prim(Prim::Unit)));
                self.scopes.pop();
                let e = Expr::ForEach { var: var.clone(), over, source: Box::new(s), body: Box::new(b) };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            Expr::ForEach { var, over, source: string, body } => {
                let before = self.out.len();
                // `for d in [-1, 1]`: unsuffixed elements take the type the
                // first use of the variable in the body asks, as a `let` of
                // one does.
                let inferred = match bare_int_array(string) && *over == Over::Items && self.open.is_empty() {
                    true => self.first_use_type(var, body, Some(&Ty::Prim(Prim::Unit))),
                    false => None,
                };
                let (s, st) = self.expr(string, inferred.map(|t| Ty::Vec(Box::new(t))).as_ref());
                // `for (k, v) in &m`, `for k in &s`: the items in key order.
                let (s, st) = match st.as_ref().map(|t| self.norm(t)) {
                    Some(t @ Ty::Map { .. }) if *over == Over::Items => self.tree_items(TreeOp::Entries, s, t),
                    Some(t @ Ty::Set(_)) if *over == Over::Items => self.tree_items(TreeOp::Keys, s, t),
                    _ => (s, st),
                };
                let item = st.as_ref().and_then(|t| self.walked(*over, t));
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
                let e = Expr::ForEach { var: var.clone(), over: *over, source: Box::new(s), body: Box::new(b) };
                (e, self.expect(want, Some(Ty::Prim(Prim::Unit))))
            }
            Expr::Assign { name, value } => {
                let target = self.lookup(name.as_str());
                let (value, _) = self.expr(value, target.as_ref());
                let e = Expr::Assign { name: name.clone(), value: Box::new(value) };
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
                let e = Expr::Seq { first: Box::new(first), then: Box::new(then) };
                (e, tt)
            }
            Expr::If { cond, then, else_ } => {
                let (cond, _) = self.expr(cond, Some(&Ty::bool()));
                let (then, tt, else_, et) = self.pair(then, else_, want);
                (Expr::If { cond: Box::new(cond), then: Box::new(then), else_: Box::new(else_) }, join(tt, et))
            }
            Expr::Match { scrutinee, arms } => self.match_(scrutinee, arms, want),
            Expr::Call { callee, args } => self.call(callee, args, want),
            // `[a, b]`: rustc's `[T; 2]`, or the slice a `&[T]` wants.
            Expr::MethodCall { receiver, name, .. } if name.as_str() == purecrate_ir::ARRAY_LITERAL => {
                let (e, t) = self.expr(receiver, want);
                let slice = want.is_some_and(|w| matches!(self.norm(w), Ty::Vec(_)));
                let t = match (t, &e) {
                    (Some(Ty::Vec(elem)), Expr::Array(xs)) if !slice => Some(Ty::Array { elem, len: xs.len() }),
                    (t, _) => t,
                };
                (e, t)
            }
            Expr::MethodCall { receiver, name, args } => self.method_call(receiver, name, args, want),
            Expr::Closure { params, ret, body } => self.closure(params, ret.as_ref(), body, want),
            Expr::Construct { ty, variant, fields, base } => {
                self.construct_expr(ty, variant.as_ref(), fields, base.as_deref(), want)
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
                (Expr::Index { base: Box::new(base), index: Box::new(index) }, self.expect(want, elem))
            }
            Expr::Field { base, name } => {
                let (base, bt) = self.expr(base, None);
                // `t.0` of a tuple: its element, read as `t[0]`.
                if let (Ok(i), Some(Ty::Tuple(ts))) =
                    (name.as_str().parse::<usize>(), bt.as_ref().map(|t| self.norm(t)))
                {
                    let Some(t) = ts.get(i).cloned() else {
                        self.error(Reason::TupleField, format!("a tuple of {} elements has no `.{i}`", ts.len()));
                        return (expr.clone(), None);
                    };
                    let e = Expr::Field { base: Box::new(base), name: Name::new(format!("[{i}]")) };
                    return (e, self.expect(want, Some(t)));
                }
                let t = bt.as_ref().and_then(|bt| self.field_ty(bt, name));
                if let (None, Some(bt)) = (&t, &bt) {
                    if name.as_str().parse::<usize>().is_ok() {
                        self.error(
                            Reason::TupleField,
                            format!(
                                "`.{}` is only in v0 on a tuple, or `.0` on a one-field tuple struct, not on `{}`",
                                name.as_str(),
                                show(bt)
                            ),
                        );
                    }
                }
                let e = Expr::Field { base: Box::new(base), name: name.clone() };
                (e, self.expect(want, t))
            }
            Expr::Tuple(xs) => {
                let wants: Vec<Option<Ty>> = match want.map(|w| self.norm(w)) {
                    Some(Ty::Tuple(ts)) if ts.len() == xs.len() => ts.into_iter().map(Some).collect(),
                    _ => vec![None; xs.len()],
                };
                let (elems, tys): (Vec<Expr>, Vec<Option<Ty>>) =
                    xs.iter().zip(&wants).map(|(x, w)| self.expr(x, w.as_ref())).unzip();
                let t = tys.into_iter().collect::<Option<Vec<Ty>>>().map(Ty::Tuple);
                (Expr::Tuple(elems), self.expect(want, t))
            }
            Expr::Array(xs) => {
                let mut elem = match want.map(|w| self.norm(w)) {
                    Some(Ty::Vec(t)) => Some(*t),
                    _ => None,
                };
                let len = want.and_then(|w| self.array_len(w));
                if let Some(n) = len.filter(|n| *n != xs.len()) {
                    self.error(
                        Reason::TypeMismatch,
                        format!("expected an array of {n} elements, found one of {}", xs.len()),
                    );
                }
                if elem.is_none() {
                    if let Some(first) = xs.iter().find(|x| !needs_context(x)) {
                        elem = self.expr(first, None).1;
                    }
                }
                let elems = xs.iter().map(|x| self.expr(x, elem.as_ref()).0).collect();
                // An array where one is wanted (`[T; N]`), else a `Vec`.
                let t = match len {
                    Some(n) => elem.map(|t| Ty::Array { elem: Box::new(t), len: n }),
                    None => elem.map(|t| Ty::Vec(Box::new(t))),
                };
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
            Expr::Comment(lines) => (Expr::Comment(lines.clone()), Some(Ty::Prim(Prim::Unit))),
        }
    }

    /// An operator whose operand types stayed unknown would print as the raw
    /// JS operator. Not reported when this item already failed: the unknown
    /// type most likely comes from a binding reported there.
    fn unknown(&mut self, what: &str) {
        if !self.out.iter().any(|d| d.item == self.item) {
            self.error(
                Reason::NeedsAnnotation,
                format!("the operand types of this {what} are not known here; annotate the binding they come from"),
            );
        }
    }
}

/// Whether `ty` has a `_` for the checker to fill.
pub(crate) fn has_hole(ty: &Ty) -> bool {
    ty.any(&|t| matches!(t, Ty::Named(n) if n.as_str() == "_"))
}

/// The integer type `ty` is, as written (no alias followed).
pub(crate) fn int_of(ty: &Ty) -> Option<IntTy> {
    match ty {
        Ty::Prim(p) => p.int(),
        _ => None,
    }
}

/// The float type `ty` is, as written (no alias followed).
fn float_of(ty: &Ty) -> Option<FloatTy> {
    match ty {
        Ty::Prim(p) => p.float(),
        _ => None,
    }
}

/// The numeric type `ty` is, as written (no alias followed).
fn num_of(ty: &Ty) -> Option<Num> {
    int_of(ty).map(Num::Int).or_else(|| float_of(ty).map(Num::Float))
}

/// Expressions whose numeric type comes only from their surroundings.
fn needs_context(expr: &Expr) -> bool {
    needs_context_in(expr, &|_| false)
}

/// `needs_context`, with the bindings `open` names (of an unsuffixed
/// integer, their type not yet known) as such expressions too.
fn needs_context_in(expr: &Expr, open: &dyn std::ops::Fn(&Name) -> bool) -> bool {
    let needs = |e: &Expr| needs_context_in(e, open);
    match expr.unpositioned() {
        Expr::Lit(Lit::Int { ty: None, .. } | Lit::Float { ty: None, .. }) => true,
        Expr::Var(n) => open(n),
        Expr::Unary { op: UnOp::Neg, expr } => needs(expr),
        Expr::Binary { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem, left, right } => {
            needs(left) && needs(right)
        }
        Expr::Binary { op, left, right } if op.is_bitwise() => needs(left) && needs(right),
        Expr::Binary { op, left, .. } if op.is_shift() => needs(left),
        Expr::Unary { op: UnOp::Not, expr } => needs(expr),
        Expr::If { then, else_, .. } => needs(then) && needs(else_),
        Expr::Let { then, .. } | Expr::Seq { then, .. } => needs(then),
        Expr::Array(xs) => xs.iter().all(needs),
        Expr::MethodCall { receiver, name, .. } if name.as_str() == purecrate_ir::ARRAY_LITERAL => needs(receiver),
        _ => false,
    }
}

/// A value whose integer type comes only from where it goes: an unsuffixed
/// literal, or an `if` / `match` / block each of whose results is one or
/// diverges (`let n = match k { K::A => 1, _ => 2 };`), at least one a literal.
fn is_bare_int_valued(expr: &Expr) -> bool {
    fn results(expr: &Expr) -> Option<bool> {
        match expr.unpositioned() {
            e if is_bare_int(e) => Some(true),
            Expr::Return(_) | Expr::Break(_) | Expr::Continue | Expr::Unreachable => Some(false),
            Expr::Call { callee: Callee::Panic, .. } => Some(false),
            Expr::If { then, else_, .. } => Some(results(then)? | results(else_)?),
            Expr::Match { arms, .. } => arms.iter().try_fold(false, |any, a| Some(any | results(&a.body)?)),
            Expr::Let { then, .. } | Expr::Seq { then, .. } => results(then),
            _ => None,
        }
    }
    results(expr) == Some(true)
}

/// The tuples a value results in, a diverging result adding none:
/// `match c { W => (1, 7), B => (-1, 0) }`, `if b { (1, x) } else { return }`.
fn tuple_results(expr: &Expr) -> Option<Vec<&[Expr]>> {
    match expr.unpositioned() {
        Expr::Tuple(xs) => Some(vec![xs.as_slice()]),
        Expr::Return(_) | Expr::Break(_) | Expr::Continue | Expr::Unreachable => Some(vec![]),
        Expr::Call { callee: Callee::Panic, .. } => Some(vec![]),
        Expr::If { then, else_, .. } => Some([tuple_results(then)?, tuple_results(else_)?].concat()),
        Expr::Match { arms, .. } => {
            arms.iter().try_fold(vec![], |all, a| Some([all, tuple_results(&a.body)?].concat()))
        }
        Expr::Let { then, .. } | Expr::Seq { then, .. } => tuple_results(then),
        _ => None,
    }
}

/// `[-1, 1]`: an array literal of unsuffixed integers.
fn bare_int_array(expr: &Expr) -> bool {
    match expr.unpositioned() {
        Expr::MethodCall { receiver, name, .. } if name.as_str() == purecrate_ir::ARRAY_LITERAL => {
            matches!(receiver.unpositioned(), Expr::Array(xs) if !xs.is_empty() && xs.iter().all(is_bare_int))
        }
        _ => false,
    }
}

/// `lets` bound around `body`, the first outermost.
fn wrap_lets(lets: Vec<Bound>, body: Expr) -> Expr {
    lets.into_iter().rev().fold(body, |then, (name, ty, value)| Expr::Let {
        name,
        mutable: false,
        ty,
        value: Box::new(value),
        then: Box::new(then),
    })
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
            Prim::ParseIntError => "ParseIntError".into(),
            Prim::Unit => "()".into(),
            Prim::F32 => "f32".into(),
            Prim::F64 => "f64".into(),
            other => other.int().map(|t| t.as_str()).unwrap_or("?").into(),
        },
        Ty::Option(t) => format!("Option<{}>", show(t)),
        Ty::Result { ok, err } => format!("Result<{}, {}>", show(ok), show(err)),
        Ty::Vec(t) => format!("Vec<{}>", show(t)),
        Ty::Array { elem, len } => format!("[{}; {len}]", show(elem)),
        Ty::Map { key, value } => format!("BTreeMap<{}, {}>", show(key), show(value)),
        Ty::Set(t) => format!("BTreeSet<{}>", show(t)),
        Ty::App { name, args } => {
            format!("{}<{}>", name.as_str(), args.iter().map(show).collect::<Vec<_>>().join(", "))
        }
        Ty::Param(n) => n.as_str().to_string(),
        Ty::Tuple(ts) => format!("({})", ts.iter().map(show).collect::<Vec<_>>().join(", ")),
        Ty::Named(n) => n.as_str().to_string(),
        Ty::Fn { params, ret } => {
            format!("impl Fn({}) -> {}", params.iter().map(show).collect::<Vec<_>>().join(", "), show(ret))
        }
        Ty::Ignored { wrapper, inner } => format!("{}<{}>", wrapper.rust_name(), show(inner)),
        Ty::Never => "!".into(),
    }
}

/// Whether a `break` of this loop (not of a loop or closure inside it) in
/// `body` carries a value, and whether one carries none.
fn breaks(body: &Expr) -> (bool, bool) {
    fn walk(e: &Expr, found: &mut (bool, bool)) {
        match e {
            Expr::For { .. } | Expr::ForEach { .. } | Expr::While { .. } | Expr::Closure { .. } => {}
            Expr::Break(v) => {
                if v.is_some() {
                    found.0 = true;
                } else {
                    found.1 = true;
                }
                v.iter().for_each(|v| walk(v, found));
            }
            other => other.children().into_iter().for_each(|c| walk(c, found)),
        }
    }
    let mut found = (false, false);
    walk(body, &mut found);
    found
}
