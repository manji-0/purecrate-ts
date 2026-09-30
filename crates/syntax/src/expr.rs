use purecrate_ir::{
    Arm, BinOp, Callee, ClosureParam, Expr, Fields, FloatTy, IntTy, Lit, Name, Over, Pattern, Pos, Reason, Ty, UnOp,
    VariantBind, Wrapper,
    NEWTYPE_FIELD,
};
use syn::spanned::Spanned;
use syn::{BinOp as SynBinOp, Expr as SynExpr, Member, Pat, UnOp as SynUnOp};

use crate::item::{snippet, Cx, LineCol, ParseError};
use crate::ty::lower_type;

pub fn lower_expr(cx: &Cx, expr: &SynExpr) -> Result<Expr, ParseError> {
    match lower_expr_node(cx, expr).map_err(|e| e.or_at(expr.span())) {
        Err(e) if cx.recover(&e) => Ok(Expr::Unreachable),
        other => other,
    }
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
        SynExpr::Match(m) if m.arms.iter().any(|a| a.guard.is_some()) => lower_guarded(cx, &m.expr, &m.arms),
        SynExpr::Match(m) => {
            let mut arms = Vec::new();
            for arm in &m.arms {
                arms.push(Arm {
                    pattern: arm_pattern(lower_pat(cx, &arm.pat)?)
                        .map_err(|e| e.or_at(arm.pat.span()))?,
                    body: at(arm.body.span(), lower_expr(cx, &arm.body)?),
                });
            }
            Ok(Expr::Match {
                scrutinee: Box::new(lower_expr(cx, &m.expr)?),
                arms,
            })
        }
        SynExpr::Struct(s) => lower_struct_expr(cx, s),
        // Calls carry their own position, so a diagnostic about one points
        // at it rather than at the statement around it.
        SynExpr::Call(c) => Ok(at(c.span(), lower_call(cx, &c.func, c.args.iter().collect())?)),
        // `()` is the unit value, not an empty tuple: its type is `()` and
        // it prints as `undefined`.
        SynExpr::Tuple(t) if t.elems.is_empty() => Ok(Expr::Lit(Lit::Unit)),
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
        SynExpr::Macro(m) if m.mac.path.is_ident("matches") => lower_matches(cx, &m.mac),
        SynExpr::Macro(m) if m.mac.path.is_ident("vec") => lower_vec(cx, &m.mac),
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
        SynExpr::MethodCall(m) => Ok(at(
            m.method.span(),
            Expr::MethodCall {
                receiver: Box::new(lower_expr(cx, &m.receiver)?),
                name: Name::new(m.method.to_string()),
                args: m.args.iter().map(|a| lower_expr(cx, a)).collect::<Result<_, _>>()?,
            },
        )),
        SynExpr::Closure(c) => lower_closure(cx, c),
        SynExpr::ForLoop(f) => lower_for(cx, f),
        SynExpr::While(w) if w.label.is_some() => {
            Err(ParseError::new(Reason::Loop, format!("loop labels are not in v0: {}", snippet(expr))))
        }
        SynExpr::While(w) if matches!(&*w.cond, SynExpr::Let(_)) => Err(ParseError::new(
            Reason::Loop,
            format!("`while let` is not in v0; use `while` with a `match` inside, or `for`: {}", snippet(expr)),
        )),
        SynExpr::While(w) => Ok(Expr::While {
            cond: Box::new(lower_expr(cx, &w.cond)?),
            body: Box::new(lower_block(cx, &w.body)?),
        }),
        SynExpr::Break(b) if b.label.is_none() && b.expr.is_none() => Ok(Expr::Break),
        SynExpr::Continue(c) if c.label.is_none() => Ok(Expr::Continue),
        SynExpr::Break(_) | SynExpr::Continue(_) => Err(ParseError::new(
            Reason::Loop,
            format!("`break` and `continue` take no label or value in v0: {}", snippet(expr)),
        )),
        SynExpr::Loop(_) => Err(ParseError::new(
            Reason::Loop,
            format!("`loop` is not in v0; write `while` with its condition: {}", snippet(expr)),
        )),
        SynExpr::Reference(r) if r.mutability.is_some() => Err(ParseError::new(
            Reason::Borrow,
            format!("`&mut` borrows are not in v0: {}", snippet(expr)),
        )),
        SynExpr::Reference(r) => lower_expr(cx, &r.expr),
        SynExpr::Index(i) => Ok(Expr::Index {
            base: Box::new(lower_expr(cx, &i.expr)?),
            index: Box::new(lower_expr(cx, &i.index)?),
        }),
        SynExpr::Range(_) => Err(ParseError::new(Reason::Range, format!("ranges are not in v0: {}", snippet(expr)))),
        SynExpr::Cast(c) => Ok(Expr::Cast {
            expr: Box::new(lower_expr(cx, &c.expr)?),
            to: lower_type(&c.ty).map_err(|_| {
                ParseError::new(Reason::Cast, format!("`as` casts are not in v0: {}", snippet(expr)))
            })?,
        }),
        other => Err(ParseError::new(
            Reason::UnsupportedExpr,
            format!("unsupported expression {}", snippet(other)),
        )
        .detail(expr_kind(other))),
    }
}

