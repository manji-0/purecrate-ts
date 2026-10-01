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

pub(crate) fn closure_arrow(params: &[ClosureParam], ret: Option<&Ty>, body: &Expr, indent: usize) -> String {
    let typed = |name: &Name, ty: Option<&Ty>| match ty {
        Some(t) => format!("{}: {}", name.as_str(), emit_ty(t)),
        None => name.as_str().to_string(),
    };
    let params = params.iter().map(|p| typed(&p.name, p.ty.as_ref())).collect::<Vec<_>>().join(", ");
    match ret {
        Some(r) => arrow(&params, &emit_ty(r), body, indent),
        None => format!("({params}) => {}", arrow_expr(body, indent)),
    }
}

pub(crate) fn arrow(params: &str, ret: &str, body: &Expr, indent: usize) -> String {
    if body.needs_statements() {
        let mut out = String::new();
        emit_stmts(body, indent + 1, Sink::Return, &mut out);
        // A body that is one `return` on one line is the arrow's expression.
        if let Some(value) = out.trim().strip_prefix("return ").and_then(|v| v.strip_suffix(';')).filter(|_| out.trim_end().lines().count() == 1) {
            let value = if value.starts_with('{') { format!("({value})") } else { value.to_string() };
            return format!("({params}): {ret} => {value}");
        }
        format!("({params}): {ret} => {{\n{out}{pad}}}", pad = "  ".repeat(indent))
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
        s
    }
}

