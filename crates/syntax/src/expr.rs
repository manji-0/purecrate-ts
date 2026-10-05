use purecrate_ir::{
    Arm, BinOp, Callee, ClosureParam, Expr, Fields, FloatTy, IntTy, Lit, Name, Over, Pattern, Pos, Prim, Reason, Ty,
    UnOp, VariantBind, Wrapper, NEWTYPE_FIELD,
};
use syn::spanned::Spanned;
use syn::{BinOp as SynBinOp, Expr as SynExpr, Item as SynItem, Member, Pat, UnOp as SynUnOp};

use crate::item::{snippet, Cx, LineCol, ParseError};
use crate::ty::lower_type;

mod pattern;

use pattern::*;

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
            Member::Named(id) => {
                Ok(Expr::Field { base: Box::new(lower_expr(cx, &f.base)?), name: Name::new(id.to_string()) })
            }
            Member::Unnamed(i) if i.index == 0 => {
                Ok(Expr::Field { base: Box::new(lower_expr(cx, &f.base)?), name: Name::new(NEWTYPE_FIELD) })
            }
            Member::Unnamed(_) => Err(ParseError::new(Reason::TupleField, "tuple field access is not in v0")),
        },
        SynExpr::Assign(a) => {
            Ok(Expr::Assign { name: assign_target(&a.left)?, value: Box::new(lower_expr(cx, &a.right)?) })
        }
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
        SynExpr::Unary(u) => Ok(Expr::Unary { op: lower_un(u.op)?, expr: Box::new(lower_expr(cx, &u.expr)?) }),
        SynExpr::Try(t) => Ok(Expr::Try { expr: Box::new(lower_expr(cx, &t.expr)?), on: None }),
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
        SynExpr::Match(m) if m.arms.iter().any(|a| a.guard.is_some() || pattern::has_struct_pat(cx, &a.pat)) => {
            lower_guarded(cx, &m.expr, &m.arms, &Comments::of_span(m.span()))
        }
        SynExpr::Match(m) => {
            // A `//` above an arm opens its body.
            let comments = Comments::of_span(m.span());
            let mut arms = Vec::new();
            for arm in &m.arms {
                arms.push(Arm {
                    guard: None,
                    pattern: arm_pattern(lower_pat(cx, &arm.pat)?).map_err(|e| e.or_at(arm.pat.span()))?,
                    body: comments.above(arm.span(), at(arm.body.span(), lower_expr(cx, &arm.body)?)),
                });
            }
            Ok(Expr::Match { scrutinee: Box::new(lower_expr(cx, &m.expr)?), arms })
        }
        SynExpr::Struct(s) => lower_struct_expr(cx, s),
        // Calls carry their own position, so a diagnostic about one points
        // at it rather than at the statement around it.
        // `Vec::new()` is `vec![]`: an empty array whose type the context gives.
        SynExpr::Call(c)
            if c.args.is_empty()
                && matches!(&*c.func, SynExpr::Path(p) if p.qself.is_none() && p.path.segments.len() == 2
                    && p.path.segments[0].ident == "Vec" && p.path.segments[1].ident == "new") =>
        {
            Ok(at(c.span(), Expr::Array(Vec::new())))
        }
        SynExpr::Call(c) => Ok(at(c.span(), lower_call(cx, &c.func, c.args.iter().collect())?)),
        // `()` is the unit value, not an empty tuple: its type is `()` and
        // it prints as `undefined`.
        SynExpr::Tuple(t) if t.elems.is_empty() => Ok(Expr::Lit(Lit::Unit)),
        SynExpr::Tuple(t) => {
            let elems = t.elems.iter().map(|e| lower_expr(cx, e)).collect::<Result<Vec<_>, _>>()?;
            Ok(Expr::Tuple(elems))
        }
        SynExpr::Array(a) => {
            let elems = a.elems.iter().map(|e| lower_expr(cx, e)).collect::<Result<Vec<_>, _>>()?;
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
        SynExpr::Macro(m) => {
            Err(ParseError::new(Reason::Macro, format!("macro `{}!` is not in v0", path_text(&m.mac.path)))
                .detail(path_text(&m.mac.path)))
        }
        // `xs.iter().sum::<T>()`, `s.split(c).collect::<T>()`: the type
        // argument annotates the result. `s.parse::<T>()` is a
        // `Result<T, ParseIntError>`.
        SynExpr::MethodCall(m)
            if (m.method == "sum" || m.method == "collect" || m.method == "parse")
                && m.args.is_empty()
                && m.turbofish.as_ref().is_some_and(|t| t.args.len() == 1) =>
        {
            let method = m.method.to_string();
            let Some(syn::GenericArgument::Type(ty)) = m.turbofish.as_ref().and_then(|t| t.args.first()) else {
                return Err(ParseError::new(Reason::MethodCall, format!("`{method}::<T>()` takes a type")));
            };
            let ty = if method == "collect" { crate::ty::lower_type_with_holes(ty)? } else { lower_type(ty)? };
            let ty = if method == "parse" { Ty::result(ty, Ty::Prim(Prim::ParseIntError)) } else { ty };
            let name = cx.fresh(&method);
            let call = at(
                m.method.span(),
                Expr::MethodCall {
                    receiver: Box::new(lower_expr(cx, &m.receiver)?),
                    name: Name::new(method),
                    args: Vec::new(),
                },
            );
            Ok(Expr::Let {
                name: name.clone(),
                mutable: false,
                ty: Some(ty),
                value: Box::new(call),
                then: Box::new(Expr::Var(name)),
            })
        }
        SynExpr::MethodCall(m) if m.turbofish.is_some() => {
            Err(ParseError::new(Reason::MethodCall, format!("method call `.{}::<..>()` is not in v0", m.method))
                .detail(m.method.to_string()))
        }
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
        SynExpr::While(w) => {
            Ok(Expr::While { cond: Box::new(lower_expr(cx, &w.cond)?), body: Box::new(lower_block(cx, &w.body)?) })
        }
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
        SynExpr::Reference(r) if r.mutability.is_some() => {
            Err(ParseError::new(Reason::Borrow, format!("`&mut` borrows are not in v0: {}", snippet(expr))))
        }
        SynExpr::Reference(r) => lower_expr(cx, &r.expr),
        SynExpr::Index(i) => match &*i.index {
            SynExpr::Range(r) if matches!(r.limits, syn::RangeLimits::HalfOpen(_)) => {
                let mut args = vec![lower_expr(cx, &i.expr)?];
                for bound in [&r.start, &r.end].into_iter().flatten() {
                    args.push(lower_expr(cx, bound)?);
                }
                Ok(Expr::Call {
                    callee: Callee::Slice { of: None, start: r.start.is_some(), end: r.end.is_some() },
                    args,
                })
            }
            SynExpr::Range(_) => Err(ParseError::new(
                Reason::Range,
                format!(
                    "slicing takes a half-open range `a..b`, `a..`, or `..b` in v0, not `a..=b`: {}",
                    snippet(expr)
                ),
            )),
            index => {
                Ok(Expr::Index { base: Box::new(lower_expr(cx, &i.expr)?), index: Box::new(lower_expr(cx, index)?) })
            }
        },
        SynExpr::Range(_) => Err(ParseError::new(Reason::Range, format!("ranges are not in v0: {}", snippet(expr)))),
        SynExpr::Cast(c) => Ok(Expr::Cast {
            expr: Box::new(lower_expr(cx, &c.expr)?),
            to: lower_type(&c.ty)
                .map_err(|_| ParseError::new(Reason::Cast, format!("`as` casts are not in v0: {}", snippet(expr))))?,
        }),
        other => Err(ParseError::new(Reason::UnsupportedExpr, format!("unsupported expression {}", snippet(other)))
            .detail(expr_kind(other))),
    }
}

