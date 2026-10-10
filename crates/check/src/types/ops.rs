//! Operators and literals: arithmetic and bitwise operators rewritten into
//! `Int` calls, comparisons, and literals typed from context.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// Types two expressions that must agree, e.g. `if` branches or the
    /// operands of `+`. An untyped literal borrows the other side's type.
    pub(super) fn pair(&mut self, a: &Expr, b: &Expr, want: Option<&Ty>) -> (Expr, Option<Ty>, Expr, Option<Ty>) {
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
    pub(super) fn compared(&mut self, a: &Expr, b: &Expr) -> (Expr, Option<Ty>, Expr, Option<Ty>) {
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

    pub(super) fn lit(&mut self, lit: &Lit, negated: bool, want: Option<&Ty>) -> Typed {
        let wanted = want.and_then(|w| self.num(w));
        match lit {
            Lit::Int { value, ty, byte, hex } => {
                let value = if negated { -value } else { *value };
                let shown = value.to_string();
                let ty = match (ty, wanted) {
                    (Some(t), _) => Some(*t),
                    (None, Some(Num::Int(t))) => Some(t),
                    (None, Some(Num::Float(_))) => {
                        self.error(
                            Reason::TypeMismatch,
                            format!("integer literal `{shown}` where a float is expected; write `{shown}.0`"),
                        );
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
                let e = Expr::Lit(Lit::Int { value, ty: Some(t), byte: *byte, hex: *hex });
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
                let e = Expr::Lit(Lit::Float { digits: digits.clone(), ty: Some(ty) });
                let e = if negated {
                    Expr::Call {
                        callee: Callee::AsFloat(ty),
                        args: vec![Expr::Unary { op: UnOp::Neg, expr: Box::new(e) }],
                    }
                } else {
                    e
                };
                (e, self.expect(want, Some(Ty::Prim(ty.into()))))
            }
            Lit::Bool(_) => (Expr::Lit(lit.clone()), self.expect(want, Some(Ty::bool()))),
            Lit::Str(_) => (Expr::Lit(lit.clone()), self.expect(want, Some(Ty::Prim(Prim::Str)))),
            Lit::Char(_) => (Expr::Lit(lit.clone()), self.expect(want, Some(Ty::Prim(Prim::Char)))),
            Lit::Unit => (Expr::Lit(lit.clone()), self.expect(want, Some(Ty::Prim(Prim::Unit)))),
            Lit::Null => (Expr::Lit(lit.clone()), want.cloned()),
        }
    }

    pub(super) fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, want: Option<&Ty>) -> Typed {
        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                let (l, lt, r, rt) = self.pair(left, right, want);
                let t = join(lt, rt);
                let Some(t) = t.filter(|t| *t != Ty::Never) else {
                    self.unknown("arithmetic");
                    return (rebuild(op, l, r), None);
                };
                let e = match self.num(&t) {
                    Some(Num::Int(it)) => {
                        Expr::Call { callee: Callee::Int { ty: it, op: int_op(op) }, args: vec![l, r] }
                    }
                    Some(Num::Float(FloatTy::F32)) => {
                        Expr::Call { callee: Callee::Fround, args: vec![rebuild(op, l, r)] }
                    }
                    Some(Num::Float(FloatTy::F64)) => {
                        Expr::Call { callee: Callee::AsFloat(FloatTy::F64), args: vec![rebuild(op, l, r)] }
                    }
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
                        Ty::Prim(Prim::Char | Prim::Uuid | Prim::String | Prim::Str) => true,
                        Ty::Prim(Prim::Bool) => !ordered,
                        // std's derived `PartialEq`: the same variant.
                        Ty::Named(_) if self.is_ordering(t) => !ordered,
                        // A derived `PartialEq` (rustc refuses `==` without
                        // one), compared part by part.
                        Ty::Named(_) | Ty::Option(_) | Ty::Result { .. } | Ty::Vec(_) | Ty::Tuple(_)
                            if !ordered && self.deep_comparable(t, &mut Vec::new()) =>
                        {
                            let eq = Expr::Call { callee: Callee::DeepEq, args: vec![l, r] };
                            let e =
                                if op == BinOp::Ne { Expr::Unary { op: UnOp::Not, expr: Box::new(eq) } } else { eq };
                            return (e, self.expect(want, Some(Ty::bool())));
                        }
                        _ => false,
                    };
                    if !ok {
                        let what = if ordered { "ordering" } else { "equality" };
                        let instead = match self.norm(t) {
                            _ if !ordered => {
                                "it holds a value JS cannot compare as Rust does (`ParseIntError`, `uuid::Error`, a function); compare the parts with a `match`".into()
                            }
                            Ty::Prim(Prim::Bool) => "use `a.cmp(&b)`, which orders `false` first".into(),
                            Ty::Named(n) => format!(
                                "use `matches!(x, {}::Variant)`, a `match`, or compare the parts and chain them with `then`",
                                n.as_str()
                            ),
                            _ => "compare the parts and chain them with `then` / `then_with`".into(),
                        };
                        self.error(
                            Reason::Comparison,
                            format!("{what} on `{}` is not in v0; JS compares it differently: {instead}", show(t)),
                        );
                    }
                }
                // JS orders strings by UTF-16 unit; `char` and `str` order by
                // code point.
                let norm = t.as_ref().map(|t| self.norm(t));
                if ordered && norm == Some(Ty::Prim(Prim::Char)) {
                    let code = |e| Expr::Call { callee: Callee::CharCode(IntTy::U32), args: vec![e] };
                    return (rebuild(op, code(l), code(r)), self.expect(want, Some(Ty::bool())));
                }
                if ordered && matches!(norm, Some(Ty::Prim(Prim::String | Prim::Str))) {
                    let cmp = Expr::Call { callee: Callee::StrCmp, args: vec![l, r] };
                    let zero = Expr::Lit(Lit::Int { value: 0, ty: Some(IntTy::I32), byte: false, hex: false });
                    return (rebuild(op, cmp, zero), self.expect(want, Some(Ty::bool())));
                }
                // An `Ordering` is an object: its variant is what compares. A
                // variant written out is its name (`o.kind !== "Equal"`).
                if t.as_ref().is_some_and(|t| self.is_ordering(t)) {
                    let kind = |e| match e {
                        Expr::Construct { variant: Some(v), fields: Fields::Unit, base: None, .. } => {
                            Expr::Lit(Lit::Str(v.as_str().to_string()))
                        }
                        e => Expr::Field { base: Box::new(e), name: Name::new("kind") },
                    };
                    return (rebuild(op, kind(l), kind(r)), self.expect(want, Some(Ty::bool())));
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
                let amount_hint = is_bare_int(right).then_some(Ty::Prim(Prim::I32));
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

    /// Equality on `t` is the derived one, part by part, and each part's
    /// JS value compares as Rust's `==` does: numbers, `bool`, strings,
    /// `char`, `Uuid`, `()`, and the crate's types and std's containers of
    /// them. `ParseIntError` and `uuid::Error` are opaque here, while Rust
    /// compares their kinds; `seen` stops a recursive type.
    pub(super) fn deep_comparable(&self, t: &Ty, seen: &mut Vec<Name>) -> bool {
        match self.norm(t) {
            _ if self.num(t).is_some() => true,
            Ty::Prim(Prim::ParseIntError | Prim::UuidError) => false,
            Ty::Prim(_) => true,
            Ty::Option(x) | Ty::Vec(x) => self.deep_comparable(&x, seen),
            Ty::Result { ok, err } => self.deep_comparable(&ok, seen) && self.deep_comparable(&err, seen),
            Ty::Tuple(xs) => xs.iter().all(|x| self.deep_comparable(x, seen)),
            Ty::Named(n) if seen.contains(&n) => true,
            Ty::Named(n) => {
                seen.push(n.clone());
                let fields: Vec<Ty> = if let Some(s) = self.defs.structs.get(n.as_str()) {
                    s.fields.iter().map(|f| f.ty.clone()).collect()
                } else if let Some(e) = self.defs.enums.get(n.as_str()) {
                    e.variants
                        .iter()
                        .flat_map(|v| match &v.fields {
                            VariantFields::Unit => Vec::new(),
                            VariantFields::Tuple(ts) => ts.clone(),
                            VariantFields::Struct(fs) => fs.iter().map(|f| f.ty.clone()).collect(),
                        })
                        .collect()
                } else {
                    return false;
                };
                fields.iter().all(|f| self.deep_comparable(f, seen))
            }
            _ => false,
        }
    }

    /// `& | ^ << >>` and `!` on an integer of type `t`, as a `Callee::Int`
    /// call. `usize` is refused: Rust gives it 64 bits, which its `number`
    /// cannot hold, so `!x` or `x << 20` would have no TS value.
    pub(super) fn bit_call(&mut self, op: BinOp, t: Ty, args: Vec<Expr>) -> Typed {
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

    pub(super) fn unary(&mut self, op: UnOp, inner: &Expr, want: Option<&Ty>) -> Typed {
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
                    Some(Num::Int(it)) if it.is_signed() => {
                        Expr::Call { callee: Callee::Int { ty: it, op: IntOp::Neg }, args: vec![e] }
                    }
                    Some(Num::Float(ft)) => Expr::Call { callee: Callee::AsFloat(ft), args: vec![neg(e)] },
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
}

pub(super) fn int_op(op: BinOp) -> IntOp {
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

pub(super) fn rebuild(op: BinOp, left: Expr, right: Expr) -> Expr {
    Expr::Binary { op, left: Box::new(left), right: Box::new(right) }
}

pub(super) fn neg(e: Expr) -> Expr {
    Expr::Unary { op: UnOp::Neg, expr: Box::new(e) }
}

/// An integer literal without a suffix, possibly negated.
pub(super) fn is_bare_int(expr: &Expr) -> bool {
    match expr.unpositioned() {
        Expr::Lit(Lit::Int { ty: None, .. }) => true,
        Expr::Unary { expr, .. } => is_bare_int(expr),
        _ => false,
    }
}
