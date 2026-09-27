use purecrate_ir::{
    Arm, BinOp, Callee, Expr, Fields, FloatTy, IntTy, Lit, Name, Pattern, Ty, UnOp, VariantBind,
};
use syn::spanned::Spanned;
use syn::{BinOp as SynBinOp, Expr as SynExpr, Member, Pat, UnOp as SynUnOp};

use crate::item::{snippet, Cx, ParseError};
use crate::ty::lower_type;

pub fn lower_expr(cx: &Cx, expr: &SynExpr) -> Result<Expr, ParseError> {
    lower_expr_node(cx, expr).map_err(|e| e.or_at(expr.span()))
}

fn lower_expr_node(cx: &Cx, expr: &SynExpr) -> Result<Expr, ParseError> {
    match expr {
        SynExpr::Lit(l) => Ok(Expr::Lit(lower_lit(&l.lit)?)),
        SynExpr::Path(p) => lower_path_expr(cx, &p.path),
        SynExpr::Field(f) => match &f.member {
            Member::Named(id) => Ok(Expr::Field {
                base: Box::new(lower_expr(cx, &f.base)?),
                name: Name::new(id.to_string()),
            }),
            Member::Unnamed(_) => Err(ParseError::new("tuple field access is not in v0")),
        },
        SynExpr::Binary(b) => Ok(Expr::Binary {
            op: lower_bin(b.op)?,
            left: Box::new(lower_expr(cx, &b.left)?),
            right: Box::new(lower_expr(cx, &b.right)?),
        }),
        SynExpr::Unary(u) => Ok(Expr::Unary {
            op: lower_un(u.op)?,
            expr: Box::new(lower_expr(cx, &u.expr)?),
        }),
        SynExpr::Paren(p) => lower_expr(cx, &p.expr),
        SynExpr::Group(g) => lower_expr(cx, &g.expr),
        SynExpr::Block(b) => lower_block(cx, &b.block),
        SynExpr::If(i) => {
            let else_ = match &i.else_branch {
                Some((_, e)) => lower_expr(cx, e)?,
                None => Expr::Lit(Lit::Unit),
            };
            if let SynExpr::Let(l) = &*i.cond {
                return lower_if_let(cx, l, lower_block(cx, &i.then_branch)?, else_);
            }
            Ok(Expr::If {
                cond: Box::new(lower_expr(cx, &i.cond)?),
                then: Box::new(lower_block(cx, &i.then_branch)?),
                else_: Box::new(else_),
            })
        }
        SynExpr::Match(m) => {
            let mut arms = Vec::new();
            for arm in &m.arms {
                if arm.guard.is_some() {
                    return Err(ParseError::new("match guards are not in v0"));
                }
                arms.push(Arm {
                    pattern: arm_pattern(lower_pat(cx, &arm.pat)?)
                        .map_err(|e| e.or_at(arm.pat.span()))?,
                    body: lower_expr(cx, &arm.body)?,
                });
            }
            Ok(Expr::Match {
                scrutinee: Box::new(lower_expr(cx, &m.expr)?),
                arms,
            })
        }
        SynExpr::Struct(s) => lower_struct_expr(cx, s),
        SynExpr::Call(c) => lower_call(cx, &c.func, c.args.iter().collect()),
        SynExpr::Tuple(t) => {
            let elems = t
                .elems
                .iter()
                .map(|e| lower_expr(cx, e))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Expr::Tuple(elems))
        }
        SynExpr::Array(a) => {
            let elems = a
                .elems
                .iter()
                .map(|e| lower_expr(cx, e))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Expr::Array(elems))
        }
        SynExpr::Return(r) => {
            let inner = match &r.expr {
                Some(e) => lower_expr(cx, e)?,
                None => Expr::Lit(Lit::Unit),
            };
            Ok(Expr::Return(Box::new(inner)))
        }
        SynExpr::Macro(m) if m.mac.path.is_ident("vec") => {
            Err(ParseError::new("vec! is parsed in v1"))
        }
        SynExpr::Macro(m) if m.mac.path.is_ident("unreachable") => Ok(Expr::Unreachable),
        other => Err(ParseError::new(format!("unsupported expression {}", snippet(other)))),
    }
}