/// `expr`, marked with where its source starts, for diagnostics.
fn at(span: proc_macro2::Span, expr: Expr) -> Expr {
    let LineCol { line, col } = LineCol::of(span);
    Expr::At {
        at: Pos {
            line: line as u32,
            col: col as u32,
        },
        expr: Box::new(expr),
    }
}

/// Iterator adaptors, named so that `for` over one says so rather than
/// failing later on the method call.
const ITERATOR_ADAPTORS: &[&str] = &[
    "enumerate", "rev", "zip", "map", "filter", "filter_map", "skip", "take", "step_by", "windows", "chunks",
    "cloned", "copied", "peekable", "char_indices", "split_whitespace", "splitn", "rsplit", "lines", "keys", "values",
];

/// `for` over a half-open integer range, a string's `chars()`, `bytes()`,
/// or `split(c)` on a `char`, or a `Vec` or slice (`xs`, `&xs`, `xs.iter()`), unlabelled,
/// with a plain name for the variable. Whether `xs` is a `Vec` is checked
/// with the types. Iterator adaptors (`enumerate`, `rev`, `zip`, ...) and
/// `a..=b` stay out.
fn lower_for(cx: &Cx, f: &syn::ExprForLoop) -> Result<Expr, ParseError> {
    let reject = |what: &str| Err(ParseError::new(Reason::Loop, format!("{what}: {}", snippet(&SynExpr::ForLoop(f.clone())))));
    if f.label.is_some() {
        return reject("loop labels are not in v0");
    }
    let var = match &*f.pat {
        Pat::Ident(p) if p.by_ref.is_none() && p.mutability.is_none() && p.subpat.is_none() => Name::new(p.ident.to_string()),
        _ => return reject("`for` takes a plain name for its variable, not `mut`, `_` or a pattern"),
    };
    let each = |over: Over, source: &SynExpr| -> Result<Expr, ParseError> {
        Ok(Expr::ForEach {
            var: var.clone(),
            over,
            source: Box::new(lower_expr(cx, source)?),
            body: Box::new(lower_block(cx, &f.body)?),
        })
    };
    let (start, end) = match &*f.expr {
        SynExpr::Range(r) if matches!(r.limits, syn::RangeLimits::HalfOpen(_)) => match (&r.start, &r.end) {
            (Some(a), Some(b)) => (a, b),
            _ => return reject("`for` takes a range with both ends, `a..b`"),
        },
        SynExpr::Range(_) => return reject("`for` takes a half-open range `a..b`, not `a..=b`"),
        SynExpr::MethodCall(m) if m.args.is_empty() && m.turbofish.is_none() && m.method == "chars" => {
            return each(Over::Chars, &m.receiver);
        }
        SynExpr::MethodCall(m) if m.args.is_empty() && m.turbofish.is_none() && m.method == "bytes" => {
            return each(Over::Bytes, &m.receiver);
        }
        SynExpr::MethodCall(m)
            if m.args.is_empty() && m.turbofish.is_none() && (m.method == "iter" || m.method == "into_iter") =>
        {
            return each(Over::Items, &m.receiver);
        }
        SynExpr::MethodCall(m) if m.args.len() == 1 && m.turbofish.is_none() && m.method == "split" => {
            return Ok(Expr::ForEach {
                var,
                over: Over::Items,
                source: Box::new(Expr::Call {
                    callee: Callee::StrSplit,
                    args: vec![lower_expr(cx, &m.receiver)?, lower_expr(cx, &m.args[0])?],
                }),
                body: Box::new(lower_block(cx, &f.body)?),
            });
        }
        SynExpr::MethodCall(m) if ITERATOR_ADAPTORS.contains(&m.method.to_string().as_str()) => {
            return reject(&format!(
                "`for` over `.{}()` is not in v0: iterate `a..b`, a `Vec` or slice, `s.chars()`, or `s.bytes()`, and keep an index or a counter by hand",
                m.method
            ))
        }
        // Any other value, `s.as_bytes()` or a method returning a `Vec`
        // included: the types say whether it is one.
        other => return each(Over::Items, other),
    };
    Ok(Expr::For {
        var,
        ty: None,
        start: Box::new(lower_expr(cx, start)?),
        end: Box::new(lower_expr(cx, end)?),
        body: Box::new(lower_block(cx, &f.body)?),
    })
}

