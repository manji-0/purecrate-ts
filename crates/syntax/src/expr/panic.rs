//! `panic!`, `unreachable!`, and `assert!`: a `Callee::Panic` with the
//! message Rust gives (design/01 §7.17).

use purecrate_ir::{Callee, Expr, Lit, Reason, UnOp};
use syn::{BinOp as SynBinOp, Expr as SynExpr, UnOp as SynUnOp};

use super::format::{lower_template, parse_format_args, FormatArgs};
use super::lower_expr;
use crate::item::{Cx, ParseError};

fn refuse(name: &str, message: impl Into<String>) -> ParseError {
    ParseError::new(Reason::Macro, message).detail(name)
}

fn panic_with(message: Expr) -> Expr {
    Expr::Call { callee: Callee::Panic, args: vec![message] }
}

/// `panic!()`, `panic!("..", args)`, `unreachable!()`, and
/// `unreachable!("..", args)`.
pub(super) fn lower_panic(cx: &Cx, mac: &syn::Macro, name: &str) -> Result<Expr, ParseError> {
    let (fixed, prefix) = match name {
        "panic" => ("explicit panic", ""),
        _ => ("internal error: entered unreachable code", "internal error: entered unreachable code: "),
    };
    if mac.tokens.is_empty() {
        return Ok(panic_with(Expr::Lit(Lit::Str(fixed.into()))));
    }
    let (template, args): FormatArgs = mac.parse_body_with(parse_format_args).map_err(|e| {
        refuse(name, format!("`{name}!` takes nothing, or a string literal and its arguments as `format!` does: {e}"))
    })?;
    Ok(panic_with(lower_template(cx, &template, args, prefix)?))
}

/// `assert!(c)` and `assert!(c, "..", args)`: `if !c { panic }`, so the
/// message is built only when `c` is false, as in Rust. With no message,
/// Rust's is `assertion failed: ` and `c` as rustc prints it.
pub(super) fn lower_assert(cx: &Cx, mac: &syn::Macro) -> Result<Expr, ParseError> {
    let (cond, message) = mac
        .parse_body_with(|input: syn::parse::ParseStream| {
            let cond: SynExpr = input.parse()?;
            if input.is_empty() {
                return Ok((cond, None));
            }
            input.parse::<syn::Token![,]>()?;
            if input.is_empty() {
                return Ok((cond, None));
            }
            Ok((cond, Some(parse_format_args(input)?)))
        })
        .map_err(|e| refuse("assert", format!("`assert!` takes a condition and an optional message: {e}")))?;
    let message = match message {
        Some((template, args)) => lower_template(cx, &template, args, "")?,
        None => match shown(&cond) {
            Some(text) => Expr::Lit(Lit::Str(format!("assertion failed: {text}"))),
            None => {
                return Err(refuse(
                    "assert",
                    "this condition is not one whose text `assert!` can reproduce in v0: give it a message, `assert!(c, \"..\")`",
                ))
            }
        },
    };
    Ok(Expr::If {
        cond: Box::new(Expr::Unary { op: UnOp::Not, expr: Box::new(lower_expr(cx, &cond)?) }),
        then: Box::new(panic_with(message)),
        else_: Box::new(Expr::Lit(Lit::Unit)),
    })
}

/// `e` as rustc prints it in `assert!`'s message, for names, literals,
/// operators, casts to a named type, calls, method calls, fields, indexing, references, and
/// parentheses; `None` for anything else.
fn shown(e: &SynExpr) -> Option<String> {
    let list = |args: &syn::punctuated::Punctuated<SynExpr, syn::Token![,]>| {
        args.iter().map(shown).collect::<Option<Vec<_>>>().map(|a| a.join(", "))
    };
    Some(match e {
        SynExpr::Lit(l) => match &l.lit {
            syn::Lit::Str(s) => s.token().to_string(),
            syn::Lit::Char(c) => c.token().to_string(),
            syn::Lit::Int(i) => i.token().to_string(),
            syn::Lit::Float(f) => f.token().to_string(),
            syn::Lit::Byte(b) => b.token().to_string(),
            syn::Lit::Bool(b) => b.value.to_string(),
            _ => return None,
        },
        SynExpr::Path(p) if p.qself.is_none() && p.path.leading_colon.is_none() => p
            .path
            .segments
            .iter()
            .map(|s| s.arguments.is_none().then(|| s.ident.to_string()))
            .collect::<Option<Vec<_>>>()?
            .join("::"),
        SynExpr::Binary(b) => format!("{} {} {}", shown(&b.left)?, bin_text(&b.op)?, shown(&b.right)?),
        SynExpr::Unary(u) => {
            let op = match u.op {
                SynUnOp::Not(_) => "!",
                SynUnOp::Neg(_) => "-",
                SynUnOp::Deref(_) => "*",
                _ => return None,
            };
            format!("{op}{}", shown(&u.expr)?)
        }
        SynExpr::MethodCall(m) if m.turbofish.is_none() => {
            format!("{}.{}({})", shown(&m.receiver)?, m.method, list(&m.args)?)
        }
        SynExpr::Call(c) => format!("{}({})", shown(&c.func)?, list(&c.args)?),
        SynExpr::Field(f) => match &f.member {
            syn::Member::Named(n) => format!("{}.{n}", shown(&f.base)?),
            syn::Member::Unnamed(i) => format!("{}.{}", shown(&f.base)?, i.index),
        },
        SynExpr::Index(i) => format!("{}[{}]", shown(&i.expr)?, shown(&i.index)?),
        SynExpr::Paren(p) => format!("({})", shown(&p.expr)?),
        SynExpr::Cast(c) => match &*c.ty {
            syn::Type::Path(t) if t.qself.is_none() => format!("{} as {}", shown(&c.expr)?, t.path.get_ident()?),
            _ => return None,
        },
        SynExpr::Reference(r) => {
            format!("&{}{}", if r.mutability.is_some() { "mut " } else { "" }, shown(&r.expr)?)
        }
        _ => return None,
    })
}

fn bin_text(op: &SynBinOp) -> Option<&'static str> {
    Some(match op {
        SynBinOp::Add(_) => "+",
        SynBinOp::Sub(_) => "-",
        SynBinOp::Mul(_) => "*",
        SynBinOp::Div(_) => "/",
        SynBinOp::Rem(_) => "%",
        SynBinOp::Eq(_) => "==",
        SynBinOp::Ne(_) => "!=",
        SynBinOp::Lt(_) => "<",
        SynBinOp::Le(_) => "<=",
        SynBinOp::Gt(_) => ">",
        SynBinOp::Ge(_) => ">=",
        SynBinOp::And(_) => "&&",
        SynBinOp::Or(_) => "||",
        SynBinOp::BitAnd(_) => "&",
        SynBinOp::BitOr(_) => "|",
        SynBinOp::BitXor(_) => "^",
        SynBinOp::Shl(_) => "<<",
        SynBinOp::Shr(_) => ">>",
        _ => return None,
    })
}