/// `expr`, marked with where its source starts, for diagnostics.
fn at(span: proc_macro2::Span, expr: Expr) -> Expr {
    let LineCol { line, col } = LineCol::of(span);
    Expr::At { at: Pos { line: line as u32, col: col as u32 }, expr: Box::new(expr) }
}

/// Iterator adaptors, named so that `for` over one says so rather than
/// failing later on the method call.
const ITERATOR_ADAPTORS: &[&str] = &[
    "enumerate",
    "rev",
    "zip",
    "map",
    "filter",
    "filter_map",
    "skip",
    "take",
    "step_by",
    "windows",
    "chunks",
    "cloned",
    "copied",
    "peekable",
    "char_indices",
    "split_whitespace",
    "splitn",
    "rsplit",
    "lines",
    "keys",
    "values",
];

/// `for` over a half-open integer range, a string's `chars()`, `bytes()`,
/// or `split(c)` on a `char`, or a `Vec` or slice (`xs`, `&xs`, `xs.iter()`), unlabelled,
/// with a plain name for the variable. Whether `xs` is a `Vec` is checked
/// with the types. Iterator adaptors (`enumerate`, `rev`, `zip`, ...) and
/// `a..=b` stay out.
fn lower_for(cx: &Cx, f: &syn::ExprForLoop) -> Result<Expr, ParseError> {
    let reject =
        |what: &str| Err(ParseError::new(Reason::Loop, format!("{what}: {}", snippet(&SynExpr::ForLoop(f.clone())))));
    if f.label.is_some() {
        return reject("loop labels are not in v0");
    }
    // `for (i, x) in <items>.enumerate()`: a `usize` counter beside the loop,
    // read into `i` and then advanced at the top of each pass, so
    // `continue` keeps it right.
    let (iterable, counter) = match &*f.expr {
        SynExpr::MethodCall(m) if m.method == "enumerate" && m.args.is_empty() && m.turbofish.is_none() => {
            if matches!(&*m.receiver, SynExpr::Range(_)) {
                return reject("`for` over a range's `.enumerate()` is not in v0; the range's own value is the index");
            }
            (&*m.receiver, Some(cx.fresh("i")))
        }
        other => (other, None),
    };
    // `for (a, b) in xs` takes a fresh variable and destructures it in the
    // body; only an item loop yields tuples.
    let tuple = Destructure::of(cx, &f.pat, Reason::Loop)?;
    if counter.is_some() && tuple.as_ref().is_none_or(|t| t.len() != 2) {
        return reject("`for` over `.enumerate()` takes a pair `(i, x)`");
    }
    let mut tuple = tuple;
    let var = match &*f.pat {
        // `(i, x)` of `enumerate`: the loop iterates `x` itself.
        _ if counter.is_some() => match tuple.as_mut().and_then(|t| t.take_last_name()) {
            Some(name) => name,
            None => cx.fresh("p"),
        },
        _ if tuple.is_some() => cx.fresh("p"),
        Pat::Ident(p) if p.by_ref.is_none() && p.mutability.is_none() && p.subpat.is_none() => {
            Name::new(p.ident.to_string())
        }
        // `for _ in 0..n`: a name nothing reads.
        Pat::Wild(_) => cx.fresh("i"),
        _ => {
            return reject(
                "`for` takes a plain name, `_`, or a tuple of names for its variable, not `mut` or another pattern",
            )
        }
    };
    // Lowered after the source, so fresh names and diagnostics keep source order.
    let tuple = std::cell::Cell::new(tuple);
    let body = || -> Result<Expr, ParseError> {
        let body = lower_block(cx, &f.body)?;
        Ok(match (tuple.take(), &counter) {
            (Some(tuple), Some(i)) => {
                let advance = Expr::Assign {
                    name: i.clone(),
                    value: Box::new(Expr::Binary {
                        op: BinOp::Add,
                        left: Box::new(Expr::Var(i.clone())),
                        right: Box::new(Expr::Lit(Lit::Int {
                            value: 1,
                            ty: Some(IntTy::Usize),
                            byte: false,
                            hex: false,
                        })),
                    }),
                };
                let body = Expr::Seq { first: Box::new(advance), then: Box::new(body) };
                tuple.bind(Expr::Tuple(vec![Expr::Var(i.clone()), Expr::Var(var.clone())]), body)
            }
            (Some(tuple), None) => tuple.bind(Expr::Var(var.clone()), body),
            (None, _) => body,
        })
    };
    let counted = |each: Expr| match &counter {
        Some(i) => Expr::Let {
            name: i.clone(),
            mutable: true,
            ty: Some(Ty::Prim(purecrate_ir::Prim::Usize)),
            value: Box::new(Expr::Lit(Lit::Int { value: 0, ty: Some(IntTy::Usize), byte: false, hex: false })),
            then: Box::new(each),
        },
        None => each,
    };
    let each = |over: Over, source: &SynExpr| -> Result<Expr, ParseError> {
        let source = lower_expr(cx, source)?;
        Ok(counted(Expr::ForEach { var: var.clone(), over, source: Box::new(source), body: Box::new(body()?) }))
    };
    let (start, end) = match iterable {
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
            let source = Expr::Call {
                callee: Callee::StrSplit,
                args: vec![lower_expr(cx, &m.receiver)?, lower_expr(cx, &m.args[0])?],
            };
            return Ok(counted(Expr::ForEach {
                var: var.clone(),
                over: Over::Items,
                source: Box::new(source),
                body: Box::new(body()?),
            }));
        }
        SynExpr::MethodCall(m) if ITERATOR_ADAPTORS.contains(&m.method.to_string().as_str()) => {
            return reject(&format!(
                "`for` over `.{}()` is not in v0: iterate `a..b`, a `Vec` or slice, `s.chars()`, or `s.bytes()`, with `.enumerate()` for an index",
                m.method
            ))
        }
        // Any other value, `s.as_bytes()` or a method returning a `Vec`
        // included: the types say whether it is one.
        other => return each(Over::Items, other),
    };
    let (start, end) = (lower_expr(cx, start)?, lower_expr(cx, end)?);
    Ok(Expr::For { var: var.clone(), ty: None, start: Box::new(start), end: Box::new(end), body: Box::new(body()?) })
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
    /// `let (a, b) = value;`, typed through `name` when annotated.
    Destructure {
        tuple: Destructure,
        typed: Option<(Name, Ty)>,
        value: Expr,
    },
    Effect(Expr),
}

