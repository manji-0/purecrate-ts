//! Expressions, literals, arrows, and types, printed as TS source.

use super::*;

/// `indent` is the nesting level of the line the arrow starts on.
pub(crate) fn fn_arrow(f: &Fn, indent: usize) -> String {
    let params = f
        .params
        .iter()
        .map(|p| format!("{}: {}", p.name.as_str(), emit_ty(&p.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    arrow(&params, &emit_ty(&f.ret), &f.body, indent)
}

pub(crate) fn closure_arrow(
    params: &[ClosureParam],
    ret: Option<&Ty>,
    body: &Expr,
    indent: usize,
) -> String {
    let typed = |name: &Name, ty: Option<&Ty>| match ty {
        Some(t) => format!("{}: {}", name.as_str(), emit_ty(t)),
        None => name.as_str().to_string(),
    };
    let params = params
        .iter()
        .map(|p| typed(&p.name, p.ty.as_ref()))
        .collect::<Vec<_>>()
        .join(", ");
    match ret {
        Some(r) => arrow(&params, &emit_ty(r), body, indent),
        None => format!("({params}) => {}", arrow_expr(&crate::join::joined(body), indent)),
    }
}

pub(crate) fn arrow(params: &str, ret: &str, body: &Expr, indent: usize) -> String {
    let body = &crate::join::joined(body);
    // `String::from(x)` prints as `x`: a `match` under it is the body itself.
    let body = match body {
        Expr::Call {
            callee: Callee::StringFrom,
            args,
        } if args[0].needs_statements() => &args[0],
        other => other,
    };
    // A comment above the body's value opens a block to stand in.
    let commented = matches!(body, Expr::Seq { first, .. } if matches!(**first, Expr::Comment(_)));
    if body.needs_statements() || commented {
        let mut out = String::new();
        emit_stmts(body, indent + 1, Sink::Return, &mut out);
        // A body that is one `return` on one line is the arrow's expression.
        if let Some(value) = out
            .trim()
            .strip_prefix("return ")
            .and_then(|v| v.strip_suffix(';'))
            .filter(|_| out.trim_end().lines().count() == 1)
        {
            // An object would read as a block; a `?:` is parenthesized on the
            // arrow's line, as oxfmt prints it (`tidy::wrap_arrow` drops it).
            let value = if value.starts_with('{') || crate::tidy::is_ternary(value) {
                format!("({value})")
            } else {
                value.to_string()
            };
            return format!("({params}): {ret} => {value}");
        }
        format!(
            "({params}): {ret} => {{\n{out}{pad}}}",
            pad = "  ".repeat(indent)
        )
    } else {
        format!("({params}): {ret} => {}", arrow_expr(body, indent))
    }
}

/// An arrow body starting with `{` would parse as a block.
pub(crate) fn arrow_expr(expr: &Expr, indent: usize) -> String {
    let s = emit_expr(expr, indent);
    if leads_with_brace(expr) {
        format!("({s})")
    } else {
        // Parentheses around all of it, unless they keep an object literal
        // from reading as a block.
        let bare = crate::tidy::strip_outer(&s);
        if bare.starts_with('{') {
            s
        } else if crate::tidy::is_ternary(bare) {
            // Parenthesized on the arrow's line, as oxfmt prints it;
            // `tidy::wrap_arrow` drops the pair when the body moves down.
            format!("({bare})")
        } else if let Some(rest) = bare.strip_prefix("({") {
            // An object heading a longer body (`({ .. } satisfies T)[k]`)
            // is parenthesized itself, as oxfmt prints it.
            let close = rest.find("} ").map_or(rest.len(), |c| c + 1);
            format!("(({{{}){}", &rest[..close], &rest[close..])
        } else {
            bare.to_string()
        }
    }
}

/// Whether `emit_expr` prints an object literal first, possibly after an
/// erased wrapper's comment. At the start of an arrow body or a statement it
/// would parse as a block, so the caller parenthesizes the whole expression.
pub(crate) fn leads_with_brace(expr: &Expr) -> bool {
    match expr {
        Expr::Ignored { expr, .. } => leads_with_brace(expr),
        Expr::Construct {
            ty,
            variant,
            fields,
            base: None,
        } => {
            variant.is_some()
                || !(matches!(fields, Fields::Positional(_)) || is_closed(ty.as_str()))
        }
        Expr::Field { base, .. } => leads_with_brace(base),
        Expr::Binary { left, .. } | Expr::If { cond: left, .. } => leads_with_brace(left),
        Expr::Call {
            callee: Callee::OptionSome | Callee::StringFrom,
            args,
        } => leads_with_brace(&args[0]),
        Expr::Call {
            callee: Callee::IntFrom { from, to },
            args,
        } if *from == Some(*to) => leads_with_brace(&args[0]),
        _ => false,
    }
}

pub(crate) fn emit_ty(ty: &Ty) -> String {
    match ty {
        Ty::Prim(p) => match p {
            purecrate_ir::Prim::Bool => "boolean".into(),
            purecrate_ir::Prim::String | purecrate_ir::Prim::Str => "string".into(),
            purecrate_ir::Prim::Char => "Char".into(),
            purecrate_ir::Prim::Uuid => "Uuid".into(),
            purecrate_ir::Prim::UuidError => "UuidError".into(),
            purecrate_ir::Prim::ParseIntError => "ParseIntError".into(),
            purecrate_ir::Prim::Unit => "undefined".into(),
            other => match other.int() {
                Some(t) => t.ts_name().into(),
                None => match other.float() {
                    Some(t) => t.ts_name().into(),
                    None => "number".into(),
                },
            },
        },
        Ty::Option(inner) => format!("{} | null", emit_ty(inner)),
        Ty::Result { ok, err } => format!("Result<{}, {}>", emit_ty(ok), emit_ty(err)),
        Ty::Vec(inner) => format!("ReadonlyArray<{}>", emit_ty(inner)),
        Ty::Tuple(elems) => {
            let inner = elems.iter().map(emit_ty).collect::<Vec<_>>().join(", ");
            format!("readonly [{inner}]")
        }
        Ty::Named(n) => n.as_str().to_string(),
        // The type says what Rust wrapped (design/03 §2 says why it is gone);
        // a `Box::new(x)` in an expression is just `x`.
        Ty::Ignored { wrapper, inner } => {
            format!("/* {} */ {}", wrapper.rust_name(), emit_ty(inner))
        }
        Ty::Fn { params, ret } => {
            let params = params
                .iter()
                .enumerate()
                .map(|(i, t)| format!("_{i}: {}", emit_ty(t)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("(({params}) => {})", emit_ty(ret))
        }
        Ty::Never => "never".into(),
    }
}

pub(crate) fn emit_expr(expr: &Expr, indent: usize) -> String {
    let expr = peel_identity(expr);
    match expr {
        Expr::At { .. } => {
            unreachable!("emit takes `check::accept` output, which has no positions")
        }
        Expr::Ignored { expr, .. } => emit_expr(expr, indent),
        // An expression has no line of its own to carry a comment.
        Expr::Seq { first, then } if matches!(**first, Expr::Comment(_)) => emit_expr(then, indent),
        Expr::Comment(_) => unreachable!("a comment stands only as `first` of a `Seq`"),
        Expr::Lit(lit) => emit_lit(lit),
        Expr::Var(n) => n.as_str().to_string(),
        Expr::Field { base, name } if name.as_str() == NEWTYPE_FIELD => emit_expr(base, indent),
        Expr::Field { base, name } if name.as_str().starts_with('[') => {
            format!("{}{}", emit_expr(base, indent), name.as_str())
        }
        Expr::Field { base, name } => format!("{}.{n}", emit_expr(base, indent), n = name.as_str()),
        // `Slice.at` takes a plain `number`: a literal index needs no brand.
        Expr::Index { base, index } => format!(
            "Slice.at({}, {})",
            emit_expr(base, indent),
            match peel_identity(index) {
                Expr::Lit(lit @ Lit::Int { .. }) => bare_lit(lit),
                _ => emit_item(index, indent),
            }
        ),
        Expr::Binary { op, left, right } => {
            let p = bin_prec(*op);
            // A comparison reads a literal or a length bare: `<` and `===`
            // compare the values, and a brand does not change them.
            if matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne) {
                let (l, r) = (bare(left, indent), bare(right, indent));
                if l.is_some() || r.is_some() {
                    let l = l.unwrap_or_else(|| grouped(left, indent, p, crate::tidy::Assoc::Left, crate::tidy::Side::Left));
                    let r = r.unwrap_or_else(|| grouped(right, indent, p, crate::tidy::Assoc::Left, crate::tidy::Side::Right));
                    return format!("{l} {} {r}", bin_op(*op));
                }
            }
            format!(
                "{} {} {}",
                grouped(
                    left,
                    indent,
                    p,
                    crate::tidy::Assoc::Left,
                    crate::tidy::Side::Left
                ),
                bin_op(*op),
                grouped(
                    right,
                    indent,
                    p,
                    crate::tidy::Assoc::Left,
                    crate::tidy::Side::Right
                )
            )
        }
        Expr::Unary { op, expr } => {
            let o = match op {
                purecrate_ir::UnOp::Not => "!",
                purecrate_ir::UnOp::Neg => "-",
            };
            let inner = grouped(
                expr,
                indent,
                crate::tidy::PREC_UNARY,
                crate::tidy::Assoc::Right,
                crate::tidy::Side::Right,
            );
            if *op == purecrate_ir::UnOp::Not {
                if let Some(flipped) = crate::tidy::negate(&inner) {
                    return flipped;
                }
            }
            if inner.starts_with(o) {
                format!("{o}({inner})")
            } else {
                format!("{o}{inner}")
            }
        }
        // `|x| f(x)`, which a function name passed to `map` or `all` becomes,
        // is `f` itself: its parameter and return types are the arrow's.
        Expr::Closure { params, body, .. } if forwards(params, body).is_some() => {
            forwards(params, body).expect("checked above").to_string()
        }
        Expr::Closure { params, ret, body } => {
            format!("({})", closure_arrow(params, ret.as_ref(), body, indent))
        }
        Expr::MethodCall { name, .. } => {
            unreachable!(
                "`.{}()` reaches emit unresolved; emit takes `check::accept` output",
                name.as_str()
            )
        }
        Expr::Construct {
            ty,
            variant,
            fields,
            base,
        } => match (variant, base) {
            (Some(v), None) => emit_variant_value(ty.as_str(), v.as_str(), fields),
            (None, None) if is_closed(ty.as_str()) => {
                format!(
                    "{}({})",
                    closed_ctor(ty.as_str()),
                    emit_struct_value(fields)
                )
            }
            (None, None) => emit_struct_value(fields),
            (None, Some(base)) if is_closed(ty.as_str()) => {
                format!(
                    "{}{}",
                    closed_ctor(ty.as_str()),
                    emit_struct_update(fields, base)
                )
            }
            (None, Some(base)) => emit_struct_update(fields, base),
            (Some(_), Some(_)) => unreachable!("enum variants have no struct update"),
        },
        Expr::Match { .. } | Expr::Let { .. } => {
            as_expr(expr, indent).unwrap_or_else(|| emit_iife(expr, indent))
        }
        Expr::If { .. } if expr.needs_statements() => {
            as_expr(expr, indent).unwrap_or_else(|| emit_iife(expr, indent))
        }
        Expr::Try { .. }
        | Expr::Seq { .. }
        | Expr::Assign { .. }
        | Expr::For { .. }
        | Expr::ForEach { .. }
        | Expr::While { .. } => emit_iife(expr, indent),
        Expr::Break | Expr::Continue => {
            unreachable!("`check::accept` keeps `break` and `continue` in statement position")
        }
        Expr::If { cond, then, else_ } => format!(
            "{} ? {} : {}",
            grouped(
                cond,
                indent,
                crate::tidy::PREC_TERNARY,
                crate::tidy::Assoc::Right,
                crate::tidy::Side::Left
            ),
            grouped(
                then,
                indent,
                crate::tidy::PREC_TERNARY,
                crate::tidy::Assoc::Right,
                crate::tidy::Side::Left
            ),
            grouped(
                else_,
                indent,
                crate::tidy::PREC_TERNARY,
                crate::tidy::Assoc::Right,
                crate::tidy::Side::Right
            )
        ),
        // Variant names are identifiers other than `__proto__` (checked), so
        // an object literal is a plain table.
        Expr::Call {
            callee: purecrate_ir::Callee::Discriminant { to, table, of },
            args,
        } => match typed_table(table, of, to.is_big(), &args[0], indent) {
            Some(lookup) => format!("({lookup} as {})", to.ts_name()),
            None => {
                // `kind` may be typed `string` here (a variant literal
                // widens), and a consumer may set `noUncheckedIndexedAccess`:
                // hence both casts.
                let t = to.ts_name();
                let entries = table
                    .iter()
                    .map(|(v, d)| {
                        format!(
                            "{}: {}",
                            v.as_str(),
                            crate::tidy::strip_outer(&emit_lit(&Lit::Int {
                                value: *d,
                                ty: Some(*to)
                            }))
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("(({{ {entries} }} as Record<string, {t}>)[{}.kind] as {t})", emit_expr(&args[0], indent))
            }
        },
        Expr::Call { callee, args } => {
            if let purecrate_ir::Callee::Consume { method, over } = callee {
                let source = iterable(*over, emit_expr(&args[0], indent));
                return match method {
                    purecrate_ir::Consume::Count => format!("Iter.count({source})"),
                    purecrate_ir::Consume::Sum(int) => format!(
                        "Iter.sum({source}, Int.{}.add, {})",
                        int.as_str(),
                        crate::tidy::strip_outer(&emit_lit(&Lit::Int {
                            value: 0,
                            ty: Some(*int)
                        }))
                    ),
                    m => format!(
                        "Iter.{}({source}, {})",
                        m.ts_name(),
                        emit_item(&args[1], indent)
                    ),
                };
            }
            let c = match callee {
                purecrate_ir::Callee::Fn(n) | purecrate_ir::Callee::Local(n) => {
                    n.as_str().to_string()
                }
                purecrate_ir::Callee::Method { ty, name }
                    if is_private_method(ty.as_str(), name.as_str()) =>
                {
                    private_method(ty.as_str(), name.as_str())
                }
                purecrate_ir::Callee::Method { ty, name } => {
                    format!("{}.{}", ty.as_str(), name.as_str())
                }
                purecrate_ir::Callee::Variant { ty, variant } => {
                    format!("{}.{}", ty.as_str(), variant.as_str())
                }
                purecrate_ir::Callee::StructNew(n) if is_closed(n.as_str()) => {
                    closed_ctor(n.as_str())
                }
                purecrate_ir::Callee::StructNew(n) => format!("{}.of", n.as_str()),
                purecrate_ir::Callee::ResultOk => "Result.ok".into(),
                purecrate_ir::Callee::ResultErr => "Result.err".into(),
                purecrate_ir::Callee::OptionSome => String::new(),
                purecrate_ir::Callee::OptionNone => "null".into(),
                purecrate_ir::Callee::Int { ty, op } => {
                    format!("Int.{}.{}", ty.as_str(), op.as_str())
                }
                purecrate_ir::Callee::Discriminant { .. } => unreachable!("printed above"),
                purecrate_ir::Callee::Fround => "globalThis.Math.fround".into(),
                purecrate_ir::Callee::AsFloat(_) => String::new(),
                purecrate_ir::Callee::VecLen
                | purecrate_ir::Callee::VecIsEmpty
                | purecrate_ir::Callee::OptionIsSome
                | purecrate_ir::Callee::OptionIsNone
                | purecrate_ir::Callee::StrBytes
                | purecrate_ir::Callee::StrSplit
                | purecrate_ir::Callee::Collect { .. }
                | purecrate_ir::Callee::StringFrom
                | purecrate_ir::Callee::Slice { .. }
                | purecrate_ir::Callee::Str(_)
                | purecrate_ir::Callee::IntFrom { .. }
                | purecrate_ir::Callee::CharCode(_) => String::new(),
                purecrate_ir::Callee::CharFromU8 => "Char.fromU8".into(),
                purecrate_ir::Callee::CharFromU32 => "Char.fromU32".into(),
                purecrate_ir::Callee::Char(m) => format!("Char.{}", m.ts_name()),
                purecrate_ir::Callee::UuidParse => "Uuid.parseStr".into(),
                purecrate_ir::Callee::UuidNil => "Uuid.nil".into(),
                purecrate_ir::Callee::StrParse(t) => format!("Int.{}.parse", t.as_str()),
                purecrate_ir::Callee::StrCmp => "Str.cmp".into(),
                purecrate_ir::Callee::OrdCmp { text: false } => "Ord.cmp".into(),
                purecrate_ir::Callee::OrdCmp { text: true } => "Ord.cmpStr".into(),
                purecrate_ir::Callee::OrdThen => "Ord.then".into(),
                purecrate_ir::Callee::Consume { .. } => unreachable!("printed above"),
            };
            if let purecrate_ir::Callee::CharCode(to) = callee {
                let code = format!("Char.code({})", emit_expr(&args[0], indent));
                return if to.is_big() {
                    format!("(globalThis.BigInt({code}) as {})", to.ts_name())
                } else {
                    code
                };
            }
            if let purecrate_ir::Callee::IntFrom { from, to } = callee {
                // A discriminant widened: the table's number is cast once.
                if let Expr::Call { callee: purecrate_ir::Callee::Discriminant { table, of, .. }, args: inner } =
                    peel_identity(&args[0])
                {
                    if let (false, Some(lookup)) = (to.is_big(), typed_table(table, of, false, &inner[0], indent)) {
                        return format!("({lookup} as {})", to.ts_name());
                    }
                }
                let x = emit_expr(&args[0], indent);
                let from = from.expect("check::accept sets the source width");
                return match (from.is_big(), to.is_big()) {
                    _ if from == *to => x,
                    (false, true) => format!("(globalThis.BigInt({x}) as {})", to.ts_name()),
                    _ => format!("({} as number as {})", cast_operand(&x), to.ts_name()),
                };
            }
            if matches!(callee, purecrate_ir::Callee::StrBytes) {
                return format!("Str.bytes({})", emit_expr(&args[0], indent));
            }
            if let purecrate_ir::Callee::Slice { of, start, end } = callee {
                return emit_slice(
                    of.expect("check::accept sets what is sliced"),
                    *start,
                    *end,
                    args,
                    indent,
                );
            }
            if matches!(callee, purecrate_ir::Callee::StrSplit) {
                // JS `split` takes a string; the `char` needs no brand.
                return format!(
                    "{}.split({})",
                    emit_expr(&args[0], indent),
                    bare(&args[1], indent).unwrap_or_else(|| emit_expr(&args[1], indent))
                );
            }
            if let purecrate_ir::Callee::Collect { result } = callee {
                let pieces = emit_expr(&args[0], indent);
                return match (args.get(1), result) {
                    (None, _) => pieces,
                    (Some(f), false) => format!("{pieces}.map({})", emit_item(f, indent)),
                    (Some(f), true) => {
                        format!("Iter.tryCollect({pieces}, {})", emit_item(f, indent))
                    }
                };
            }
            if let purecrate_ir::Callee::Str(m) = callee {
                let s = emit_expr(&args[0], indent);
                let needle = || bare(&args[1], indent).unwrap_or_else(|| emit_item(&args[1], indent));
                return match m {
                    purecrate_ir::StrMethod::Len => format!("Str.len({s})"),
                    purecrate_ir::StrMethod::IsEmpty => format!("{s}.length === 0"),
                    purecrate_ir::StrMethod::StartsWith => format!("{s}.startsWith({})", needle()),
                    purecrate_ir::StrMethod::EndsWith => format!("{s}.endsWith({})", needle()),
                    purecrate_ir::StrMethod::Contains => format!("{s}.includes({})", needle()),
                    purecrate_ir::StrMethod::AsStr => s,
                    purecrate_ir::StrMethod::StripPrefix => {
                        format!("Str.stripPrefix({s}, {})", needle())
                    }
                    purecrate_ir::StrMethod::StripSuffix => {
                        format!("Str.stripSuffix({s}, {})", needle())
                    }
                    purecrate_ir::StrMethod::SplitOnce => {
                        format!("Str.splitOnce({s}, {})", needle())
                    }
                };
            }
            if matches!(callee, purecrate_ir::Callee::VecLen) {
                return format!("({}.length as Usize)", emit_expr(&args[0], indent));
            }
            match callee {
                purecrate_ir::Callee::VecIsEmpty => {
                    return format!("{}.length === 0", emit_expr(&args[0], indent))
                }
                purecrate_ir::Callee::OptionIsSome => {
                    return format!("{} !== null", emit_expr(&args[0], indent))
                }
                purecrate_ir::Callee::OptionIsNone => {
                    return format!("{} === null", emit_expr(&args[0], indent))
                }
                _ => {}
            }
            if matches!(callee, purecrate_ir::Callee::Fround) {
                return format!(
                    "(globalThis.Math.fround({}) as F32)",
                    emit_expr(&args[0], indent)
                );
            }
            if let purecrate_ir::Callee::AsFloat(ft) = callee {
                return format!("({} as {})", cast_operand(&emit_expr(&args[0], indent)), ft.ts_name());
            }
            if matches!(callee, purecrate_ir::Callee::OptionNone) {
                return "null".into();
            }
            if matches!(
                callee,
                purecrate_ir::Callee::OptionSome | purecrate_ir::Callee::StringFrom
            ) {
                return emit_expr(&args[0], indent);
            }
            // A shift amount is a plain `number` in the runtime: a literal
            // there needs no brand.
            let shift = matches!(callee, purecrate_ir::Callee::Int { op, .. } if op.is_shift());
            let a = args
                .iter()
                .enumerate()
                .map(|(i, e)| match (shift && i == 1, peel_identity(e)) {
                    (true, Expr::Lit(lit @ Lit::Int { .. })) => bare_lit(lit),
                    _ => emit_item(e, indent),
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{c}({a})")
        }
        Expr::Tuple(elems) | Expr::Array(elems) => {
            let items: Vec<String> = elems.iter().map(|e| emit_item(e, indent)).collect();
            bracket_list(&items, indent)
        }
        Expr::Return(e) => format!("(() => {{ return {}; }})()", emit_expr(e, indent)),
        Expr::Unreachable => "assertNever(undefined as never)".into(),
        Expr::Cast { .. } => unreachable!("`check::accept` rewrites `as`"),
    }
}

/// Print `expr` as an operand of a parent of `parent` precedence.
fn grouped(
    expr: &Expr,
    indent: usize,
    parent: u8,
    assoc: crate::tidy::Assoc,
    side: crate::tidy::Side,
) -> String {
    let s = emit_expr(expr, indent);
    // A `match` or `if` may print as a test (`k.kind === "A"`): what it
    // printed decides, as `tidy::group` reads it.
    if matches!(peel_identity(expr), Expr::If { .. } | Expr::Match { .. }) && !crate::tidy::has_top_as(&s) {
        return crate::tidy::group(&s, parent, assoc, side);
    }
    if crate::tidy::needs_paren(expr_prec(expr), parent, assoc, side) {
        format!("({s})")
    } else {
        s
    }
}

fn bin_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Mul | BinOp::Div | BinOp::Rem => crate::tidy::PREC_MUL,
        BinOp::Add | BinOp::Sub => crate::tidy::PREC_ADD,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => crate::tidy::PREC_REL,
        BinOp::Eq | BinOp::Ne => crate::tidy::PREC_EQ,
        BinOp::And => crate::tidy::PREC_AND,
        BinOp::Or => crate::tidy::PREC_OR,
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
            unreachable!("`check::accept` rewrites bitwise operators into `Int` calls")
        }
    }
}

fn expr_prec(expr: &Expr) -> u8 {
    match peel_identity(expr) {
        Expr::Binary { op, .. } => bin_prec(*op),
        Expr::Unary { .. } => crate::tidy::PREC_UNARY,
        // Printed as a comparison (`s.length === 0`, `o !== null`).
        Expr::Call {
            callee:
                purecrate_ir::Callee::Str(purecrate_ir::StrMethod::IsEmpty)
                | purecrate_ir::Callee::VecIsEmpty
                | purecrate_ir::Callee::OptionIsSome
                | purecrate_ir::Callee::OptionIsNone,
            ..
        } => crate::tidy::PREC_EQ,
        Expr::If { .. } | Expr::Match { .. } | Expr::Let { .. } => crate::tidy::PREC_TERNARY,
        _ => crate::tidy::PREC_ATOMIC,
    }
}

/// `eDiscriminants[e.kind]`, a `number` (a `bigint` for a 64-bit width),
/// where `e` is a place or a call: it has the enum's type, so its `kind` is
/// one of the table's keys, and the lookup is a number even under
/// `noUncheckedIndexedAccess`. The table is a `const` of the file, checked
/// against the enum's variants (`satisfies`), printed after its imports.
/// `None` for a value whose `kind` may have widened to `string`.
fn typed_table(table: &[(Name, i128)], of: &Name, big: bool, subject: &Expr, indent: usize) -> Option<String> {
    if !(is_place(subject) || matches!(peel_identity(subject), Expr::Call { .. })) {
        return None;
    }
    let key = (of.as_str().to_string(), big);
    let name = TABLES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some((_, name, _)) = t.iter().find(|(k, ..)| *k == key) {
            return name.clone();
        }
        let base = format!("{}Discriminants{}", lower_initial(of.as_str()), if big && t.iter().any(|(k, ..)| k.0 == key.0) { "Big" } else { "" });
        let name = temp(&base, 0);
        let (n, num) = if big { ("n", "bigint") } else { ("", "number") };
        let entries = table.iter().map(|(v, d)| format!("{}: {d}{n}", v.as_str())).collect::<Vec<_>>().join(", ");
        let decl = format!(
            "/** Each `{of}` variant's discriminant. */\nconst {name} = {{ {entries} }} satisfies Record<{of}[\"kind\"], {num}>;\n",
            of = of.as_str()
        );
        t.push((key, name.clone(), decl));
        name
    });
    Some(format!("{name}[{}.kind]", emit_expr(subject, indent)))
}

fn lower_initial(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().chain(c).collect()).unwrap_or_default()
}

/// `{ ...base, a: e1 }`. `?` in the fields and the base is hoisted before this
/// expression, fields first, so the spread running first in JS does not move
/// a failing `?` ahead of an earlier one.
pub(crate) fn emit_struct_update(fields: &Fields, base: &Expr) -> String {
    let Fields::Named(pairs) = fields else {
        unreachable!("struct update has named fields");
    };
    let mut parts = vec![format!("...{}", emit_expr(base, 0))];
    for (name, expr) in pairs {
        parts.push(field_pair(name.as_str(), emit_item(expr, 0)));
    }
    format!("({{ {} }})", parts.join(", "))
}

/// `name: value`, or `name` alone when the value is a variable of that name.
fn field_pair(name: &str, value: String) -> String {
    if value == name {
        value
    } else {
        format!("{name}: {value}")
    }
}

pub(crate) fn emit_struct_value(fields: &Fields) -> String {
    match fields {
        Fields::Unit => "{}".into(),
        Fields::Positional(elems) => {
            let inner = elems
                .iter()
                .map(|e| emit_item(e, 0))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        Fields::Named(pairs) => {
            let inner = pairs
                .iter()
                .map(|(n, e)| field_pair(n.as_str(), emit_item(e, 0)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {inner} }}")
        }
    }
}

pub(crate) fn emit_variant_value(_ty: &str, variant: &str, fields: &Fields) -> String {
    match fields {
        Fields::Unit => format!("{{ kind: \"{variant}\" }}"),
        Fields::Positional(elems) if elems.len() == 1 => {
            let value = emit_item(&elems[0], 0);
            if value == "value" {
                format!("{{ kind: \"{variant}\", value }}")
            } else {
                format!("{{ kind: \"{variant}\", value: {value} }}")
            }
        }
        Fields::Positional(elems) => {
            let inner = elems
                .iter()
                .map(|e| emit_item(e, 0))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ kind: \"{variant}\", content: [{inner}] }}")
        }
        Fields::Named(pairs) => {
            let inner = pairs
                .iter()
                .map(|(n, e)| field_pair(n.as_str(), emit_item(e, 0)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ kind: \"{variant}\", {inner} }}")
        }
    }
}

/// `expr` as one TS expression, without an inline function, when it can be:
/// a `match` on a place whose arms are expressions, as a chain of `?:` (or
/// `||` / `&&` where the arms are `true` and `false`); an `if` whose
/// branches can be; a `let` of a place or a literal, read where it is
/// used. A place and a literal read the same anywhere in an expression, as
/// nothing in one assigns. `None` otherwise: the inline function stays,
/// which evaluates a scrutinee that is not a place once.
pub(crate) fn as_expr(expr: &Expr, indent: usize) -> Option<String> {
    match expr {
        Expr::Let {
            name,
            mutable: false,
            value,
            then,
            ..
        } if value.is_inlinable() => as_expr(&subst(then, name, value), indent),
        Expr::Match { scrutinee, arms } if is_place(scrutinee) => {
            match_expr(scrutinee, arms, indent)
        }
        Expr::If { cond, then, else_ } => {
            let (t, e) = (as_expr(then, indent)?, as_expr(else_, indent)?);
            Some(fold(
                emit_expr(cond, indent),
                (t, bool_lit(then)),
                (e, bool_lit(else_)),
            ))
        }
        e if !e.needs_statements() => Some(emit_expr(e, indent)),
        _ => None,
    }
}

fn bool_lit(e: &Expr) -> Option<bool> {
    match e {
        Expr::Lit(Lit::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// `!test`, as `a !== b` for `a === b` (`opt === null || opt`).
fn not_test(test: &str) -> String {
    use crate::tidy::{group, Assoc, Side, PREC_UNARY};
    crate::tidy::negate(test).unwrap_or_else(|| format!("!{}", group(test, PREC_UNARY, Assoc::Right, Side::Right)))
}

/// `test ? then : else_`, as `||` / `&&` when either side is a `bool`
/// literal. Operands are parenthesized only when precedence requires it.
fn fold(
    test: String,
    (then, then_lit): (String, Option<bool>),
    (else_, else_lit): (String, Option<bool>),
) -> String {
    use crate::tidy::{group, Assoc, Side, PREC_AND, PREC_OR, PREC_TERNARY, PREC_UNARY};
    match (then_lit, else_lit) {
        (Some(true), Some(false)) => crate::tidy::strip_outer(&test).to_string(),
        (Some(false), Some(true)) => match crate::tidy::negate(&test) {
            Some(flipped) => flipped,
            None => format!("!{}", group(&test, PREC_UNARY, Assoc::Right, Side::Right)),
        },
        (Some(true), _) => format!(
            "{} || {}",
            group(&test, PREC_OR, Assoc::Left, Side::Left),
            group(&else_, PREC_OR, Assoc::Left, Side::Right)
        ),
        (Some(false), _) => format!(
            "{} && {}",
            group(&not_test(&test), PREC_AND, Assoc::Left, Side::Left),
            group(&else_, PREC_AND, Assoc::Left, Side::Right)
        ),
        (_, Some(false)) => format!(
            "{} && {}",
            group(&test, PREC_AND, Assoc::Left, Side::Left),
            group(&then, PREC_AND, Assoc::Left, Side::Right)
        ),
        (_, Some(true)) => format!(
            "{} || {}",
            group(&not_test(&test), PREC_OR, Assoc::Left, Side::Left),
            group(&then, PREC_OR, Assoc::Left, Side::Right)
        ),
        _ => {
            let t = crate::tidy::strip_outer(&then);
            // `unwrap_or` of a name, literal, or field: `x ?? d` is the test
            // `x !== null` with the same `x` in the Some arm (`??` keeps 0/false).
            if test == format!("{t} !== null") {
                // A cast beside `??` is parenthesized, as oxfmt prints it.
                let d = crate::tidy::strip_outer(&else_);
                if crate::tidy::has_top_as(d) { format!("{t} ?? ({d})") } else { format!("{t} ?? {d}") }
            } else {
                // A cast in a branch, and a `??` anywhere in it, is
                // parenthesized, as oxfmt prints it.
                let coalesces = |g: &str| crate::tidy::top_prec(g) == PREC_OR && g.contains(" ?? ");
                let branch = |s: &str, side| {
                    let g = group(s, PREC_TERNARY, Assoc::Right, side);
                    if crate::tidy::has_top_as(&g) || coalesces(&g) { format!("({g})") } else { g }
                };
                let test = group(&test, PREC_TERNARY, Assoc::Right, Side::Left);
                let test = if coalesces(&test) { format!("({test})") } else { test };
                format!(
                    "{} ? {} : {}",
                    test,
                    // A `?:` in the middle is parenthesized, as oxfmt prints it
                    // on one line; `tidy::wrap` drops the pair when it splits.
                    branch(&then, Side::Left),
                    branch(&else_, Side::Right)
                )
            }
        }
    }
}

/// Replace each name `p` binds with the place it reads. A tuple of names
/// reads `read[i]`. `None` when `p` binds something else.
pub(crate) fn bind_in(p: &Pattern, read: Expr, body: &mut Expr) -> Option<()> {
    match p {
        Pattern::Var(n) => {
            *body = subst(body, n, &read);
            Some(())
        }
        Pattern::Wildcard => Some(()),
        Pattern::Tuple(ps) => {
            for (i, e) in ps.iter().enumerate() {
                bind_in(
                    e,
                    Expr::Field {
                        base: Box::new(read.clone()),
                        name: Name::new(format!("[{i}]")),
                    },
                    body,
                )?;
            }
            Some(())
        }
        _ => None,
    }
}

/// The arms in order, each a test on `scrutinee` and its body with the
/// pattern's bindings read from `scrutinee`; the last is the `else`, as
/// `check::accept` has made the `match` exhaustive.
fn match_expr(scrutinee: &Expr, arms: &[purecrate_ir::Arm], indent: usize) -> Option<String> {
    if arms.iter().any(|a| a.guard.is_some()) {
        return None;
    }
    let subject = emit_expr(scrutinee, indent);
    let field = |base: &Expr, name: &str| Expr::Field {
        base: Box::new(base.clone()),
        name: Name::new(name),
    };
    let mut parts: Vec<(Option<String>, String, Option<bool>)> = Vec::new();
    for (i, arm) in arms.iter().enumerate() {
        let last = i + 1 == arms.len();
        let mut body = arm.body.clone();
        let bind = bind_in;
        let test = match &arm.pattern {
            Pattern::Wildcard => None,
            Pattern::Var(n) => {
                body = subst(&body, n, scrutinee);
                None
            }
            Pattern::OptionSome(p) => {
                bind(p, scrutinee.clone(), &mut body)?;
                two_way_test(&arm.pattern, &subject)
            }
            Pattern::ResultOk(p) => {
                bind(p, field(scrutinee, "value"), &mut body)?;
                two_way_test(&arm.pattern, &subject)
            }
            Pattern::ResultErr(p) => {
                bind(p, field(scrutinee, "error"), &mut body)?;
                two_way_test(&arm.pattern, &subject)
            }
            Pattern::OptionNone => two_way_test(&arm.pattern, &subject),
            Pattern::Variant {
                variant, bind: vb, ..
            } => {
                match vb {
                    VariantBind::Unit => {}
                    VariantBind::Tuple(ps) => {
                        for (k, p) in ps.iter().enumerate() {
                            let place = if ps.len() == 1 {
                                field(scrutinee, "value")
                            } else {
                                field(&field(scrutinee, "content"), &format!("[{k}]"))
                            };
                            bind(p, place, &mut body)?;
                        }
                    }
                    VariantBind::Struct(ps) => {
                        for (f, p) in ps {
                            bind(p, field(scrutinee, f.as_str()), &mut body)?;
                        }
                    }
                }
                Some(format!("{subject}.kind === \"{}\"", variant.as_str()))
            }
            // Variants that bind nothing (`Cons(_, _)` counts).
            Pattern::Or(alts)
                if alts
                    .iter()
                    .all(|a| matches!(a, Pattern::Variant { .. }) && a.bindings().is_empty()) =>
            {
                Some(
                    alts.iter()
                        .filter_map(|a| match a {
                            Pattern::Variant { variant, .. } => {
                                Some(format!("{subject}.kind === \"{}\"", variant.as_str()))
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" || "),
                )
            }
            p if p.is_lit_case() => Some(lit_test(p, &subject)?),
            _ => return None,
        };
        let lit = bool_lit(&body);
        let text = as_expr(&body, indent)?;
        parts.push((if last { None } else { test }, text, lit));
    }
    let (_, mut acc, mut acc_lit) = parts.pop()?;
    while let Some((test, text, lit)) = parts.pop() {
        let test = test?;
        acc = fold(test, (text, lit), (acc, acc_lit));
        acc_lit = None;
    }
    Some(acc)
}

/// `expr` with each read of `name` replaced by `with`. Nested binders of
/// `name` hide it, so a reused name in another arm is not rewritten.
pub(crate) fn subst(expr: &Expr, name: &Name, with: &Expr) -> Expr {
    let mut out = expr.clone();
    fn go(e: &mut Expr, name: &Name, with: &Expr) {
        match e {
            Expr::Var(n) if n == name => *e = with.clone(),
            Expr::Let {
                name: n,
                value,
                then,
                ..
            } => {
                go(value, name, with);
                if n != name {
                    go(then, name, with);
                }
            }
            Expr::Match { scrutinee, arms } => {
                go(scrutinee, name, with);
                for arm in arms {
                    let bound = arm.pattern.bindings().iter().any(|b| *b == name);
                    if !bound {
                        if let Some(g) = &mut arm.guard {
                            go(g, name, with);
                        }
                        go(&mut arm.body, name, with);
                    }
                }
            }
            Expr::For {
                var,
                start,
                end,
                body,
                ..
            } => {
                go(start, name, with);
                go(end, name, with);
                if var != name {
                    go(body, name, with);
                }
            }
            Expr::ForEach {
                var, source, body, ..
            } => {
                go(source, name, with);
                if var != name {
                    go(body, name, with);
                }
            }
            Expr::Closure { params, body, .. } => {
                if !params.iter().any(|p| p.name == *name) {
                    go(body, name, with);
                }
            }
            _ => e.children_mut().into_iter().for_each(|c| go(c, name, with)),
        }
    }
    go(&mut out, name, with);
    out
}

pub(crate) fn emit_iife(expr: &Expr, indent: usize) -> String {
    let mut body = String::new();
    emit_stmts(expr, indent + 1, Sink::Return, &mut body);
    format!("(() => {{\n{body}{pad}}})()", pad = "  ".repeat(indent))
}

pub(crate) fn emit_lit(lit: &Lit) -> String {
    match lit {
        Lit::Bool(b) => if *b { "true" } else { "false" }.into(),
        Lit::Int { value, ty } => match ty {
            Some(t) if t.is_big() => format!("({value}n as {})", t.ts_name()),
            Some(t) => format!("({value} as {})", t.ts_name()),
            None => value.to_string(),
        },
        Lit::Float { digits, ty } => match ty {
            Some(FloatTy::F32) => f32_literal(digits),
            Some(FloatTy::F64) => format!("({digits} as F64)"),
            None => digits.clone(),
        },
        Lit::Str(s) => js_string(s),
        Lit::Char(c) => format!("({} as Char)", js_string(&c.to_string())),
        Lit::Unit => "undefined".into(),
        Lit::Null => "null".into(),
    }
}

/// Rust rounds the decimal straight to `f32`. `Math.fround(1.0000000596…)`
/// would round it to `f64` first, which can land on a midpoint between two
/// `f32`s and then tie the wrong way. The `f32` value is printed as an exact
/// decimal instead: every `f32` is an `f64`, so JS reads it without rounding.
pub(crate) fn f32_literal(digits: &str) -> String {
    match digits.parse::<f32>() {
        Ok(v) if v.is_finite() => format!("({:?} as F32)", f64::from(v)),
        // rustc rejects an out-of-range literal, so the input never gets here.
        _ => format!("(globalThis.Math.fround({digits}) as F32)"),
    }
}

/// A double-quoted literal escaped as JSON escapes it, plus U+2028 and
/// U+2029, which end a line in older JS parsers.
pub(crate) fn js_string(s: &str) -> String {
    // The quote that needs fewer escapes, double on a tie, as oxfmt picks.
    let q = if s.matches('"').count() > s.matches('\'').count() { '\'' } else { '"' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

pub(crate) fn bin_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::Eq => "===",
        BinOp::Ne => "!==",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
            unreachable!("`check::accept` rewrites bitwise operators into `Int` calls")
        }
    }
}

pub(crate) fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// `&x[a..b]` and its open forms. A string goes through `Str.slice`, which
/// counts UTF-8 bytes; a `Vec` or slice is checked in Rust's order (start,
/// end, then a reversed range) with Rust's messages.
fn emit_slice(
    of: purecrate_ir::SliceOf,
    start: bool,
    end: bool,
    args: &[Expr],
    indent: usize,
) -> String {
    let base = emit_item(&args[0], indent);
    let b = end.then(|| emit_item(&args[args.len() - 1], indent));
    match of {
        purecrate_ir::SliceOf::Str => {
            let a = if start {
                emit_item(&args[1], indent)
            } else {
                "0 as Usize".into()
            };
            match b {
                Some(b) => format!("Str.slice({base}, {a}, {b})"),
                None => format!("Str.slice({base}, {a})"),
            }
        }
        purecrate_ir::SliceOf::Items => {
            let a = if start {
                emit_item(&args[1], indent)
            } else {
                "null".into()
            };
            format!(
                "Slice.range({base}, {a}, {})",
                b.unwrap_or_else(|| "null".into())
            )
        }
    }
}

/// The function a closure only forwards its parameters to, in order.
fn forwards<'e>(params: &[purecrate_ir::ClosureParam], body: &'e Expr) -> Option<&'e str> {
    match body {
        Expr::Call { callee: purecrate_ir::Callee::Fn(f), args }
            if args.len() == params.len()
                && args.iter().zip(params).all(|(a, p)| matches!(a, Expr::Var(n) if *n == p.name))
                && params.iter().all(|p| p.name != *f) =>
        {
            Some(f.as_str())
        }
        _ => None,
    }
}

/// A cast the printer writes around a value (`(1 as I32)`, `(xs.length as
/// Usize)`): one operand, parenthesized for an operator beside it.
fn is_cast(expr: &Expr) -> bool {
    match peel_identity(expr) {
        Expr::Ignored { expr, .. } => is_cast(expr),
        Expr::Lit(Lit::Int { ty: Some(_), .. } | Lit::Float { ty: Some(_), .. } | Lit::Char(_)) => true,
        Expr::Call { callee, .. } => matches!(
            callee,
            purecrate_ir::Callee::VecLen
                | purecrate_ir::Callee::AsFloat(_)
                | purecrate_ir::Callee::Fround
                | purecrate_ir::Callee::IntFrom { .. }
                | purecrate_ir::Callee::Discriminant { .. }
                | purecrate_ir::Callee::CharCode(_)
        ),
        _ => false,
    }
}

/// `[a, b]`; where an element spans lines (a function called at once), one
/// element per line, each one indent in, as oxfmt prints it.
fn bracket_list(items: &[String], indent: usize) -> String {
    if !items.iter().any(|i| i.contains('\n')) {
        return format!("[{}]", items.join(", "));
    }
    let pad = "  ".repeat(indent);
    let body: String = items.iter().map(|i| format!("\n{pad}  {},", i.replace('\n', "\n  "))).collect();
    format!("[{body}\n{pad}]")
}

/// `(test)` after `if`; a test that spans lines (a function called at once)
/// opens the parentheses, one indent in, as oxfmt prints it.
pub(crate) fn if_test(test: &str, pad: &str) -> String {
    if test.contains('\n') {
        format!("(\n{pad}  {}\n{pad})", test.replace('\n', "\n  "))
    } else {
        format!("({test})")
    }
}

/// The operand of `as`: parenthesized where it is a binary expression or a
/// `?:` (`(a / b) as F64`), as oxfmt prints it.
fn cast_operand(s: &str) -> String {
    let s = crate::tidy::strip_outer(s);
    if crate::tidy::top_prec(s) < crate::tidy::PREC_UNARY { format!("({s})") } else { s.to_string() }
}

/// An argument, an element, or a field value: a comma or a brace already
/// bounds it, so a cast or an arrow needs no parentheses of its own
/// (`f(1 as I32)`, `Iter.all(xs, (b: U8): boolean => b > 0)`).
pub(crate) fn emit_item(expr: &Expr, indent: usize) -> String {
    let s = emit_expr(expr, indent);
    if is_cast(expr)
        || matches!(peel_identity(expr), Expr::Closure { .. } | Expr::Construct { base: Some(_), .. })
    {
        crate::tidy::strip_outer(&s).to_string()
    } else {
        s
    }
}

/// A comparison's operand without its brand: an integer or `char` literal
/// (`2`, `"a"`), or a length (`xs.length`). `None` for anything else.
fn bare(expr: &Expr, indent: usize) -> Option<String> {
    match peel_identity(expr) {
        Expr::Ignored { expr, .. } => bare(expr, indent),
        Expr::Lit(lit @ (Lit::Int { ty: Some(_), .. } | Lit::Char(_))) => Some(bare_lit(lit)),
        Expr::Call { callee: purecrate_ir::Callee::VecLen, args } => {
            Some(format!("{}.length", emit_expr(&args[0], indent)))
        }
        _ => None,
    }
}

/// A literal as a comparison reads it: an integer or a `char` without its
/// brand (`48`, `2n`, `"a"`); anything else as `emit_lit` prints it.
pub(crate) fn bare_lit(lit: &Lit) -> String {
    match lit {
        Lit::Int { value, ty: Some(t) } if t.is_big() => format!("{value}n"),
        Lit::Int { value, .. } => value.to_string(),
        Lit::Char(c) => js_string(&c.to_string()),
        other => emit_lit(other),
    }
}
