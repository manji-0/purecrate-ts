use purecrate_ir::{
    Arm, BinOp, Callee, ClosureParam, Expr, Fields, FloatTy, IntTy, Lit, Name, Pattern, Reason, Ty, UnOp,
    VariantBind,
    NEWTYPE_FIELD,
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
            Member::Unnamed(i) if i.index == 0 => Ok(Expr::Field {
                base: Box::new(lower_expr(cx, &f.base)?),
                name: Name::new(NEWTYPE_FIELD),
            }),
            Member::Unnamed(_) => Err(ParseError::new(Reason::TupleField, "tuple field access is not in v0")),
        },
        SynExpr::Assign(a) => Ok(Expr::Assign {
            name: assign_target(&a.left)?,
            value: Box::new(lower_expr(cx, &a.right)?),
        }),
        SynExpr::Binary(b) if compound_op(b.op).is_some() => {
            let name = assign_target(&b.left)?;
            let op = compound_op(b.op).expect("checked by the guard");
            Ok(Expr::Assign {
                value: Box::new(Expr::Binary {
                    op: lower_bin(op)?,
                    left: Box::new(Expr::Var(name.clone())),
                    right: Box::new(lower_expr(cx, &b.right)?),
                }),
                name,
            })
        }
        SynExpr::Binary(b) => Ok(Expr::Binary {
            op: lower_bin(b.op)?,
            left: Box::new(lower_expr(cx, &b.left)?),
            right: Box::new(lower_expr(cx, &b.right)?),
        }),
        SynExpr::Unary(u) if matches!(u.op, SynUnOp::Deref(_)) => lower_expr(cx, &u.expr),
        SynExpr::Unary(u) => Ok(Expr::Unary {
            op: lower_un(u.op)?,
            expr: Box::new(lower_expr(cx, &u.expr)?),
        }),
        SynExpr::Try(t) => Ok(Expr::Try {
            expr: Box::new(lower_expr(cx, &t.expr)?),
            on: None,
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
                    return Err(ParseError::new(Reason::MatchGuard, "match guards are not in v0"));
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
        SynExpr::Macro(m) if m.mac.path.is_ident("unreachable") => Ok(Expr::Unreachable),
        SynExpr::Macro(m) => Err(ParseError::new(
            Reason::Macro,
            format!("macro `{}!` is not in v0", path_text(&m.mac.path)),
        )
        .detail(path_text(&m.mac.path))),
        SynExpr::MethodCall(m) if m.turbofish.is_some() => Err(ParseError::new(
            Reason::MethodCall,
            format!("method call `.{}::<..>()` is not in v0", m.method),
        )
        .detail(m.method.to_string())),
        SynExpr::MethodCall(m) => Ok(Expr::MethodCall {
            receiver: Box::new(lower_expr(cx, &m.receiver)?),
            name: Name::new(m.method.to_string()),
            args: m.args.iter().map(|a| lower_expr(cx, a)).collect::<Result<_, _>>()?,
        }),
        SynExpr::Closure(c) => lower_closure(cx, c),
        SynExpr::Loop(_) | SynExpr::While(_) | SynExpr::ForLoop(_) | SynExpr::Break(_) | SynExpr::Continue(_) => {
            Err(ParseError::new(Reason::Loop, format!("loops are not in v0: {}", snippet(expr))))
        }
        SynExpr::Reference(r) if r.mutability.is_some() => Err(ParseError::new(
            Reason::Borrow,
            format!("`&mut` borrows are not in v0: {}", snippet(expr)),
        )),
        SynExpr::Reference(r) => lower_expr(cx, &r.expr),
        SynExpr::Index(_) => Err(ParseError::new(Reason::Index, format!("indexing is not in v0: {}", snippet(expr)))),
        SynExpr::Range(_) => Err(ParseError::new(Reason::Range, format!("ranges are not in v0: {}", snippet(expr)))),
        SynExpr::Cast(_) => Err(ParseError::new(Reason::Cast, format!("`as` casts are not in v0: {}", snippet(expr)))),
        other => Err(ParseError::new(
            Reason::UnsupportedExpr,
            format!("unsupported expression {}", snippet(other)),
        )
        .detail(expr_kind(other))),
    }
}

