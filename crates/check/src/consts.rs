//! Compile-time values: `const` items and enum discriminants. rustc
//! evaluates them before the program runs, and rejects one that overflows,
//! so the TS gets the folded literal rather than the computation.
//!
//! The expressions are what a subset crate writes for flags and limits:
//! literals, other consts, `E::A as T`, unary `-` and `!`, and the
//! arithmetic, bitwise, and shift operators, evaluated with rustc's rules
//! (a checked result for arithmetic, a wrapped one for shifts).

use purecrate_ir::{BinOp, Enum, Expr, IntTy, Lit, Name, Prim, Ty, UnOp, VariantFields};

use crate::defs::Defs;

/// Deep enough for any real chain of consts; a cycle (which rustc also
/// rejects) stops here instead of recursing forever.
const MAX_DEPTH: usize = 64;

/// The value of a `const` of type `ty`, as a typed literal.
pub fn fold(defs: &Defs<'_>, ty: &Ty, value: &Expr) -> Result<Lit, String> {
    Folder { defs, depth: 0 }.value(ty, value)
}

/// Each variant's discriminant: explicit ones folded, the others one past
/// the previous (the first `0`), in the `repr` type (`isize` without one).
pub fn discriminants(defs: &Defs<'_>, e: &Enum) -> Result<Vec<(Name, i128)>, String> {
    Folder { defs, depth: 0 }.discriminants(e)
}

/// Every variant carries no fields, so `as` gives its discriminant.
pub fn is_fieldless(e: &Enum) -> bool {
    e.variants.iter().all(|v| matches!(v.fields, VariantFields::Unit))
}

/// `isize` is 64 bits on every target the output is compared against.
fn repr_of(e: &Enum) -> (IntTy, &'static str) {
    match e.repr {
        Some(t) => (t, t.as_str()),
        None => (IntTy::I64, "isize"),
    }
}

struct Folder<'d, 'a> {
    defs: &'d Defs<'a>,
    depth: usize,
}

impl Folder<'_, '_> {
    fn deeper(&self) -> Result<Self, String> {
        if self.depth >= MAX_DEPTH {
            return Err("consts refer to each other in a cycle".into());
        }
        Ok(Folder { defs: self.defs, depth: self.depth + 1 })
    }