/// A block's `const` items, as immutable typed `let`s at its top in
/// declaration order: an item is visible from the whole block, and rustc,
/// which runs on the input, rejects a const whose evaluation fails, so the
/// TS computation never panics where Rust would have folded a value.
fn lower_block_node(cx: &Cx, block: &syn::Block) -> Result<Expr, ParseError> {
    let consts: Vec<&syn::ItemConst> = block
        .stmts
        .iter()
        .filter_map(|s| match s {
            syn::Stmt::Item(SynItem::Const(c)) => Some(c),
            _ => None,
        })
        .collect();
    let names = consts.iter().map(|c| c.ident.to_string()).collect();
    cx.with_local_consts(names, || lower_block_stmts(cx, block, &consts))
}

fn lower_block_stmts(cx: &Cx, block: &syn::Block, consts: &[&syn::ItemConst]) -> Result<Expr, ParseError> {
    let mut stmts = Vec::new();
    for c in consts {
        let span = c.span();
        let lowered = (|| {
            if c.ident == "_" {
                return Err(ParseError::new(Reason::UnsupportedItem, "`const _` is not in v0").detail("const"));
            }
            Ok(Stmt::Let {
                name: Name::new(c.ident.to_string()),
                mutable: false,
                ty: Some(lower_type(&c.ty)?),
                value: lower_expr(cx, &c.expr)?,
            })
        })();
        match lowered {
            Ok(stmt) => stmts.push((span, stmt)),
            Err(e) => {
                let e = e.or_at(span);
                if !cx.recover(&e) {
                    return Err(e);
                }
            }
        }
    }
    let mut tail: Option<Expr> = None;
    let comments = Comments::of(block);
    let last = block.stmts.len().saturating_sub(1);
    for (i, stmt) in block.stmts.iter().enumerate() {
        let span = stmt.span();
        let lowered = match stmt {
            syn::Stmt::Local(local) => lower_local(cx, local).map(|l| stmts.push((span, l))),
            syn::Stmt::Expr(e, None) if i == last => {
                lower_expr(cx, e).map(|e| tail = Some(comments.above(span, at(span, e))))
            }
            // `return x;` ends the block with the same meaning as `return x`.
            syn::Stmt::Expr(e @ SynExpr::Return(_), Some(_)) if i == last => {
                lower_expr(cx, e).map(|e| tail = Some(comments.above(span, at(span, e))))
            }
            syn::Stmt::Expr(e, _) => lower_expr(cx, e).map(|e| stmts.push((span, Stmt::Effect(e)))),
            syn::Stmt::Item(SynItem::Const(_)) => Ok(()),
            syn::Stmt::Item(_) => Err(ParseError::new(
                Reason::BlockItem,
                "items inside blocks other than `const` are not in v0; move the item to the crate level",
            )),
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
    // A comment before the `}` of a block that ends in a statement ends it.
    let closing = if tail.is_none() { comments.closing(block.stmts.last().map(|s| s.span())) } else { Vec::new() };
    let acc = tail.unwrap_or(Expr::Lit(Lit::Unit));
    let acc = if closing.is_empty() {
        acc
    } else {
        Expr::Seq { first: Box::new(Expr::Comment(closing)), then: Box::new(acc) }
    };
    Ok(stmts.into_iter().rev().fold(acc, |then, (span, stmt)| {
        let node = match stmt {
            Stmt::Let { name, mutable, ty, value } => {
                Expr::Let { name, mutable, ty, value: Box::new(value), then: Box::new(then) }
            }
            Stmt::Destructure { tuple, typed, value } => match typed {
                Some((name, ty)) => Expr::Let {
                    name: name.clone(),
                    mutable: false,
                    ty: Some(ty),
                    value: Box::new(value),
                    then: Box::new(tuple.bind(Expr::Var(name), then)),
                },
                // `let (a, b) = match x { .., None => return 0 };`: a
                // `return` may end a `let`'s value but not a `match`'s
                // scrutinee, so the value is bound first.
                None if value.any(|e| matches!(e, Expr::Return(_))) => {
                    let name = cx.fresh("pair");
                    Expr::Let {
                        name: name.clone(),
                        mutable: false,
                        ty: None,
                        value: Box::new(value),
                        then: Box::new(tuple.bind(Expr::Var(name), then)),
                    }
                }
                None => tuple.bind(value, then),
            },
            Stmt::Effect(first) => Expr::Seq { first: Box::new(first), then: Box::new(then) },
        };
        comments.above(span, at(span, node))
    }))
}

/// The source lines of a block or a `match`, to find the `//` comments
/// around each of its statements or arms; syn keeps no comments.
pub(super) struct Comments {
    lines: Vec<String>,
    first: usize,
}

/// The text of a `//` comment line, or `None` for code, a blank line,
/// `///`, `//!`, or `/* */`.
fn comment_text(line: &str) -> Option<String> {
    match line.trim().strip_prefix("//") {
        Some(rest) if !rest.starts_with('/') && !rest.starts_with('!') => {
            Some(one_line(rest.strip_prefix(' ').unwrap_or(rest).trim_end()))
        }
        _ => None,
    }
}

/// `text` with what JS reads as a line end and Rust does not (a lone CR,
/// U+2028, U+2029) made a space: in a printed `//` comment, the rest of
/// the line would be code.
pub(crate) fn one_line(text: &str) -> String {
    text.replace(['\r', '\u{2028}', '\u{2029}'], " ")
}

/// Where a `//` comment starts in `line` after code: outside a string, a
/// `char`, and a lifetime-free byte literal.
fn line_end_comment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(_) if b == b'\\' => i += 1,
            Some(q) if b == q => quote = None,
            Some(_) => {}
            // A `'` opens a `char` only where one ends two or three bytes on.
            None if b == b'\''
                && (bytes.get(i + 2) == Some(&b'\'')
                    || (bytes.get(i + 1) == Some(&b'\\') && bytes.get(i + 3) == Some(&b'\''))) =>
            {
                quote = Some(b'\'')
            }
            None if b == b'"' => quote = Some(b'"'),
            None if b == b'/' && bytes.get(i + 1) == Some(&b'/') => return Some(i),
            None => {}
        }
        i += 1;
    }
    None
}