pub fn lower_block(cx: &Cx, block: &syn::Block) -> Result<Expr, ParseError> {
    lower_block_node(cx, block).map_err(|e| e.or_at(block.span()))
}

enum Stmt {
    Let {
        name: Name,
        mutable: bool,
        ty: Option<Ty>,
        value: Expr,
    },
    Effect(Expr),
}

fn lower_block_node(cx: &Cx, block: &syn::Block) -> Result<Expr, ParseError> {
    let mut stmts = Vec::new();
    let mut tail: Option<Expr> = None;
    let last = block.stmts.len().saturating_sub(1);
    for (i, stmt) in block.stmts.iter().enumerate() {
        match stmt {
            syn::Stmt::Local(local) => stmts.push(lower_local(cx, local)?),
            syn::Stmt::Expr(e, None) if i == last => tail = Some(lower_expr(cx, e)?),
            // `return x;` ends the block with the same meaning as `return x`.
            syn::Stmt::Expr(e @ SynExpr::Return(_), Some(_)) if i == last => {
                tail = Some(lower_expr(cx, e)?)
            }
            syn::Stmt::Expr(e, _) => stmts.push(Stmt::Effect(lower_expr(cx, e)?)),
            syn::Stmt::Item(_) => return Err(ParseError::new(Reason::BlockItem, "items inside blocks are not in v0")),
            syn::Stmt::Macro(m) => {
                let name = path_text(&m.mac.path);
                return Err(ParseError::new(Reason::Macro, format!("macro `{name}!` is not in v0")).detail(name));
            }
        }
    }
    let acc = tail.unwrap_or(Expr::Lit(Lit::Unit));
    Ok(stmts.into_iter().rev().fold(acc, |then, stmt| match stmt {
        Stmt::Let {
            name,
            mutable,
            ty,
            value,
        } => Expr::Let {
            name,
            mutable,
            ty,
            value: Box::new(value),
            then: Box::new(then),
        },
        Stmt::Effect(first) => Expr::Seq {
            first: Box::new(first),
            then: Box::new(then),
        },
    }))
}

fn lower_local(cx: &Cx, local: &syn::Local) -> Result<Stmt, ParseError> {
    let (pat, ty) = match &local.pat {
        Pat::Type(t) => (&*t.pat, Some(lower_type(&t.ty)?)),
        other => (other, None),
    };
    let (name, mutable) = match pat {
        Pat::Ident(id) if id.by_ref.is_none() && id.subpat.is_none() => {
            (Name::new(id.ident.to_string()), id.mutability.is_some())
        }
        _ => return Err(ParseError::new(Reason::LetPattern, "only simple let bindings in v0")),
    };
    let init = local
        .init
        .as_ref()
        .ok_or_else(|| ParseError::new(Reason::LetPattern, "let without initializer"))?;
    if init.diverge.is_some() {
        return Err(ParseError::new(Reason::LetElse, "let-else is not in v0"));
    }
    Ok(Stmt::Let {
        name,
        mutable,
        ty,
        value: lower_expr(cx, &init.expr)?,
    })
}

fn assign_target(place: &SynExpr) -> Result<Name, ParseError> {
    match place {
        SynExpr::Path(p) if p.qself.is_none() => p
            .path
            .get_ident()
            .map(|id| Name::new(id.to_string()))
            .ok_or_else(|| ParseError::new(Reason::PlaceAssign, "only a local variable can be assigned in v0")),
        SynExpr::Field(_) => Err(ParseError::new(Reason::PlaceAssign, 
            "assigning to a field is not in v0; build a new struct with `..` or all fields",
        )),
        _ => Err(ParseError::new(Reason::PlaceAssign, "only a local variable can be assigned in v0")),
    }
}