pub fn lower_block(cx: &Cx, block: &syn::Block) -> Result<Expr, ParseError> {
    match lower_block_node(cx, block).map_err(|e| e.or_at(block.span())) {
        Err(e) if cx.recover(&e) => Ok(Expr::Unreachable),
        other => other,
    }
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
        let span = stmt.span();
        let lowered = match stmt {
            syn::Stmt::Local(local) => lower_local(cx, local).map(|l| stmts.push((span, l))),
            syn::Stmt::Expr(e, None) if i == last => lower_expr(cx, e).map(|e| tail = Some(at(span, e))),
            // `return x;` ends the block with the same meaning as `return x`.
            syn::Stmt::Expr(e @ SynExpr::Return(_), Some(_)) if i == last => {
                lower_expr(cx, e).map(|e| tail = Some(at(span, e)))
            }
            syn::Stmt::Expr(e, _) => lower_expr(cx, e).map(|e| stmts.push((span, Stmt::Effect(e)))),
            syn::Stmt::Item(_) => Err(ParseError::new(Reason::BlockItem, "items inside blocks are not in v0")),
            syn::Stmt::Macro(m) => {
                let name = path_text(&m.mac.path);
                Err(ParseError::new(Reason::Macro, format!("macro `{name}!` is not in v0")).detail(name))
            }
        };
        // In recovery (`survey --all-causes`) a statement that fails is
        // recorded and left out, and the rest of the block goes on.
        if let Err(e) = lowered {
            let e = e.or_at(span);
            if !cx.recover(&e) {
                return Err(e);
            }
        }
    }
    let acc = tail.unwrap_or(Expr::Lit(Lit::Unit));
    Ok(stmts.into_iter().rev().fold(acc, |then, (span, stmt)| {
        let node = match stmt {
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
        };
        at(span, node)
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
        SynBinOp::BitAndAssign(_) => SynBinOp::BitAnd(token::And::default()),
        SynBinOp::BitOrAssign(_) => SynBinOp::BitOr(token::Or::default()),
        SynBinOp::BitXorAssign(_) => SynBinOp::BitXor(token::Caret::default()),
        SynBinOp::ShlAssign(_) => SynBinOp::Shl(token::Shl::default()),
        SynBinOp::ShrAssign(_) => SynBinOp::Shr(token::Shr::default()),
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

fn wrapper_new(segs: &[String]) -> Option<Wrapper> {
    match segs {
        [name, new] if new == "new" => match name.as_str() {
            "Box" => Some(Wrapper::Box),
            "Arc" => Some(Wrapper::Arc),
            "Mutex" => Some(Wrapper::Mutex),
            _ => None,
        },
        _ => None,
    }
}

fn int_from(segs: &[String]) -> Option<IntTy> {
    match segs {
        [ty, from] if from == "from" => IntTy::of_suffix(ty),
        _ => None,
    }
}

/// `Uuid::parse_str`, `Uuid::try_parse`, `Uuid::nil`, also through `uuid::`.
fn uuid_fn(segs: &[String]) -> Option<Callee> {
    let segs: Vec<&str> = segs.iter().map(String::as_str).collect();
    let rest = match segs.as_slice() {
        ["uuid", "Uuid", rest @ ..] | ["Uuid", rest @ ..] => rest,
        _ => return None,
    };
    match rest {
        ["parse_str"] | ["try_parse"] => Some(Callee::UuidParse),
        ["nil"] => Some(Callee::UuidNil),
        _ => None,
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
            if let Some(wrapper) = wrapper_new(&segs) {
                if args.len() != 1 {
                    return Err(ParseError::new(
                        Reason::ConstructShape,
                        format!("`{}::new` takes 1 argument, got {}", wrapper.rust_name(), args.len()),
                    ));
                }
                return Ok(Expr::Ignored {
                    wrapper,
                    expr: Box::new(args.into_iter().next().unwrap()),
                });
            }
            let callee = if segs == ["Some"] {
                Callee::OptionSome
            } else if segs == ["None"] {
                Callee::OptionNone
            } else if segs == ["Ok"] {
                Callee::ResultOk
            } else if segs == ["Err"] {
                Callee::ResultErr
            } else if segs == ["String", "from"] {
                Callee::StringFrom
            } else if segs == ["char", "from"] {
                Callee::CharFromU8
            } else if segs == ["char", "from_u32"] {
                Callee::CharFromU32
            } else if let Some(callee) = uuid_fn(&segs) {
                callee
            } else if let Some(to) = int_from(&segs) {
                Callee::IntFrom { from: None, to }
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

/// A `match` with guards, as one without: arms are tried in order, and a
/// guarded arm is `if match $g { p => guard, _ => false } { match $g { p =>
/// body, _ => unreachable } } else { <the arms after it> }`, where `$g` holds
/// the scrutinee (each element of a tuple scrutinee) so it is evaluated
/// once. The guard runs only when its pattern matched, and the arms left
/// after the guarded ones must be exhaustive by themselves, as rustc
/// requires. `n if n > 3 =>` binds `n` to the scrutinee.
fn lower_guarded(cx: &Cx, scrutinee: &SynExpr, arms: &[syn::Arm]) -> Result<Expr, ParseError> {
    // Bind the scrutinee (or each tuple element) once.
    let mut binds: Vec<(Name, Expr)> = Vec::new();
    let subject = match scrutinee {
        SynExpr::Tuple(t) if !t.elems.is_empty() => {
            let mut elems = Vec::new();
            for e in &t.elems {
                let name = cx.fresh("g");
                binds.push((name.clone(), lower_expr(cx, e)?));
                elems.push(Expr::Var(name));
            }
            Expr::Tuple(elems)
        }
        other => {
            let name = cx.fresh("g");
            binds.push((name.clone(), lower_expr(cx, other)?));
            Expr::Var(name)
        }
    };
    let tuple = matches!(subject, Expr::Tuple(_));
    let mut lowered = Vec::new();
    for arm in arms {
        let raw = lower_pat(cx, &arm.pat)?;
        let guard = match &arm.guard {
            Some((_, g)) => Some(lower_expr(cx, g)?),
            None => None,
        };
        let pattern = match (&raw, &guard) {
            (Pattern::Var(_), Some(_)) if !tuple => raw,
            _ => arm_pattern(raw).map_err(|e| e.or_at(arm.pat.span()))?,
        };
        lowered.push((pattern, guard, at(arm.body.span(), lower_expr(cx, &arm.body)?)));
    }
    let chain = guard_chain(&subject, &lowered);
    Ok(binds.into_iter().rev().fold(chain, |then, (name, value)| Expr::Let {
        name,
        mutable: false,
        ty: None,
        value: Box::new(value),
        then: Box::new(then),
    }))
}

/// `if hit_1 { take_1 } else if hit_2 { take_2 } .. else { unreachable }`,
/// arm by arm in order: `hit` is `match $g { p => guard, _ => false }` and
/// `take` is `match $g { p => body, _ => unreachable }`. Each `match` stands
/// alone rather than inside another on the same value, so TS narrows
/// nothing that a later `switch` would contradict. A pattern that always
/// matches (`_`, a binding, a tuple of those) needs no `match`.
fn guard_chain(subject: &Expr, arms: &[(Pattern, Option<Expr>, Expr)]) -> Expr {
    let mut acc = Expr::Unreachable;
    for (pattern, guard, body) in arms.iter().rev() {
        let (hit, take) = match always_binds(subject, pattern) {
            Some(lets) => {
                let wrap = |e: &Expr| {
                    lets.iter().rev().fold(e.clone(), |then, (name, value)| Expr::Let {
                        name: name.clone(),
                        mutable: false,
                        ty: None,
                        value: Box::new(value.clone()),
                        then: Box::new(then),
                    })
                };
                (guard.as_ref().map(wrap), wrap(body))
            }
            None => {
                let two = |first: Expr, rest: Expr| Expr::Match {
                    scrutinee: Box::new(subject.clone()),
                    arms: vec![
                        Arm { pattern: pattern.clone(), body: first },
                        Arm { pattern: Pattern::Wildcard, body: rest },
                    ],
                };
                let hit = two(guard.clone().unwrap_or(Expr::Lit(Lit::Bool(true))), Expr::Lit(Lit::Bool(false)));
                (Some(hit), two(body.clone(), Expr::Unreachable))
            }
        };
        acc = match hit {
            None => take,
            Some(hit) => Expr::If {
                cond: Box::new(hit),
                then: Box::new(take),
                else_: Box::new(acc),
            },
        };
    }
    acc
}

/// For a pattern that matches every value: the names it binds, each with
/// what it binds to.
fn always_binds(subject: &Expr, pattern: &Pattern) -> Option<Vec<(Name, Expr)>> {
    match (pattern, subject) {
        (Pattern::Wildcard, _) => Some(Vec::new()),
        (Pattern::Var(n), s) => Some(vec![(n.clone(), s.clone())]),
        (Pattern::Tuple(ps), Expr::Tuple(xs)) if ps.len() == xs.len() => {
            let mut out = Vec::new();
            for (p, x) in ps.iter().zip(xs) {
                out.extend(always_binds(x, p)?);
            }
            Some(out)
        }
        _ => None,
    }
}

/// `matches!(e, p)` is std's `match e { p => true, _ => false }`, and
/// `matches!(e, p if c)` is `match e { p => c, _ => false }`.
fn lower_matches(cx: &Cx, mac: &syn::Macro) -> Result<Expr, ParseError> {
    let (scrutinee, pat, guard) = mac
        .parse_body_with(|input: syn::parse::ParseStream| {
            let e: SynExpr = input.parse()?;
            input.parse::<syn::Token![,]>()?;
            let p = Pat::parse_multi_with_leading_vert(input)?;
            let guard = if input.peek(syn::Token![if]) {
                input.parse::<syn::Token![if]>()?;
                Some(input.parse::<SynExpr>()?)
            } else {
                None
            };
            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
            Ok((e, p, guard))
        })
        .map_err(|e| ParseError::new(Reason::Macro, format!("`matches!` expects `matches!(value, pattern)`: {e}")))?;
    let pattern = arm_pattern(lower_pat(cx, &pat)?).map_err(|e| e.or_at(pat.span()))?;
    if pattern == Pattern::Wildcard {
        return Err(ParseError::new(
            Reason::ArmPattern,
            "`matches!(x, _)` does not test `x`; write `true`, or the condition of `_ if c`",
        ));
    }
    let hit = match guard {
        Some(g) => lower_expr(cx, &g)?,
        None => Expr::Lit(Lit::Bool(true)),
    };
    Ok(Expr::Match {
        scrutinee: Box::new(lower_expr(cx, &scrutinee)?),
        arms: vec![
            Arm { pattern, body: hit },
            Arm {
                pattern: Pattern::Wildcard,
                body: Expr::Lit(Lit::Bool(false)),
            },
        ],
    })
}

/// `vec![a, b]`: a `Vec` of exactly the listed elements, printed as the
/// same array literal as `[a, b]` (which rustc types as an array, not a
/// `Vec`). The repeat form `vec![x; n]` is rejected: its length is a value.
fn lower_vec(cx: &Cx, mac: &syn::Macro) -> Result<Expr, ParseError> {
    let elems = mac
        .parse_body_with(|input: syn::parse::ParseStream| {
            let first: Option<SynExpr> = if input.is_empty() { None } else { Some(input.parse()?) };
            if first.is_some() && input.peek(syn::Token![;]) {
                input.parse::<syn::Token![;]>()?;
                input.parse::<SynExpr>()?;
                return Ok(None);
            }
            let mut elems: Vec<SynExpr> = first.into_iter().collect();
            while !input.is_empty() {
                input.parse::<syn::Token![,]>()?;
                if input.is_empty() {
                    break;
                }
                elems.push(input.parse()?);
            }
            Ok(Some(elems))
        })
        .map_err(|e| ParseError::new(Reason::Macro, format!("`vec!` expects `vec![a, b, ..]`: {e}")).detail("vec"))?
        .ok_or_else(|| {
            ParseError::new(Reason::Macro, "`vec![x; n]` is not in v0: list the elements, `vec![x, x]`").detail("vec")
        })?;
    Ok(Expr::Array(elems.iter().map(|e| lower_expr(cx, e)).collect::<Result<_, _>>()?))
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
        Pat::Range(r) => {
            let bound = |e: &Option<Box<syn::Expr>>| match e.as_deref() {
                Some(SynExpr::Lit(l)) => lower_lit(&l.lit),
                _ => Err(ParseError::new(
                    Reason::UnsupportedPattern,
                    "range patterns need a literal at both ends in v0",
                )),
            };
            Ok(Pattern::Range {
                lo: bound(&r.start)?,
                hi: bound(&r.end)?,
                inclusive: matches!(r.limits, syn::RangeLimits::Closed(_)),
            })
        }
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
        Pat::Tuple(t) if t.elems.len() > 1 => Ok(Pattern::Tuple(
            t.elems
                .iter()
                .map(|p| lower_pat(cx, p))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Pat::Or(o) => Ok(Pattern::Or(
            o.cases
                .iter()
                .map(|p| lower_pat(cx, p))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        other => Err(ParseError::new(
            Reason::UnsupportedPattern,
            format!("unsupported pattern {}", snippet(other)),
        )),
    }
}

/// v0 prints `match` as `switch (x.kind)`: each arm names one variant and
/// binds its fields to plain names, names several variants of one enum with
/// `A | B` binding nothing, or is `_` (the variants no other arm names).
fn arm_pattern(pattern: Pattern) -> Result<Pattern, ParseError> {
    if pattern.is_lit_case() {
        return Ok(pattern);
    }
    match &pattern {
        Pattern::Wildcard => return Ok(pattern),
        Pattern::Tuple(elems) => {
            tuple_elems(elems)?;
            return Ok(pattern);
        }
        Pattern::Or(alts) if pattern.is_tuple_case() => {
            for alt in alts {
                let Pattern::Tuple(elems) = alt else {
                    return Err(ParseError::new(Reason::ArmPattern, format!(
                        "each side of `|` must be a tuple pattern here, found {}",
                        describe_pat(alt)
                    )));
                };
                tuple_elems(elems)?;
            }
            if let Some(name) = pattern.bindings().first() {
                return Err(ParseError::new(Reason::ArmPattern, format!(
                    "`|` arms may not bind names in v0, found binding `{}`; write one arm per tuple",
                    name.as_str()
                )));
            }
            return Ok(pattern);
        }
        Pattern::Or(alts) => {
            for alt in alts {
                if !matches!(alt, Pattern::Variant { .. }) {
                    return Err(ParseError::new(Reason::ArmPattern, format!(
                        "each side of `|` must name an enum variant in v0, found {}",
                        describe_pat(alt)
                    )));
                }
                variant_fields(alt)?;
                if let Some(name) = alt.bindings().first() {
                    return Err(ParseError::new(Reason::ArmPattern, format!(
                        "`|` arms may not bind names in v0, found binding `{}`; write one arm per variant",
                        name.as_str()
                    )));
                }
            }
            return Ok(pattern);
        }
        _ => {}
    }
    variant_fields(&pattern)?;
    Ok(pattern)
}

/// The elements of a tuple arm: `_`, a binding, or what an arm of its own
/// may be. A tuple inside a tuple is not flattened.
fn tuple_elems(elems: &[Pattern]) -> Result<(), ParseError> {
    for elem in elems {
        match elem {
            Pattern::Wildcard | Pattern::Var(_) => {}
            p if p.is_tuple_case() => {
                return Err(ParseError::new(Reason::NestedPattern, "tuple patterns may not nest in v0"));
            }
            p => {
                arm_pattern(p.clone())?;
            }
        }
    }
    Ok(())
}

/// Checks that `pattern` names a case and binds only names or `_` inside.
fn variant_fields(pattern: &Pattern) -> Result<(), ParseError> {
    let inner: Vec<&Pattern> = match pattern {
        Pattern::Variant { bind, .. } => match bind {
            VariantBind::Unit => Vec::new(),
            VariantBind::Tuple(pats) => pats.iter().collect(),
            VariantBind::Struct(pairs) => pairs.iter().map(|(_, p)| p).collect(),
        },
        Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => vec![&**p],
        Pattern::OptionNone => Vec::new(),
        Pattern::Wildcard
        | Pattern::Var(_)
        | Pattern::Lit(_)
        | Pattern::Or(_)
        | Pattern::Range { .. }
        | Pattern::Tuple(_) => {
            return Err(ParseError::new(Reason::ArmPattern, format!(
                "match arms must name an enum variant, `Some`/`None`, `Ok`/`Err`, an integer, a `char`, or a range of either, a string literal, or be `_` in v0, found {}",
                describe_pat(pattern)
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
    Ok(())
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
        Pattern::Or(_) => "`|`".into(),
        Pattern::Range { .. } => "a range".into(),
        Pattern::Tuple(_) => "a tuple".into(),
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
        syn::Lit::Char(c) => Ok(Lit::Char(c.value())),
        // `b'@'` is a `u8`.
        syn::Lit::Byte(b) => Ok(Lit::Int {
            value: i128::from(b.value()),
            ty: Some(IntTy::U8),
        }),
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
        SynBinOp::BitAnd(_) => BinOp::BitAnd,
        SynBinOp::BitOr(_) => BinOp::BitOr,
        SynBinOp::BitXor(_) => BinOp::BitXor,
        SynBinOp::Shl(_) => BinOp::Shl,
        SynBinOp::Shr(_) => BinOp::Shr,
        _ => {
            let text = snippet(&op);
            return Err(ParseError::new(Reason::UnsupportedOperator, format!("operator {text} is not in v0")).detail(text.trim_matches('`')));
        }
    })
}

fn lower_un(op: SynUnOp) -> Result<UnOp, ParseError> {
    match op {
        SynUnOp::Not(_) => Ok(UnOp::Not),
        SynUnOp::Neg(_) => Ok(UnOp::Neg),
        _ => {
            let text = snippet(&op);
            Err(ParseError::new(Reason::UnsupportedOperator, format!("operator {text} is not in v0")).detail(text.trim_matches('`')))
        }
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