impl Comments {
    fn of(block: &syn::Block) -> Self {
        Self::of_span(block.span())
    }

    pub(super) fn of_span(span: proc_macro2::Span) -> Self {
        Self {
            lines: span.source_text().unwrap_or_default().lines().map(String::from).collect(),
            first: span.start().line,
        }
    }

    /// `node` after the `//` lines above `span`: the nearest run of them,
    /// across blank lines between it and `span`, and a `//` after the code
    /// on `span`'s last line, moved above it. Code, `///`, `//!`, or
    /// `/* */` ends the run.
    pub(super) fn above(&self, span: proc_macro2::Span, node: Expr) -> Expr {
        let mut text = Vec::new();
        // The first line holds the block's `{` or the `match`.
        for i in (1..span.start().line.saturating_sub(self.first)).rev() {
            let line = self.lines.get(i).map_or("", |l| l.as_str());
            match comment_text(line) {
                Some(t) => text.push(t),
                None if line.trim().is_empty() && text.is_empty() => {}
                None => break,
            }
        }
        text.reverse();
        // The first line starts at the block's `{`, not at column 0.
        let last = span.end().line.saturating_sub(self.first);
        if let Some(line) = self.lines.get(last).filter(|_| last > 0) {
            let tail = line.get(span.end().column..).unwrap_or("");
            if let Some(at) = line_end_comment(tail) {
                if let Some(t) = comment_text(&tail[at..]) {
                    text.push(t);
                }
            }
        }
        if text.is_empty() {
            return node;
        }
        Expr::Seq { first: Box::new(Expr::Comment(text)), then: Box::new(node) }
    }