/// `x op= e` as `x = x op e`.
fn compound_op(op: SynBinOp) -> Option<SynBinOp> {
    use syn::token;
    Some(match op {
        SynBinOp::AddAssign(_) => SynBinOp::Add(token::Plus::default()),
        SynBinOp::SubAssign(_) => SynBinOp::Sub(token::Minus::default()),
        SynBinOp::MulAssign(_) => SynBinOp::Mul(token::Star::default()),
        SynBinOp::DivAssign(_) => SynBinOp::Div(token::Slash::default()),
        SynBinOp::RemAssign(_) => SynBinOp::Rem(token::Percent::default()),
        _ => return None,
    })
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
            base: None,
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
        _ => Err(path_error(&segs, format!("unsupported path {}", segs.join("::")))),
    }
}

fn lower_closure(cx: &Cx, c: &syn::ExprClosure) -> Result<Expr, ParseError> {
    let modifier = if c.lifetimes.is_some() {
        Some("`for<..>`")
    } else if c.constness.is_some() {
        Some("`const`")
    } else if c.movability.is_some() {
        Some("`static`")
    } else if c.asyncness.is_some() {
        Some("`async`")
    } else {
        None
    };
    if let Some(what) = modifier {
        return Err(ParseError::new(Reason::Closure, format!("{what} closures are not in v0")).detail(what));
    }
    let params = c
        .inputs
        .iter()
        .map(|p| {
            let (pat, ty) = match p {
                Pat::Type(t) => (&*t.pat, Some(lower_type(&t.ty)?)),
                other => (other, None),
            };
            match pat {
                Pat::Ident(id) if id.by_ref.is_none() && id.mutability.is_none() && id.subpat.is_none() => {
                    Ok(ClosureParam {
                        name: Name::new(id.ident.to_string()),
                        ty,
                    })
                }
                Pat::Wild(_) => Ok(ClosureParam {
                    name: Name::new("_"),
                    ty,
                }),
                other => Err(ParseError::new(
                    Reason::ParamPattern,
                    format!("closure parameters are plain names in v0, found {}", snippet(other)),
                )
                .or_at(other.span())),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ret = match &c.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, t) => Some(lower_type(t)?),
    };
    Ok(Expr::Closure {
        params,
        ret,
        body: Box::new(lower_expr(cx, &c.body)?),
    })
}

fn lower_struct_expr(cx: &Cx, s: &syn::ExprStruct) -> Result<Expr, ParseError> {
    let segs: Vec<String> = s.path.segments.iter().map(|p| p.ident.to_string()).collect();
    if s.rest.is_some() && segs.len() != 1 {
        return Err(ParseError::new(
            Reason::StructUpdate,
            "functional record update syntax requires a struct",
        ));
    }
    let base = match &s.rest {
        Some(e) => Some(Box::new(lower_expr(cx, e)?)),
        None => None,
    };
    let fields = Fields::Named(
        s.fields
            .iter()
            .map(|f| {
                let name = match &f.member {
                    Member::Named(id) => Name::new(id.to_string()),
                    Member::Unnamed(_) => {
                        return Err(ParseError::new(Reason::PositionalFields, "positional struct fields unsupported"))
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
            base,
        }),
        [ty, var] if cx.is_enum(ty) => Ok(Expr::Construct {
            ty: Name::new(ty.clone()),
            variant: Some(Name::new(var.clone())),
            fields,
            base: None,
        }),
        _ => Err(path_error(&segs, format!(
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
            } else if segs.len() == 2 && cx.is_variant(&segs[0], &segs[1]) {
                Callee::Variant {
                    ty: Name::new(segs[0].clone()),
                    variant: Name::new(segs[1].clone()),
                }
            } else if segs.len() == 2 && (cx.is_enum(&segs[0]) || cx.is_struct(&segs[0])) {
                Callee::Method {
                    ty: Name::new(segs[0].clone()),
                    name: Name::new(segs[1].clone()),
                }
            } else if segs.len() == 1 {
                Callee::Fn(Name::new(segs[0].clone()))
            } else {
                return Err(path_error(&segs, format!(
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
                    base: None,
                });
            }
            Ok(Expr::Call { callee, args })
        }
        _ => Err(ParseError::new(Reason::ExternalPath, "only simple calls in v0")),
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
                                return Err(ParseError::new(Reason::PositionalFields, "unnamed fields in struct pattern"))
                            }
                        };
                        Ok((name, lower_pat(cx, &f.pat)?))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            path_variant_pat(cx, &s.path, bind)
        }
        Pat::Tuple(t) if t.elems.len() == 1 => lower_pat(cx, &t.elems[0]),
        other => Err(ParseError::new(
            Reason::UnsupportedPattern,
            format!("unsupported pattern {}", snippet(other)),
        )),
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
            return Err(ParseError::new(Reason::ArmPattern, format!(
                "match arms must name an enum variant, `Some`/`None` or `Ok`/`Err` in v0, found {}",
                describe_pat(&pattern)
            )))
        }
    };
    if let Some(bad) = inner
        .into_iter()
        .find(|p| !matches!(p, Pattern::Var(_) | Pattern::Wildcard))
    {
        return Err(ParseError::new(Reason::NestedPattern, format!(
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
                Err(ParseError::new(Reason::ExternalPath, format!("unknown variant {var}")).detail(var.to_string()))
            }
        }
        _ => Err(path_error(&segs, format!(
            "unsupported pattern path {}",
            segs.join("::")
        ))),
    }
}

/// Bare `Some`/`None`/`Ok`/`Err` always mean the prelude's, as in Rust.
fn prelude_pat(name: &str, bind: VariantBind) -> Result<Option<Pattern>, ParseError> {
    let one = |bind: VariantBind| match bind {
        VariantBind::Tuple(mut ps) if ps.len() == 1 => Ok(Box::new(ps.remove(0))),
        _ => Err(ParseError::new(Reason::UnsupportedPattern, format!("`{name}` takes exactly one field"))),
    };
    Ok(Some(match name {
        "Some" => Pattern::OptionSome(one(bind)?),
        "Ok" => Pattern::ResultOk(one(bind)?),
        "Err" => Pattern::ResultErr(one(bind)?),
        "None" if bind == VariantBind::Unit => Pattern::OptionNone,
        "None" => return Err(ParseError::new(Reason::UnsupportedPattern, "`None` has no fields")),
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
            return Err(ParseError::new(Reason::IfLetVariant, 
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
                    ParseError::new(Reason::LiteralSuffix, format!("integer suffix `{s}` is not in v0"))
                })?),
            };
            let value = i
                .base10_parse::<i128>()
                .map_err(|e| ParseError::new(Reason::UnsupportedLiteral, e.to_string()))?;
            Ok(Lit::Int { value, ty })
        }
        syn::Lit::Float(f) => match float_suffix(f.suffix()) {
            Some(ty) => Ok(Lit::Float {
                digits: f.base10_digits().to_string(),
                ty,
            }),
            None => Err(ParseError::new(Reason::LiteralSuffix, format!(
                "float suffix `{}` is not in v0",
                f.suffix()
            ))),
        },
        syn::Lit::Str(s) => Ok(Lit::Str(s.value())),
        _ => Err(ParseError::new(Reason::UnsupportedLiteral, "unsupported literal")),
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
        _ => return Err(ParseError::new(Reason::UnsupportedOperator, "unsupported binary operator")),
    })
}

fn lower_un(op: SynUnOp) -> Result<UnOp, ParseError> {
    match op {
        SynUnOp::Not(_) => Ok(UnOp::Not),
        SynUnOp::Neg(_) => Ok(UnOp::Neg),
        _ => Err(ParseError::new(Reason::UnsupportedOperator, "unsupported unary operator")),
    }
}

fn path_text(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// The `syn` variant name, e.g. `Async`, for tallying unsupported syntax.
fn expr_kind(expr: &SynExpr) -> String {
    let debug = format!("{expr:?}");
    debug
        .strip_prefix("Expr::")
        .unwrap_or(&debug)
        .split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or("")
        .to_string()
}

/// A path that names nothing crate-local. `Self` paths get their own reason:
/// they name a crate-local type, just not by its name.
fn path_error(segs: &[String], message: String) -> ParseError {
    let reason = if segs.first().is_some_and(|s| s == "Self") {
        Reason::SelfType
    } else {
        Reason::ExternalPath
    };
    ParseError::new(reason, message).detail(segs.join("::"))
}