/// Whether `emit_expr` prints an object literal first, possibly after an
/// erased wrapper's comment. At the start of an arrow body or a statement it
/// would parse as a block, so the caller parenthesizes the whole expression.
pub(crate) fn leads_with_brace(expr: &Expr) -> bool {
    match expr {
        Expr::Ignored { expr, .. } => leads_with_brace(expr),
        Expr::Construct { ty, variant, fields, base: None } => {
            variant.is_some() || !(matches!(fields, Fields::Positional(_)) || is_closed(ty.as_str()))
        }
        Expr::Field { base, .. } => leads_with_brace(base),
        Expr::Binary { left, .. } => !matches!(**left, Expr::Binary { .. }) && leads_with_brace(left),
        Expr::Call { callee: Callee::OptionSome | Callee::StringFrom, args } => leads_with_brace(&args[0]),
        Expr::Call { callee: Callee::IntFrom { from, to }, args } if *from == Some(*to) => leads_with_brace(&args[0]),
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
        Ty::Ignored { wrapper, inner } => {
            format!("/* {} */ {}", wrapper.comment(), emit_ty(inner))
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
    match expr {
        Expr::At { .. } => unreachable!("emit takes `check::accept` output, which has no positions"),
        Expr::Ignored { wrapper, expr } => {
            format!("/* {} */ {}", wrapper.comment(), emit_expr(expr, indent))
        }
        Expr::Lit(lit) => emit_lit(lit),
        Expr::Var(n) => n.as_str().to_string(),
        Expr::Field { base, name } if name.as_str() == NEWTYPE_FIELD => emit_expr(base, indent),
        Expr::Field { base, name } if name.as_str().starts_with('[') => format!("{}{}", emit_expr(base, indent), name.as_str()),
        Expr::Field { base, name } => format!("{}.{n}", emit_expr(base, indent), n = name.as_str()),
        Expr::Index { base, index } => format!("Slice.at({}, {})", emit_expr(base, indent), emit_expr(index, indent)),
        Expr::Binary { op, left, right } => format!(
            "{} {} {}",
            operand(left, indent),
            bin_op(*op),
            operand(right, indent)
        ),
        Expr::Unary { op, expr } => {
            let o = match op {
                purecrate_ir::UnOp::Not => "!",
                purecrate_ir::UnOp::Neg => "-",
            };
            let inner = operand(expr, indent);
            if *op == purecrate_ir::UnOp::Not {
                if let Some(flipped) = crate::tidy::negate(&inner) {
                    return format!("({flipped})");
                }
            }
            if inner.starts_with(o) {
                format!("{o}({inner})")
            } else {
                format!("{o}{inner}")
            }
        }
        Expr::Closure { params, ret, body } => format!("({})", closure_arrow(params, ret.as_ref(), body, indent)),
        Expr::MethodCall { name, .. } => {
            unreachable!("`.{}()` reaches emit unresolved; emit takes `check::accept` output", name.as_str())
        }
        Expr::Construct { ty, variant, fields, base } => match (variant, base) {
            (Some(v), None) => emit_variant_value(ty.as_str(), v.as_str(), fields),
            (None, None) if is_closed(ty.as_str()) => {
                format!("{}({})", closed_ctor(ty.as_str()), emit_struct_value(fields))
            }
            (None, None) => emit_struct_value(fields),
            (None, Some(base)) if is_closed(ty.as_str()) => {
                format!("{}{}", closed_ctor(ty.as_str()), emit_struct_update(fields, base))
            }
            (None, Some(base)) => emit_struct_update(fields, base),
            (Some(_), Some(_)) => unreachable!("enum variants have no struct update"),
        },
        Expr::Match { .. } | Expr::Let { .. } => as_expr(expr, indent).unwrap_or_else(|| emit_iife(expr, indent)),
        Expr::If { .. } if expr.needs_statements() => as_expr(expr, indent).unwrap_or_else(|| emit_iife(expr, indent)),
        Expr::Try { .. }
        | Expr::Seq { .. }
        | Expr::Assign { .. }
        | Expr::For { .. }
        | Expr::ForEach { .. }
        | Expr::While { .. } => emit_iife(expr, indent),
        Expr::Break | Expr::Continue => unreachable!("`check::accept` keeps `break` and `continue` in statement position"),
        Expr::If { cond, then, else_ } => format!(
            "({} ? {} : {})",
            emit_expr(cond, indent),
            emit_expr(then, indent),
            emit_expr(else_, indent)
        ),
        // Variant names are identifiers other than `__proto__` (checked), so
        // an object literal is a plain table.
        Expr::Call {
            callee: purecrate_ir::Callee::Discriminant { to, table },
            args,
        } => {
            let entries = table
                .iter()
                .map(|(v, d)| format!("{}: {}", v.as_str(), emit_lit(&Lit::Int { value: *d, ty: Some(*to) })))
                .collect::<Vec<_>>()
                .join(", ");
            // `kind` may be typed `string` (a variant literal widens), and a
            // consumer may set `noUncheckedIndexedAccess`: hence both casts.
            let t = to.ts_name();
            format!("(({{ {entries} }} as Record<string, {t}>)[{}.kind] as {t})", emit_expr(&args[0], indent))
        }
        Expr::Call { callee, args } => {
            if let purecrate_ir::Callee::Consume { method, over } = callee {
                let source = iterable(*over, emit_expr(&args[0], indent));
                return match method {
                    purecrate_ir::Consume::Count => format!("Iter.count({source})"),
                    purecrate_ir::Consume::Sum(int) => format!(
                        "Iter.sum({source}, Int.{}.add, {})",
                        int.as_str(),
                        emit_lit(&Lit::Int { value: 0, ty: Some(*int) })
                    ),
                    m => format!("Iter.{}({source}, {})", m.ts_name(), emit_expr(&args[1], indent)),
                };
            }
            let c = match callee {
                purecrate_ir::Callee::Fn(n) | purecrate_ir::Callee::Local(n) => n.as_str().to_string(),
                purecrate_ir::Callee::Method { ty, name } if is_private_method(ty.as_str(), name.as_str()) => {
                    private_method(ty.as_str(), name.as_str())
                }
                purecrate_ir::Callee::Method { ty, name } => {
                    format!("{}.{}", ty.as_str(), name.as_str())
                }
                purecrate_ir::Callee::Variant { ty, variant } => {
                    format!("{}.{}", ty.as_str(), variant.as_str())
                }
                purecrate_ir::Callee::StructNew(n) if is_closed(n.as_str()) => closed_ctor(n.as_str()),
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
                purecrate_ir::Callee::StrCmp => "Str.cmp".into(),
                purecrate_ir::Callee::OrdCmp { text: false } => "Ord.cmp".into(),
                purecrate_ir::Callee::OrdCmp { text: true } => "Ord.cmpStr".into(),
                purecrate_ir::Callee::OrdThen => "Ord.then".into(),
                purecrate_ir::Callee::Consume { .. } => unreachable!("printed above"),
            };
            if let purecrate_ir::Callee::CharCode(to) = callee {
                let code = format!("Char.code({})", emit_expr(&args[0], indent));
                return if to.is_big() { format!("(globalThis.BigInt({code}) as {})", to.ts_name()) } else { code };
            }
            if let purecrate_ir::Callee::IntFrom { from, to } = callee {
                let x = emit_expr(&args[0], indent);
                let from = from.expect("check::accept sets the source width");
                return match (from.is_big(), to.is_big()) {
                    _ if from == *to => x,
                    (false, true) => format!("(globalThis.BigInt({x}) as {})", to.ts_name()),
                    _ => format!("({x} as number as {})", to.ts_name()),
                };
            }
            if matches!(callee, purecrate_ir::Callee::StrBytes) {
                return format!("Str.bytes({})", emit_expr(&args[0], indent));
            }
            if let purecrate_ir::Callee::Slice { of, start, end } = callee {
                return emit_slice(of.expect("check::accept sets what is sliced"), *start, *end, args, indent);
            }
            if matches!(callee, purecrate_ir::Callee::StrSplit) {
                return format!("{}.split({})", emit_expr(&args[0], indent), emit_expr(&args[1], indent));
            }
            if let purecrate_ir::Callee::Str(m) = callee {
                let s = emit_expr(&args[0], indent);
                let needle = || emit_expr(&args[1], indent);
                return match m {
                    purecrate_ir::StrMethod::Len => format!("Str.len({s})"),
                    purecrate_ir::StrMethod::IsEmpty => format!("({s}.length === 0)"),
                    purecrate_ir::StrMethod::StartsWith => format!("{s}.startsWith({})", needle()),
                    purecrate_ir::StrMethod::EndsWith => format!("{s}.endsWith({})", needle()),
                    purecrate_ir::StrMethod::Contains => format!("{s}.includes({})", needle()),
                    purecrate_ir::StrMethod::AsStr => s,
                    purecrate_ir::StrMethod::StripPrefix => format!("Str.stripPrefix({s}, {})", needle()),
                    purecrate_ir::StrMethod::StripSuffix => format!("Str.stripSuffix({s}, {})", needle()),
                };
            }
            if matches!(callee, purecrate_ir::Callee::VecLen) {
                return format!("(({}.length) as Usize)", emit_expr(&args[0], indent));
            }
            match callee {
                purecrate_ir::Callee::VecIsEmpty => return format!("({}.length === 0)", emit_expr(&args[0], indent)),
                purecrate_ir::Callee::OptionIsSome => return format!("({} !== null)", emit_expr(&args[0], indent)),
                purecrate_ir::Callee::OptionIsNone => return format!("({} === null)", emit_expr(&args[0], indent)),
                _ => {}
            }
            if matches!(callee, purecrate_ir::Callee::Fround) {
                return format!("(globalThis.Math.fround({}) as F32)", emit_expr(&args[0], indent));
            }
            if let purecrate_ir::Callee::AsFloat(ft) = callee {
                return format!("({} as {})", emit_expr(&args[0], indent), ft.ts_name());
            }
            if matches!(callee, purecrate_ir::Callee::OptionNone) {
                return "null".into();
            }
            if matches!(callee, purecrate_ir::Callee::OptionSome | purecrate_ir::Callee::StringFrom) {
                return emit_expr(&args[0], indent);
            }
            let a = args
                .iter()
                .map(|e| emit_expr(e, indent))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{c}({a})")
        }
        Expr::Tuple(elems) => {
            let inner = elems
                .iter()
                .map(|e| emit_expr(e, indent))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        Expr::Array(elems) => {
            let inner = elems
                .iter()
                .map(|e| emit_expr(e, indent))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        Expr::Return(e) => format!("(() => {{ return {}; }})()", emit_expr(e, indent)),
        Expr::Unreachable => "assertNever(undefined as never)".into(),
        Expr::Cast { .. } => unreachable!("`check::accept` rewrites `as`"),
    }
}

/// The IR has no parentheses, so a nested operator is always grouped.
pub(crate) fn operand(expr: &Expr, indent: usize) -> String {
    let s = emit_expr(expr, indent);
    match expr {
        Expr::Binary { .. } => format!("({s})"),
        _ => s,
    }
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
        parts.push(field_pair(name.as_str(), emit_expr(expr, 0)));
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
                .map(|e| emit_expr(e, 0))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        Fields::Named(pairs) => {
            let inner = pairs
                .iter()
                .map(|(n, e)| field_pair(n.as_str(), emit_expr(e, 0)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ {inner} }}")
        }
    }
}

pub(crate) fn emit_variant_value(_ty: &str, variant: &str, fields: &Fields) -> String {
    match fields {
        Fields::Unit => format!("{{ kind: \"{variant}\" }}"),
        Fields::Positional(elems) => {
            let inner = elems
                .iter()
                .map(|e| emit_expr(e, 0))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{ kind: \"{variant}\", content: [{inner}] }}")
        }
        Fields::Named(pairs) => {
            let inner = pairs
                .iter()
                .map(|(n, e)| field_pair(n.as_str(), emit_expr(e, 0)))
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
        Expr::Let { name, mutable: false, value, then, .. } if is_place(value) || matches!(**value, Expr::Lit(_)) => {
            as_expr(&subst(then, name, value), indent)
        }
        Expr::Match { scrutinee, arms } if is_place(scrutinee) => match_expr(scrutinee, arms, indent),
        Expr::If { cond, then, else_ } => {
            let (t, e) = (as_expr(then, indent)?, as_expr(else_, indent)?);
            Some(fold(emit_expr(cond, indent), (t, bool_lit(then)), (e, bool_lit(else_))))
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

/// `test ? then : else_`, as `||` / `&&` when either side is a `bool`
/// literal. Each operand of `||` / `&&` is parenthesized.
fn fold(test: String, (then, then_lit): (String, Option<bool>), (else_, else_lit): (String, Option<bool>)) -> String {
    match (then_lit, else_lit) {
        (Some(true), Some(false)) => format!("({test})"),
        (Some(false), Some(true)) => match crate::tidy::negate(&test) {
            Some(flipped) => format!("({flipped})"),
            None => format!("!({test})"),
        },
        // The other side may be a `?:` or an `||`, which bind looser.
        (Some(true), _) => format!("(({test}) || ({else_}))"),
        (Some(false), _) => format!("(!({test}) && ({else_}))"),
        (_, Some(false)) => format!("(({test}) && ({then}))"),
        (_, Some(true)) => format!("(!({test}) || ({then}))"),
        _ => format!("(({test}) ? {then} : {else_})"),
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
    let field = |base: &Expr, name: &str| Expr::Field { base: Box::new(base.clone()), name: Name::new(name) };
    let mut parts: Vec<(Option<String>, String, Option<bool>)> = Vec::new();
    for (i, arm) in arms.iter().enumerate() {
        let last = i + 1 == arms.len();
        let mut body = arm.body.clone();
        let bind = |p: &Pattern, read: Expr, body: &mut Expr| -> Option<()> {
            match p {
                Pattern::Var(n) => {
                    *body = subst(body, n, &read);
                    Some(())
                }
                Pattern::Wildcard => Some(()),
                _ => None,
            }
        };
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
            Pattern::Variant { variant, bind: vb, .. } => {
                match vb {
                    VariantBind::Unit => {}
                    VariantBind::Tuple(ps) => {
                        for (k, p) in ps.iter().enumerate() {
                            bind(p, field(&field(scrutinee, "content"), &format!("[{k}]")), &mut body)?;
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
            Pattern::Or(alts) if alts.iter().all(|a| matches!(a, Pattern::Variant { bind: VariantBind::Unit, .. })) => Some(
                alts.iter()
                    .filter_map(|a| match a {
                        Pattern::Variant { variant, .. } => Some(format!("{subject}.kind === \"{}\"", variant.as_str())),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" || "),
            ),
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

/// `expr` with each read of `name` replaced by `with`. Names are unique in
/// a function (`check::rename`), so none is shadowed.
fn subst(expr: &Expr, name: &Name, with: &Expr) -> Expr {
    let mut out = expr.clone();
    fn go(e: &mut Expr, name: &Name, with: &Expr) {
        match e {
            Expr::Var(n) if n == name => *e = with.clone(),
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
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c < ' ' || c == '\u{2028}' || c == '\u{2029}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
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
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// `&x[a..b]` and its open forms. A string goes through `Str.slice`, which
/// counts UTF-8 bytes; a `Vec` or slice is checked in Rust's order (start,
/// end, then a reversed range) with Rust's messages.
fn emit_slice(of: purecrate_ir::SliceOf, start: bool, end: bool, args: &[Expr], indent: usize) -> String {
    let base = emit_expr(&args[0], indent);
    let b = end.then(|| emit_expr(&args[args.len() - 1], indent));
    match of {
        purecrate_ir::SliceOf::Str => {
            let a = if start { emit_expr(&args[1], indent) } else { "(0 as Usize)".into() };
            match b {
                Some(b) => format!("Str.slice({base}, {a}, {b})"),
                None => format!("Str.slice({base}, {a})"),
            }
        }
        purecrate_ir::SliceOf::Items => {
            let a = if start { emit_expr(&args[1], indent) } else { "null".into() };
            format!("Slice.range({base}, {a}, {})", b.unwrap_or_else(|| "null".into()))
        }
    }
}