    /// The `//` lines after the last statement of the block, before its `}`.
    fn closing(&self, after: Option<proc_macro2::Span>) -> Vec<String> {
        let from = after.map_or(1, |s| s.end().line.saturating_sub(self.first) + 1);
        let to = self.lines.len().saturating_sub(1);
        (from..to).filter_map(|i| comment_text(&self.lines[i])).collect()
    }
}

fn lower_local(cx: &Cx, local: &syn::Local) -> Result<Stmt, ParseError> {
    let (pat, ty) = match &local.pat {
        Pat::Type(t) => (&*t.pat, Some(lower_type(&t.ty)?)),
        other => (other, None),
    };
    if let Some(tuple) = Destructure::of(cx, pat, Reason::LetPattern)? {
        let init = local.init.as_ref().ok_or_else(|| ParseError::new(Reason::LetPattern, "let without initializer"))?;
        if init.diverge.is_some() {
            return Err(ParseError::new(Reason::LetElse, "let-else is not in v0"));
        }
        return Ok(Stmt::Destructure {
            tuple,
            typed: ty.map(|ty| (cx.fresh("t"), ty)),
            value: lower_expr(cx, &init.expr)?,
        });
    }
    let (name, mutable) = match pat {
        Pat::Ident(id) if cx.is_local_const(&id.ident.to_string()) => return Err(const_pattern(&id.ident.to_string())),
        Pat::Ident(id) if id.by_ref.is_none() && id.subpat.is_none() => {
            (Name::new(id.ident.to_string()), id.mutability.is_some())
        }
        _ => {
            return Err(ParseError::new(
                Reason::LetPattern,
                "a `let` binds a name, `mut` a name, or a tuple of them in v0",
            ))
        }
    };
    let init = local.init.as_ref().ok_or_else(|| ParseError::new(Reason::LetPattern, "let without initializer"))?;
    if init.diverge.is_some() {
        return Err(ParseError::new(Reason::LetElse, "let-else is not in v0"));
    }
    Ok(Stmt::Let { name, mutable, ty, value: lower_expr(cx, &init.expr)? })
}