pub fn lower_block(cx: &Cx, block: &syn::Block) -> Result<Expr, ParseError> {
    lower_block_node(cx, block).map_err(|e| e.or_at(block.span()))
}

fn lower_block_node(cx: &Cx, block: &syn::Block) -> Result<Expr, ParseError> {
    let mut lets: Vec<(Name, Option<Ty>, Expr)> = Vec::new();
    let mut tail: Option<Expr> = None;
    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Local(local) => {
                let (pat, ty) = match &local.pat {
                    Pat::Type(t) => (&*t.pat, Some(lower_type(&t.ty)?)),
                    other => (other, None),
                };
                let name = match pat {
                    Pat::Ident(id) if id.by_ref.is_none() && id.mutability.is_none() && id.subpat.is_none() => {
                        Name::new(id.ident.to_string())
                    }
                    _ => return Err(ParseError::new("only simple let bindings in v0")),
                };
                let init = local
                    .init
                    .as_ref()
                    .ok_or_else(|| ParseError::new("let without initializer"))?;
                if init.diverge.is_some() {
                    return Err(ParseError::new("let-else is not in v0"));
                }
                lets.push((name, ty, lower_expr(cx, &init.expr)?));
            }
            syn::Stmt::Expr(e, _) => {
                if tail.is_some() {
                    return Err(ParseError::new("multiple tail expressions in a block"));
                }
                tail = Some(lower_expr(cx, e)?);
            }
            syn::Stmt::Item(_) => return Err(ParseError::new("items inside blocks are not in v0")),
            syn::Stmt::Macro(_) => return Err(ParseError::new("macros in blocks are not in v0")),
        }
    }
    let mut acc = tail.unwrap_or(Expr::Lit(Lit::Unit));
    for (name, ty, value) in lets.into_iter().rev() {
        acc = Expr::Let {
            name,
            ty,
            value: Box::new(value),
            then: Box::new(acc),
        };
    }
    Ok(acc)
}

fn lower_path_expr(cx: &Cx, path: &syn::Path) -> Result<Expr, ParseError> {
    let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    match segs.as_slice() {
        [one] => match one.as_str() {
            "true" => Ok(Expr::Lit(Lit::Bool(true))),
            "false" => Ok(Expr::Lit(Lit::Bool(false))),
            "None" => Ok(Expr::Call {
                callee: Callee::OptionNone,
                args: vec![],
            }),
            _ => Ok(Expr::var(one.clone())),
        },
        [ty, var] if cx.is_enum(ty) => Ok(Expr::Construct {
            ty: Name::new(ty.clone()),
            variant: Some(Name::new(var.clone())),
            fields: Fields::Unit,
        }),
        [ty, var] if ty == "Result" && (var == "ok" || var == "Ok") => Ok(Expr::Call {
            callee: Callee::ResultOk,
            args: vec![],
        }),
        [ty, var] if ty == "Result" && (var == "err" || var == "Err") => Ok(Expr::Call {
            callee: Callee::ResultErr,
            args: vec![],
        }),
        [a, b] => Ok(Expr::Call {
            callee: Callee::Fn(Name::new(format!("{a}::{b}"))),
            args: vec![],
        }),
        _ => Err(ParseError::new(format!("unsupported path {}", segs.join("::")))),
    }
}

