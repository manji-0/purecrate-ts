//! The typed crate must leave nothing for emit to guess. Inference fills in
//! literal widths, `?` kinds, and conversion sources, and rewrites numeric
//! operators into width-checked calls. Anything still missing would print as
//! a raw JS operator or a default, which is where TS stops matching Rust.
//! `types` reports the usual causes with better messages; this is the last
//! gate before emit.

use purecrate_ir::{BinOp, Callee, Crate, Expr, Item, Lit, Reason, UnOp};

use crate::Diagnostic;

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        if let Item::Fn(f) = item {
            if let Some(what) = missing(&f.body, false) {
                out.push(Diagnostic::at(
                    i,
                    Reason::NeedsAnnotation,
                    format!("the type of {what} is not known here; annotate the binding it comes from"),
                ));
            }
        }
    }
    out
}

/// The first untyped node, described. `float_call` is true when `expr` is the
/// argument of `Fround` / `AsFloat`, the one place raw arithmetic is typed.
fn missing(expr: &Expr, float_call: bool) -> Option<&'static str> {
    let own = match expr {
        Expr::Lit(Lit::Int { ty: None, .. }) => Some("an integer literal"),
        Expr::Lit(Lit::Float { ty: None, .. }) => Some("a float literal"),
        Expr::Try { on: None, .. } => Some("the operand of `?`"),
        Expr::Call { callee: Callee::IntFrom { from: None, .. }, .. } => Some("the argument of an integer `from`"),
        Expr::MethodCall { .. } => Some("a method receiver"),
        Expr::Binary { op: BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem, .. } if !float_call => {
            Some("an arithmetic operand")
        }
        Expr::Binary { op, .. } if op.is_bitwise() || op.is_shift() => Some("a bitwise operand"),
        Expr::Unary { op: UnOp::Neg, .. } if !float_call => Some("a negated operand"),
        _ => None,
    };
    if own.is_some() {
        return own;
    }
    let floats = matches!(expr, Expr::Call { callee: Callee::Fround | Callee::AsFloat(_), .. });
    expr.children().into_iter().find_map(|c| missing(c, floats))
}