fn assign_target(place: &SynExpr) -> Result<Name, ParseError> {
    match place {
        SynExpr::Path(p) if p.qself.is_none() => p
            .path
            .get_ident()
            .map(|id| Name::new(id.to_string()))
            .ok_or_else(|| ParseError::new(Reason::PlaceAssign, "only a local variable can be assigned in v0")),
        SynExpr::Field(_) => Err(ParseError::new(
            Reason::PlaceAssign,
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
            "None" => Ok(Expr::Call { callee: Callee::OptionNone, args: vec![] }),
            _ => Ok(Expr::var(one.clone())),
        },
        [ty, var] if cx.is_enum(ty) => Ok(Expr::Construct {
            ty: Name::new(ty.clone()),
            variant: Some(Name::new(var.clone())),
            fields: Fields::Unit,
            base: None,
        }),
        [ty, var] if ty == "Result" && (var == "ok" || var == "Ok") => {
            Ok(Expr::Call { callee: Callee::ResultOk, args: vec![] })
        }
        [ty, var] if ty == "Result" && (var == "err" || var == "Err") => {
            Ok(Expr::Call { callee: Callee::ResultErr, args: vec![] })
        }
        [a, b] => Ok(Expr::Call { callee: Callee::Fn(Name::new(format!("{a}::{b}"))), args: vec![] }),
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
    // `|(a, b)| body` takes a fresh parameter and destructures it in the body.
    let mut tuples = Vec::new();
    let params = c
        .inputs
        .iter()
        .map(|p| {
            let (pat, ty) = match p {
                Pat::Type(t) => (&*t.pat, Some(lower_type(&t.ty)?)),
                other => (other, None),
            };
            if let Some(tuple) = Destructure::of(cx, pat, Reason::ParamPattern)? {
                let name = cx.fresh("p");
                tuples.push((name.clone(), tuple));
                return Ok(ClosureParam { name, ty });
            }
            match pat {
                Pat::Ident(id) if id.by_ref.is_none() && id.mutability.is_none() && id.subpat.is_none() => {
                    Ok(ClosureParam { name: Name::new(id.ident.to_string()), ty })
                }
                Pat::Wild(_) => Ok(ClosureParam { name: Name::new("_"), ty }),
                // `|&x|`: a reference reads as its value.
                Pat::Reference(r) if r.mutability.is_none() => match &*r.pat {
                    Pat::Ident(id) if id.by_ref.is_none() && id.mutability.is_none() && id.subpat.is_none() => {
                        Ok(ClosureParam { name: Name::new(id.ident.to_string()), ty })
                    }
                    other => Err(ParseError::new(
                        Reason::ParamPattern,
                        format!("closure parameters are plain names or tuples of them in v0, found {}", snippet(other)),
                    )
                    .or_at(other.span())),
                },
                other => Err(ParseError::new(
                    Reason::ParamPattern,
                    format!("closure parameters are plain names or tuples of them in v0, found {}", snippet(other)),
                )
                .or_at(other.span())),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ret = match &c.output {
        syn::ReturnType::Default => None,
        syn::ReturnType::Type(_, t) => Some(lower_type(t)?),
    };
    let body = tuples
        .into_iter()
        .rev()
        .fold(lower_expr(cx, &c.body)?, |body, (name, tuple)| tuple.bind(Expr::Var(name), body));
    Ok(Expr::Closure { params, ret, body: Box::new(body) })
}

fn lower_struct_expr(cx: &Cx, s: &syn::ExprStruct) -> Result<Expr, ParseError> {
    let segs: Vec<String> = s.path.segments.iter().map(|p| p.ident.to_string()).collect();
    if s.rest.is_some() && segs.len() != 1 {
        return Err(ParseError::new(Reason::StructUpdate, "functional record update syntax requires a struct"));
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
        [ty] if cx.is_struct(ty) => Ok(Expr::Construct { ty: Name::new(ty.clone()), variant: None, fields, base }),
        [ty, var] if cx.is_enum(ty) => {
            Ok(Expr::Construct { ty: Name::new(ty.clone()), variant: Some(Name::new(var.clone())), fields, base: None })
        }
        _ => Err(path_error(&segs, format!("unknown struct constructor {}", segs.join("::")))),
    }
}

fn wrapper_new(segs: &[String]) -> Option<Wrapper> {
    match segs {
        [name, new] if new == "new" => match name.as_str() {
            "Box" => Some(Wrapper::Box),
            "Arc" => Some(Wrapper::Arc),
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
    let args = args.into_iter().map(|a| lower_expr(cx, a)).collect::<Result<Vec<_>, _>>()?;
    match func {
        SynExpr::Path(p) => {
            let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
            if segs == ["Mutex", "new"] {
                return Err(ParseError::new(
                    Reason::Mutex,
                    "`Mutex::new` is shared mutable state; the subset is pure functions (design/02 §3.7)",
                )
                .detail("Mutex"));
            }
            if let Some(wrapper) = wrapper_new(&segs) {
                if args.len() != 1 {
                    return Err(ParseError::new(
                        Reason::ConstructShape,
                        format!("`{}::new` takes 1 argument, got {}", wrapper.rust_name(), args.len()),
                    ));
                }
                return Ok(Expr::Ignored { wrapper, expr: Box::new(args.into_iter().next().unwrap()) });
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
            } else if segs == ["String", "new"] {
                Callee::StringNew
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
                Callee::Variant { ty: Name::new(segs[0].clone()), variant: Name::new(segs[1].clone()) }
            } else if segs.len() == 2 && (cx.is_enum(&segs[0]) || cx.is_struct(&segs[0])) {
                Callee::Method { ty: Name::new(segs[0].clone()), name: Name::new(segs[1].clone()) }
            } else if segs.len() == 1 {
                Callee::Fn(Name::new(segs[0].clone()))
            } else {
                return Err(path_error(&segs, format!("unsupported call {}", segs.join("::"))));
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

fn lower_lit(lit: &syn::Lit) -> Result<Lit, ParseError> {
    match lit {
        syn::Lit::Bool(b) => Ok(Lit::Bool(b.value)),
        syn::Lit::Int(i) => {
            let suffix = i.suffix();
            if let Some(Some(ty)) = float_suffix(suffix) {
                return Ok(Lit::Float { digits: i.base10_digits().to_string(), ty: Some(ty) });
            }
            let ty = match suffix {
                "" => None,
                s => Some(IntTy::of_suffix(s).ok_or_else(|| {
                    ParseError::new(Reason::LiteralSuffix, format!("integer suffix `{s}` is not in v0"))
                })?),
            };
            let value =
                i.base10_parse::<i128>().map_err(|e| ParseError::new(Reason::UnsupportedLiteral, e.to_string()))?;
            let hex = i.to_string().starts_with("0x") || i.to_string().starts_with("0X");
            Ok(Lit::Int { value, ty, byte: false, hex })
        }
        syn::Lit::Float(f) => match float_suffix(f.suffix()) {
            Some(ty) => Ok(Lit::Float { digits: f.base10_digits().to_string(), ty }),
            None => Err(ParseError::new(Reason::LiteralSuffix, format!("float suffix `{}` is not in v0", f.suffix()))),
        },
        syn::Lit::Str(s) => Ok(Lit::Str(s.value())),
        syn::Lit::Char(c) => Ok(Lit::Char(c.value())),
        // `b'@'` is a `u8`.
        syn::Lit::Byte(b) => Ok(Lit::Int { value: i128::from(b.value()), ty: Some(IntTy::U8), byte: true, hex: false }),
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
            return Err(ParseError::new(Reason::UnsupportedOperator, format!("operator {text} is not in v0"))
                .detail(text.trim_matches('`')));
        }
    })
}

fn lower_un(op: SynUnOp) -> Result<UnOp, ParseError> {
    match op {
        SynUnOp::Not(_) => Ok(UnOp::Not),
        SynUnOp::Neg(_) => Ok(UnOp::Neg),
        _ => {
            let text = snippet(&op);
            Err(ParseError::new(Reason::UnsupportedOperator, format!("operator {text} is not in v0"))
                .detail(text.trim_matches('`')))
        }
    }
}

fn path_text(path: &syn::Path) -> String {
    path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::")
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
    let reason = if segs.first().is_some_and(|s| s == "Self") { Reason::SelfType } else { Reason::ExternalPath };
    ParseError::new(reason, message).detail(segs.join("::"))
}