fn lower_struct_expr(cx: &Cx, s: &syn::ExprStruct) -> Result<Expr, ParseError> {
    if s.rest.is_some() {
        return Err(ParseError::new("struct update syntax `..` is v1"));
    }
    let segs: Vec<String> = s.path.segments.iter().map(|p| p.ident.to_string()).collect();
    let fields = Fields::Named(
        s.fields
            .iter()
            .map(|f| {
                let name = match &f.member {
                    Member::Named(id) => Name::new(id.to_string()),
                    Member::Unnamed(_) => {
                        return Err(ParseError::new("positional struct fields unsupported"))
                    }
                };
                Ok((name, lower_expr(cx, &f.expr)?))
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    match segs.as_slice() {
        [ty] if cx.is_struct(ty) => Ok(Expr::Construct {
            ty: Name::new(ty.clone()),
            variant: None,
            fields,
        }),
        [ty, var] if cx.is_enum(ty) => Ok(Expr::Construct {
            ty: Name::new(ty.clone()),
            variant: Some(Name::new(var.clone())),
            fields,
        }),
        _ => Err(ParseError::new(format!(
            "unknown struct constructor {}",
            segs.join("::")
        ))),
    }
}

fn lower_call(cx: &Cx, func: &SynExpr, args: Vec<&SynExpr>) -> Result<Expr, ParseError> {
    let args = args
        .into_iter()
        .map(|a| lower_expr(cx, a))
        .collect::<Result<Vec<_>, _>>()?;
    match func {
        SynExpr::Path(p) => {
            let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
            let callee = if segs == ["Some"] {
                Callee::OptionSome
            } else if segs == ["None"] {
                Callee::OptionNone
            } else if segs == ["Ok"] {
                Callee::ResultOk
            } else if segs == ["Err"] {
                Callee::ResultErr
            } else if segs.len() == 1 && cx.is_struct(&segs[0]) {
                Callee::StructNew(Name::new(segs[0].clone()))
            } else if segs.len() == 2 && cx.is_enum(&segs[0]) {
                Callee::Variant {
                    ty: Name::new(segs[0].clone()),
                    variant: Name::new(segs[1].clone()),
                }
            } else if segs.len() == 1 {
                Callee::Fn(Name::new(segs[0].clone()))
            } else {
                return Err(ParseError::new(format!(
                    "unsupported call {}",
                    segs.join("::")
                )));
            };
            if matches!(callee, Callee::Variant { .. }) {
                return Ok(Expr::Construct {
                    ty: match &callee {
                        Callee::Variant { ty, .. } => ty.clone(),
                        _ => unreachable!(),
                    },
                    variant: match &callee {
                        Callee::Variant { variant, .. } => Some(variant.clone()),
                        _ => None,
                    },
                    fields: Fields::Positional(args),
                });
            }
            Ok(Expr::Call { callee, args })
        }
        _ => Err(ParseError::new("only simple calls in v0")),
    }
}

fn lower_pat(cx: &Cx, pat: &Pat) -> Result<Pattern, ParseError> {
    lower_pat_node(cx, pat).map_err(|e| e.or_at(pat.span()))
}

fn lower_pat_node(cx: &Cx, pat: &Pat) -> Result<Pattern, ParseError> {
    match pat {
        Pat::Wild(_) => Ok(Pattern::Wildcard),
        Pat::Ident(id) if id.ident == "None" && id.subpat.is_none() => Ok(Pattern::OptionNone),
        Pat::Ident(id) if id.by_ref.is_none() && id.mutability.is_none() && id.subpat.is_none() => {
            Ok(Pattern::Var(Name::new(id.ident.to_string())))
        }
        Pat::Lit(l) => Ok(Pattern::Lit(lower_lit(&l.lit)?)),
        Pat::Path(p) => path_variant_pat(cx, &p.path, VariantBind::Unit),
        Pat::TupleStruct(t) => {
            let bind = VariantBind::Tuple(
                t.elems
                    .iter()
                    .map(|e| lower_pat(cx, e))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            path_variant_pat(cx, &t.path, bind)
        }
        Pat::Struct(s) => {
            let bind = VariantBind::Struct(
                s.fields
                    .iter()
                    .map(|f| {
                        let name = match &f.member {
                            Member::Named(id) => Name::new(id.to_string()),
                            Member::Unnamed(_) => {
                                return Err(ParseError::new("unnamed fields in struct pattern"))
                            }
                        };
                        Ok((name, lower_pat(cx, &f.pat)?))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            path_variant_pat(cx, &s.path, bind)
        }
        Pat::Tuple(t) if t.elems.len() == 1 => lower_pat(cx, &t.elems[0]),
        other => Err(ParseError::new(format!("unsupported pattern {}", snippet(other)))),
    }
}

/// v0 prints `match` as `switch (x.kind)`: each arm names one variant and
/// binds its fields to plain names.
fn arm_pattern(pattern: Pattern) -> Result<Pattern, ParseError> {
    let inner: Vec<&Pattern> = match &pattern {
        Pattern::Variant { bind, .. } => match bind {
            VariantBind::Unit => Vec::new(),
            VariantBind::Tuple(pats) => pats.iter().collect(),
            VariantBind::Struct(pairs) => pairs.iter().map(|(_, p)| p).collect(),
        },
        Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => vec![&**p],
        Pattern::OptionNone => Vec::new(),
        Pattern::Wildcard | Pattern::Var(_) | Pattern::Lit(_) => {
            return Err(ParseError::new(format!(
                "match arms must name an enum variant, `Some`/`None` or `Ok`/`Err` in v0, found {}",
                describe_pat(&pattern)
            )))
        }
    };
    if let Some(bad) = inner
        .into_iter()
        .find(|p| !matches!(p, Pattern::Var(_) | Pattern::Wildcard))
    {
        return Err(ParseError::new(format!(
            "variant fields may only bind names or `_` in v0, found {}",
            describe_pat(bad)
        )));
    }
    Ok(pattern)
}

fn describe_pat(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "`_`".into(),
        Pattern::Var(n) => format!("binding `{}`", n.as_str()),
        Pattern::Lit(_) => "a literal".into(),
        Pattern::Variant { ty, variant, .. } => {
            format!("nested variant `{}::{}`", ty.as_str(), variant.as_str())
        }
        Pattern::OptionSome(_) => "nested `Some(..)`".into(),
        Pattern::OptionNone => "nested `None`".into(),
        Pattern::ResultOk(_) => "nested `Ok(..)`".into(),
        Pattern::ResultErr(_) => "nested `Err(..)`".into(),
    }
}

fn path_variant_pat(cx: &Cx, path: &syn::Path, bind: VariantBind) -> Result<Pattern, ParseError> {
    let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    if let [one] = segs.as_slice() {
        if let Some(p) = prelude_pat(one, bind.clone())? {
            return Ok(p);
        }
    }
    match segs.as_slice() {
        [ty, var] if cx.is_enum(ty) => Ok(Pattern::Variant {
            ty: Name::new(ty.clone()),
            variant: Name::new(var.clone()),
            bind,
        }),
        [var] => {
            if let Some(ty) = cx.enum_for_variant(var) {
                Ok(Pattern::Variant {
                    ty: Name::new(ty),
                    variant: Name::new(var.clone()),
                    bind,
                })
            } else {
                Err(ParseError::new(format!("unknown variant {var}")))
            }
        }
        _ => Err(ParseError::new(format!(
            "unsupported pattern path {}",
            segs.join("::")
        ))),
    }
}

/// Bare `Some`/`None`/`Ok`/`Err` always mean the prelude's, as in Rust.
fn prelude_pat(name: &str, bind: VariantBind) -> Result<Option<Pattern>, ParseError> {
    let one = |bind: VariantBind| match bind {
        VariantBind::Tuple(mut ps) if ps.len() == 1 => Ok(Box::new(ps.remove(0))),
        _ => Err(ParseError::new(format!("`{name}` takes exactly one field"))),
    };
    Ok(Some(match name {
        "Some" => Pattern::OptionSome(one(bind)?),
        "Ok" => Pattern::ResultOk(one(bind)?),
        "Err" => Pattern::ResultErr(one(bind)?),
        "None" if bind == VariantBind::Unit => Pattern::OptionNone,
        "None" => return Err(ParseError::new("`None` has no fields")),
        _ => return Ok(None),
    }))
}

/// `if let P = e { a } else { b }` is `match e { P => a, <the other case> => b }`.
fn lower_if_let(cx: &Cx, l: &syn::ExprLet, then: Expr, else_: Expr) -> Result<Expr, ParseError> {
    let pattern = arm_pattern(lower_pat(cx, &l.pat)?).map_err(|e| e.or_at(l.pat.span()))?;
    let other = match &pattern {
        Pattern::OptionSome(_) => Pattern::OptionNone,
        Pattern::OptionNone => Pattern::OptionSome(Box::new(Pattern::Wildcard)),
        Pattern::ResultOk(_) => Pattern::ResultErr(Box::new(Pattern::Wildcard)),
        Pattern::ResultErr(_) => Pattern::ResultOk(Box::new(Pattern::Wildcard)),
        _ => {
            return Err(ParseError::new(
                "`if let` on an enum variant is not in v0; use `match` with every variant",
            )
            .or_at(l.pat.span()))
        }
    };
    Ok(Expr::Match {
        scrutinee: Box::new(lower_expr(cx, &l.expr)?),
        arms: vec![
            Arm {
                pattern,
                body: then,
            },
            Arm {
                pattern: other,
                body: else_,
            },
        ],
    })
}

fn lower_lit(lit: &syn::Lit) -> Result<Lit, ParseError> {
    match lit {
        syn::Lit::Bool(b) => Ok(Lit::Bool(b.value)),
        syn::Lit::Int(i) => {
            let suffix = i.suffix();
            if let Some(Some(ty)) = float_suffix(suffix) {
                return Ok(Lit::Float {
                    digits: i.base10_digits().to_string(),
                    ty: Some(ty),
                });
            }
            let ty = match suffix {
                "" => None,
                s => Some(IntTy::of_suffix(s).ok_or_else(|| {
                    ParseError::new(format!("integer suffix `{s}` is not in v0"))
                })?),
            };
            let value = i
                .base10_parse::<i128>()
                .map_err(|e| ParseError::new(e.to_string()))?;
            Ok(Lit::Int { value, ty })
        }
        syn::Lit::Float(f) => match float_suffix(f.suffix()) {
            Some(ty) => Ok(Lit::Float {
                digits: f.base10_digits().to_string(),
                ty,
            }),
            None => Err(ParseError::new(format!(
                "float suffix `{}` is not in v0",
                f.suffix()
            ))),
        },
        syn::Lit::Str(s) => Ok(Lit::Str(s.value())),
        _ => Err(ParseError::new("unsupported literal")),
    }
}

/// `Some(None)` for no suffix, `None` for a suffix that is not a float type.
fn float_suffix(suffix: &str) -> Option<Option<FloatTy>> {
    match suffix {
        "" => Some(None),
        "f32" => Some(Some(FloatTy::F32)),
        "f64" => Some(Some(FloatTy::F64)),
        _ => None,
    }
}

fn lower_bin(op: SynBinOp) -> Result<BinOp, ParseError> {
    Ok(match op {
        SynBinOp::Add(_) => BinOp::Add,
        SynBinOp::Sub(_) => BinOp::Sub,
        SynBinOp::Mul(_) => BinOp::Mul,
        SynBinOp::Div(_) => BinOp::Div,
        SynBinOp::Rem(_) => BinOp::Rem,
        SynBinOp::Eq(_) => BinOp::Eq,
        SynBinOp::Ne(_) => BinOp::Ne,
        SynBinOp::Lt(_) => BinOp::Lt,
        SynBinOp::Le(_) => BinOp::Le,
        SynBinOp::Gt(_) => BinOp::Gt,
        SynBinOp::Ge(_) => BinOp::Ge,
        SynBinOp::And(_) => BinOp::And,
        SynBinOp::Or(_) => BinOp::Or,
        _ => return Err(ParseError::new("unsupported binary operator")),
    })
}

fn lower_un(op: SynUnOp) -> Result<UnOp, ParseError> {
    match op {
        SynUnOp::Not(_) => Ok(UnOp::Not),
        SynUnOp::Neg(_) => Ok(UnOp::Neg),
        _ => Err(ParseError::new("unsupported unary operator")),
    }
}