    fn value(&self, ty: &Ty, expr: &Expr) -> Result<Lit, String> {
        match ty {
            Ty::Prim(p) => match p.int() {
                Some(it) => Ok(Lit::Int { value: self.int(it, expr)?, ty: Some(it), byte: false, hex: false }),
                None => match (p, expr) {
                    (_, Expr::Var(n)) => self.named(n, ty).and_then(|(t, v)| self.deeper()?.value(&t, v)),
                    (Prim::Bool, Expr::Lit(l @ Lit::Bool(_))) => Ok(l.clone()),
                    (Prim::Bool, Expr::Unary { op: UnOp::Not, expr }) => match self.value(ty, expr)? {
                        Lit::Bool(b) => Ok(Lit::Bool(!b)),
                        _ => unreachable!("a `bool` const folds to a `bool`"),
                    },
                    (Prim::Bool, Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right }) => {
                        let (Lit::Bool(l), Lit::Bool(r)) = (self.value(ty, left)?, self.value(ty, right)?) else {
                            unreachable!("a `bool` const folds to a `bool`")
                        };
                        Ok(Lit::Bool(if *op == BinOp::And { l && r } else { l || r }))
                    }
                    (Prim::Char, Expr::Lit(l @ Lit::Char(_))) | (Prim::Str, Expr::Lit(l @ Lit::Str(_))) => {
                        Ok(l.clone())
                    }
                    (_, Expr::Lit(Lit::Float { digits, ty: None | Some(_) })) if p.float().is_some() => {
                        Ok(Lit::Float { digits: digits.clone(), ty: p.float() })
                    }
                    (_, Expr::Unary { op: UnOp::Neg, expr }) if p.float().is_some() => match self.value(ty, expr)? {
                        Lit::Float { digits, ty } if !digits.starts_with('-') => {
                            Ok(Lit::Float { digits: format!("-{digits}"), ty })
                        }
                        Lit::Float { digits, ty } => Ok(Lit::Float { digits: digits[1..].to_string(), ty }),
                        _ => unreachable!("a float const folds to a float"),
                    },
                    _ => Err(format!("a `{}` const is a literal or another const in v0", crate::types::show(ty))),
                },
            },
            _ => Err(format!(
                "a const of type `{}` is not in v0: consts are integers, floats, `bool`, `char`, or `&str`",
                crate::types::show(ty)
            )),
        }
    }

    /// The const `n`, which must have type `want`.
    fn named(&self, n: &Name, want: &Ty) -> Result<(Ty, &Expr), String> {
        let c = self
            .defs
            .consts
            .get(n.as_str())
            .ok_or_else(|| format!("`{}` is not a const; a const expression reads only consts", n.as_str()))?;
        if &c.ty != want {
            return Err(format!(
                "`{}` is a `{}`, where a `{}` is expected",
                n.as_str(),
                crate::types::show(&c.ty),
                crate::types::show(want)
            ));
        }
        Ok((c.ty.clone(), &c.value))
    }

    fn int(&self, it: IntTy, expr: &Expr) -> Result<i128, String> {
        let (lo, hi) = it.bounds();
        let fits = |v: i128| -> Result<i128, String> {
            if (lo..=hi).contains(&v) {
                Ok(v)
            } else {
                Err(format!("{v} does not fit `{}`; rustc rejects the overflow", it.as_str()))
            }
        };
        match expr {
            Expr::Lit(Lit::Int { value, ty, .. }) => match ty {
                Some(t) if *t != it => Err(format!("`{value}{}` where a `{}` is expected", t.as_str(), it.as_str())),
                _ => fits(*value),
            },
            Expr::Var(n) => {
                let (_, v) = self.named(n, &Ty::Prim(Prim::from(it)))?;
                self.deeper()?.int(it, v)
            }
            Expr::Cast { expr, to } if *to == Ty::Prim(Prim::from(it)) => {
                let Expr::Construct { ty, variant: Some(variant), .. } = &**expr else {
                    return Err("`as` in a const reads an enum variant's discriminant, `E::A as T`".into());
                };
                let e = self
                    .defs
                    .enums
                    .get(ty.as_str())
                    .filter(|e| is_fieldless(e))
                    .ok_or_else(|| format!("`{}` is not an enum without fields", ty.as_str()))?;
                let table = self.deeper()?.discriminants(e)?;
                let (_, v) = table
                    .into_iter()
                    .find(|(n, _)| n == variant)
                    .ok_or_else(|| format!("`{}` has no variant `{}`", ty.as_str(), variant.as_str()))?;
                fits(v)
            }
            Expr::Unary { op: UnOp::Neg, expr } => {
                if !it.is_signed() {
                    return Err(format!("cannot negate a `{}`", it.as_str()));
                }
                fits(-self.int(it, expr)?)
            }
            Expr::Unary { op: UnOp::Not, expr } => {
                let v = self.int(it, expr)?;
                let (_, full) = if it == IntTy::Usize { (0, u64::MAX.into()) } else { it.bounds() };
                fits(if it.is_signed() { !v } else { full ^ v })
            }
            Expr::Binary { op, left, right } if op.is_shift() => {
                let v = self.int(it, left)?;
                let amount = self.shift_amount(right)?;
                let bits = it.bits();
                if !(0..i128::from(bits)).contains(&amount) {
                    return Err(format!("shift by {amount} overflows a `{}`; rustc rejects it", it.as_str()));
                }
                let shifted = if *op == BinOp::Shl { v << amount } else { v >> amount };
                fits(wrap(it, shifted))
            }
            Expr::Binary { op, left, right } => {
                let l = self.int(it, left)?;
                let r = self.int(it, right)?;
                match op {
                    BinOp::Add => fits(l + r),
                    BinOp::Sub => fits(l - r),
                    BinOp::Mul => fits(l.checked_mul(r).ok_or("overflow")?),
                    BinOp::Div | BinOp::Rem if r == 0 => Err("division by zero; rustc rejects it".into()),
                    BinOp::Div => fits(l / r),
                    BinOp::Rem => fits(l % r),
                    BinOp::BitAnd => fits(l & r),
                    BinOp::BitOr => fits(l | r),
                    BinOp::BitXor => fits(l ^ r),
                    _ => Err(format!("`{}` is not an integer operator", op_text(*op))),
                }
            }
            _ => Err("a const expression in v0 is literals, consts, `E::A as T`, and integer operators".into()),
        }
    }

    /// The right of `<<` / `>>` has its own integer type; an unsuffixed
    /// literal is `i32`, as rustc types it.
    fn shift_amount(&self, expr: &Expr) -> Result<i128, String> {
        match expr {
            Expr::Lit(Lit::Int { value, ty, .. }) => self.int(
                ty.unwrap_or(IntTy::I32),
                &Expr::Lit(Lit::Int { value: *value, ty: *ty, byte: false, hex: false }),
            ),
            Expr::Var(n) => {
                let c = self.defs.consts.get(n.as_str()).ok_or_else(|| format!("`{}` is not a const", n.as_str()))?;
                let it = match &c.ty {
                    Ty::Prim(p) => p.int(),
                    _ => None,
                }
                .ok_or_else(|| format!("a shift amount is an integer, `{}` is not", n.as_str()))?;
                self.deeper()?.int(it, &c.value)
            }
            other => self.int(IntTy::I32, other),
        }
    }

    fn discriminants(&self, e: &Enum) -> Result<Vec<(Name, i128)>, String> {
        let (it, label) = repr_of(e);
        let (lo, hi) = it.bounds();
        let mut out = Vec::new();
        let mut next: Option<i128> = Some(0);
        for v in &e.variants {
            let value = match &v.discriminant {
                Some(expr) => {
                    self.int(it, expr).map_err(|m| format!("`{}::{}`: {m}", e.name.as_str(), v.name.as_str()))?
                }
                None => next.ok_or_else(|| {
                    format!(
                        "`{}::{}` would follow the largest `{label}`; rustc rejects it",
                        e.name.as_str(),
                        v.name.as_str()
                    )
                })?,
            };
            if !(lo..=hi).contains(&value) {
                return Err(format!("`{}::{}` = {value} does not fit `{label}`", e.name.as_str(), v.name.as_str()));
            }
            if let Some((other, _)) = out.iter().find(|(_, d)| *d == value) {
                return Err(format!(
                    "`{}::{}` and `{}::{}` both have discriminant {value}; rustc rejects it",
                    e.name.as_str(),
                    Name::as_str(other),
                    e.name.as_str(),
                    v.name.as_str()
                ));
            }
            next = (value < hi).then_some(value + 1);
            out.push((v.name.clone(), value));
        }
        Ok(out)
    }
}

/// Keeps the low `bits` of `v`, read as `it` (two's complement if signed).
fn wrap(it: IntTy, v: i128) -> i128 {
    let bits = it.bits();
    let masked = v & ((1i128 << bits) - 1);
    if it.is_signed() && masked >= 1i128 << (bits - 1) {
        masked - (1i128 << bits)
    } else {
        masked
    }
}

fn op_text(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        _ => "?",
    }
}
