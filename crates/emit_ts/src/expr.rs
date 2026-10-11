//! Expressions, literals, arrows, and types, printed as TS source.

use super::*;
use crate::tx::{CondStyle, Op, Tx};

/// `indent` is the nesting level of the line the arrow starts on.
pub(crate) fn fn_arrow(f: &Fn, indent: usize) -> String {
    let checked = checked_strings(f);
    let f = &checked;
    // `check` marks the parameters the body never reads (`_a`); one read
    // only in a side `join` folds away (`if false { a } else { 1 }`) is
    // unread in the printed body too, and TS refuses it the same way.
    let joined = crate::join::joined(&f.body);
    let printed = |n: &Name| {
        if n.as_str().starts_with('_') || crate::join::mentions(&joined, n) {
            return n.as_str().to_string();
        }
        (0..)
            .map(|k| if k == 0 { format!("_{}", n.as_str()) } else { format!("_{}{k}", n.as_str()) })
            .find(|c| {
                !f.params.iter().any(|p| p.name.as_str() == c)
                    && !crate::join::mentions(&joined, &Name::new(c.as_str()))
            })
            .expect("a free name")
    };
    let params =
        f.params.iter().map(|p| format!("{}: {}", printed(&p.name), emit_ty(&p.ty))).collect::<Vec<_>>().join(", ");
    let mut pushed = BTreeSet::new();
    f.body.walk(|e| pushed.extend(e.grown().map(|n| n.as_str().to_string())));
    crate::scoped(&crate::PUSHED, pushed, || {
        crate::scoped(&crate::TESTED, tested(&f.body), || arrow(&params, &emit_ty(&f.ret), &f.body, indent))
    })
}

/// The places `body` tests with a `match` (a tuple's elements each), the
/// names its arms bind, and the locals bound to one of those, to a fixed
/// point: what TS may narrow (`context::TESTED`).
fn tested(body: &Expr) -> Vec<Expr> {
    let mut out: Vec<Expr> = Vec::new();
    body.walk(|e| {
        if let Expr::Match { scrutinee, arms } = e {
            let parts = match &**scrutinee {
                Expr::Tuple(xs) => xs.iter().collect(),
                s => vec![s],
            };
            out.extend(parts.into_iter().filter(|p| is_place(p)).cloned());
            for arm in arms {
                out.extend(arm.pattern.bindings().into_iter().map(|n| Expr::Var(n.clone())));
            }
        }
        // `o == Ordering::Less` reads `o.kind === "Less"`, which narrows `o`.
        if let Expr::Binary { op: BinOp::Eq | BinOp::Ne, left, right } = e {
            for side in [left, right] {
                if let Expr::Field { base, name } = peel_identity(side) {
                    if name.as_str() == "kind" && is_place(base) {
                        out.push((**base).clone());
                    }
                }
            }
        }
    });
    loop {
        let mut grew = false;
        body.walk(|e| {
            if let Expr::Let { name, value, .. } = e {
                let copy = Expr::Var(name.clone());
                if !out.contains(&copy) && out.iter().any(|t| crate::stmt::within(value, t)) {
                    out.push(copy);
                    grew = true;
                }
            }
        });
        if !grew {
            return out;
        }
    }
}

/// `f` with each parameter that may hold a string checked on entry, where
/// `f` is `pub`: a caller in TS may pass a lone surrogate, which no Rust
/// `str` holds, in a string or anywhere in an open struct, an enum, an
/// `Option`, a `Vec`, or a tuple (`context::holds_string`).
pub(crate) fn checked_strings(f: &Fn) -> Fn {
    let mut body = f.body.clone();
    if f.vis == purecrate_ir::Vis::Pub {
        for p in f.params.iter().rev() {
            if crate::context::holds_string(&p.ty) {
                let check = Expr::Call { callee: Callee::StrWellFormed, args: vec![Expr::Var(p.name.clone())] };
                body = Expr::Seq { first: Box::new(check), then: Box::new(body) };
            }
        }
    }
    Fn { body, ..f.clone() }
}

/// `a && m` as `if a { m } else { false }`, and `a || m` as `if a { true }
/// else { m }`, where an operand is no expression (`as_tx`) and the `&&`
/// stands where a statement may (a body's value, a `let`'s or an
/// assignment's, a `return`'s, a branch's): the `if` is printed as
/// statements there, where `&&` would hold an inline function that oxfmt
/// breaks at each operator. Inside an expression (a test, an operand, an
/// argument) the `&&` stays: an `if` there is one inline function, across
/// which TS keeps no narrowing of the left side (`x !== null && ..`). A
/// closure's body is split when the closure is printed.
pub(crate) fn split_operands(e: &mut Expr) {
    split_at(e, true);
}

fn split_at(e: &mut Expr, statement: bool) {
    match e {
        Expr::Closure { .. } => return,
        Expr::Seq { first, then } => {
            split_at(first, true);
            split_at(then, statement);
        }
        Expr::Let { value, then, .. } => {
            split_at(value, true);
            split_at(then, statement);
        }
        Expr::Assign { value, .. } | Expr::Return(value) => split_at(value, true),
        Expr::If { cond, then, else_ } => {
            split_at(cond, false);
            // `if a && m { x } else { y }`, `m` no expression, as a statement:
            // `if a { if m { x } else { y } } else { y }`, which keeps what
            // `a` narrows for `x` without an inline function in the test.
            // The branch said twice is one that is cheap to say twice.
            if statement {
                // `if { let r = v; t } { .. }` as `let r = v; if t { .. }`:
                // the test runs first either way, and `rename` gave each
                // binding a name of its own in the function.
                match &**cond {
                    Expr::Let { name, mutable, ty, value, then: test } => {
                        let inner = Expr::If { cond: test.clone(), then: then.clone(), else_: else_.clone() };
                        *e = Expr::Let {
                            name: name.clone(),
                            mutable: *mutable,
                            ty: ty.clone(),
                            value: value.clone(),
                            then: Box::new(inner),
                        };
                        return split_at(e, true);
                    }
                    Expr::Seq { first, then: test } if !matches!(**first, Expr::Comment(_)) => {
                        let inner = Expr::If { cond: test.clone(), then: then.clone(), else_: else_.clone() };
                        *e = Expr::Seq { first: first.clone(), then: Box::new(inner) };
                        return split_at(e, true);
                    }
                    _ => {}
                }
                if let Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } = &**cond {
                    let copied = if *op == BinOp::And { &**else_ } else { &**then };
                    if as_tx(left, 0).is_some() && as_tx(right, 0).is_none() && cheap(copied) {
                        let inner = Expr::If { cond: right.clone(), then: then.clone(), else_: else_.clone() };
                        let (t, f) = match op {
                            BinOp::And => (inner, (**else_).clone()),
                            _ => ((**then).clone(), inner),
                        };
                        *e = Expr::If { cond: left.clone(), then: Box::new(t), else_: Box::new(f) };
                        return split_at(e, true);
                    }
                }
            }
            split_at(then, statement);
            split_at(else_, statement);
        }
        Expr::Match { scrutinee, arms } => {
            split_at(scrutinee, false);
            for arm in arms {
                split_at(&mut arm.body, statement);
            }
        }
        Expr::For { start, end, body, .. } => {
            split_at(start, false);
            split_at(end, false);
            split_at(body, true);
        }
        Expr::ForEach { source, body, .. } => {
            split_at(source, false);
            split_at(body, true);
        }
        Expr::While { cond, body } => {
            split_at(cond, false);
            split_at(body, true);
        }
        _ => {
            for c in e.children_mut() {
                split_at(c, false);
            }
        }
    }
    if !statement {
        return;
    }
    let Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } = e else { return };
    if as_tx(left, 0).is_some() && as_tx(right, 0).is_some() {
        return;
    }
    let (left, right) =
        (std::mem::replace(&mut **left, Expr::Lit(Lit::Unit)), std::mem::replace(&mut **right, Expr::Lit(Lit::Unit)));
    let (then, else_) = match op {
        BinOp::And => (right, Expr::Lit(Lit::Bool(false))),
        _ => (Expr::Lit(Lit::Bool(true)), right),
    };
    *e = Expr::If { cond: Box::new(left), then: Box::new(then), else_: Box::new(else_) };
    // The branches stand where the `&&` did.
    if let Expr::If { then, else_, .. } = e {
        split_at(then, true);
        split_at(else_, true);
    }
}

/// A value that costs nothing to print twice: no statements, no call but a
/// constructor (`Ok(b)`), and not long.
fn cheap(e: &Expr) -> bool {
    !e.needs_statements()
        && !e.any(|x| {
            matches!(x, Expr::Call { callee, .. }
                if !matches!(callee, Callee::OptionNone | Callee::ResultOk | Callee::ResultErr))
                || matches!(x, Expr::Closure { .. })
        })
        && as_tx(e, 0).is_some_and(|t| t.print().len() <= 60)
}

pub(crate) fn closure_arrow(params: &[ClosureParam], ret: Option<&Ty>, body: &Expr, indent: usize) -> String {
    let typed = |name: &Name, ty: Option<&Ty>| match ty {
        Some(t) => format!("{}: {}", name.as_str(), emit_ty(t)),
        None => name.as_str().to_string(),
    };
    let params = params.iter().map(|p| typed(&p.name, p.ty.as_ref())).collect::<Vec<_>>().join(", ");
    match ret {
        Some(r) => arrow(&params, &emit_ty(r), body, indent),
        None => format!("({params}) => {}", arrow_expr(&crate::join::joined(body), indent)),
    }
}

pub(crate) fn arrow(params: &str, ret: &str, body: &Expr, indent: usize) -> String {
    let body = &crate::join::joined(body);
    // `String::from(x)` prints as `x`: a `match` under it is the body itself.
    let body = match body {
        Expr::Call { callee: Callee::StringFrom, args } if args[0].needs_statements() => &args[0],
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
            let choice = as_tx(body, indent + 1).is_some_and(|t| t.is_cond()) || crate::tidy::is_ternary(value);
            let value = if value.starts_with('{') || choice { format!("({value})") } else { value.to_string() };
            return format!("({params}): {ret} => {value}");
        }
        format!("({params}): {ret} => {{\n{out}{pad}}}", pad = "  ".repeat(indent))
    } else {
        format!("({params}): {ret} => {}", arrow_expr(body, indent))
    }
}

/// An arrow body starting with `{` would parse as a block.
pub(crate) fn arrow_expr(expr: &Expr, indent: usize) -> String {
    let t = emit_tx(expr, indent);
    let s = t.print();
    if leads_with_brace(expr) {
        format!("({s})")
    } else {
        // Parentheses around all of it, unless they keep an object literal
        // from reading as a block.
        let bare = crate::tidy::strip_outer(&s);
        if bare.starts_with('{') {
            s
        } else if t.is_cond() {
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
        Expr::Construct { ty, variant, fields, base: None } => {
            variant.is_some() || !(matches!(fields, Fields::Positional(_)) || is_closed(ty.as_str()))
        }
        Expr::Field { base, .. } => leads_with_brace(base),
        Expr::Binary { left, .. } | Expr::If { cond: left, .. } => leads_with_brace(left),
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
            let params =
                params.iter().enumerate().map(|(i, t)| format!("_{i}: {}", emit_ty(t))).collect::<Vec<_>>().join(", ");
            format!("(({params}) => {})", emit_ty(ret))
        }
        Ty::Never => "never".into(),
    }
}

pub(crate) fn emit_expr(expr: &Expr, indent: usize) -> String {
    emit_tx(expr, indent).print()
}

/// `expr` as a tree of the operators it prints as (`crate::tx`): binary
/// operators, `!`, `-`, `?:`, and the calls and `match`es that print as one
/// (`o !== null`, `xs.length === 0`, `x ?? d`). Anything else is an atom.
pub(crate) fn emit_tx(expr: &Expr, indent: usize) -> Tx {
    use purecrate_ir::Callee;
    let expr = peel_identity(expr);
    match expr {
        Expr::Ignored { expr, .. } => emit_tx(expr, indent),
        Expr::Seq { first, then } if matches!(**first, Expr::Comment(_)) => emit_tx(then, indent),
        Expr::Binary { op, left, right } => {
            // A comparison reads a literal or a length bare: `<` and `===`
            // compare the values, and a brand does not change them.
            let comparison = matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne);
            let side = |e: &Expr| match comparison.then(|| bare(e, indent)).flatten() {
                Some(b) => Tx::atom(b),
                None => emit_tx(e, indent),
            };
            Tx::bin(tx_op(*op), side(left), side(right))
        }
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => emit_tx(expr, indent).not(),
        // Only a float is negated here (an integer is `Int.*.neg`): a
        // literal bare, as `-90.0`; a value multiplied by `-1`, which flips
        // its sign exactly, `-0.0` included (lint refuses `-` on a brand).
        // The `AsFloat` around it brands it back.
        Expr::Unary { op: purecrate_ir::UnOp::Neg, expr } => match peel_identity(expr) {
            Expr::Lit(Lit::Float { digits, ty: Some(FloatTy::F64) | None }) => {
                Tx::Neg(Box::new(Tx::atom(f64_digits(digits))))
            }
            // An `f32` literal is its rounded value; rounding is symmetric in
            // sign, so its negation is the negated value (no `* -1` on a
            // literal, which lint refuses as an erasing operation on `0.0`).
            Expr::Lit(Lit::Float { digits, ty: Some(FloatTy::F32) })
                if digits.parse::<f32>().is_ok_and(f32::is_finite) =>
            {
                let v = digits.parse::<f32>().expect("checked by the guard");
                Tx::Neg(Box::new(Tx::atom(format!("{:?}", f64::from(v)))))
            }
            e => Tx::bin(Op::Mul, emit_tx(e, indent), Tx::atom("-1")),
        },
        Expr::Match { .. } | Expr::Let { .. } => {
            as_tx(expr, indent).unwrap_or_else(|| Tx::atom(emit_iife(expr, indent)))
        }
        Expr::If { .. } if expr.needs_statements() => {
            as_tx(expr, indent).unwrap_or_else(|| Tx::atom(emit_iife(expr, indent)))
        }
        // `!c ? a : b` is `c ? b : a`, unless `b` is a `?:` that would move
        // into the middle.
        Expr::If { cond, then, else_ }
            if !matches!(peel_identity(else_), Expr::If { .. } | Expr::Match { .. })
                && matches!(peel_identity(cond), Expr::Unary { op: purecrate_ir::UnOp::Not, .. }) =>
        {
            let Expr::Unary { expr: positive, .. } = peel_identity(cond) else { unreachable!("matched above") };
            emit_tx(&Expr::If { cond: positive.clone(), then: else_.clone(), else_: then.clone() }, indent)
        }
        // `c ? true : false` is `c`, as oxlint's `no-unneeded-ternary` asks.
        Expr::If { cond, then, else_ }
            if bool_lit(then).is_some() && bool_lit(else_).is_some() && bool_lit(then) != bool_lit(else_) =>
        {
            fold(emit_tx(cond, indent), (Tx::atom(""), bool_lit(then)), (Tx::atom(""), bool_lit(else_)))
        }
        Expr::If { cond, then, else_ } => Tx::Cond(
            CondStyle::Plain,
            Box::new(emit_tx(cond, indent)),
            Box::new(emit_tx(then, indent)),
            Box::new(emit_tx(else_, indent)),
        ),
        Expr::Call { callee: callee @ (Callee::OptionIsSome | Callee::OptionIsNone), args } => {
            let op = if matches!(callee, Callee::OptionIsSome) { Op::Ne } else { Op::Eq };
            Tx::bin(op, emit_tx(&args[0], indent), Tx::atom("null"))
        }
        Expr::Call { callee: Callee::VecIsEmpty | Callee::Str(purecrate_ir::StrMethod::IsEmpty), args } => {
            Tx::bin(Op::Eq, Tx::atom(format!("{}.length", receiver(emit_tx(&args[0], indent)))), Tx::atom("0"))
        }
        Expr::Call { callee: Callee::OptionSome | Callee::StringFrom, args } => emit_tx(&args[0], indent),
        // A `string` and a `Char` or `string`: JS `+` joins them.
        Expr::Call { callee: Callee::StrConcat, args } => {
            // A `char` literal needs no `as Char` to be joined.
            let piece = match &args[1] {
                Expr::Lit(Lit::Char(c)) => Tx::atom(js_string(&c.to_string())),
                other => emit_tx(other, indent),
            };
            Tx::bin(Op::Add, emit_tx(&args[0], indent), piece)
        }
        Expr::Call { callee: Callee::IntFrom { from: Some(from), to }, args } if from == to => {
            emit_tx(&args[0], indent)
        }
        _ => Tx::atom(emit_atom(expr, indent)),
    }
}

/// `expr` where it binds tighter than any operator: a name, a literal, a
/// member, a call, or what the printer parenthesizes itself.
fn emit_atom(expr: &Expr, indent: usize) -> String {
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
        Expr::Field { base, name } => format!("{}.{n}", receiver(emit_tx(base, indent)), n = name.as_str()),
        // `Slice.at` takes a plain `number`: a literal index needs no brand,
        // nor a `u32 as usize`, whose value is already that `number`.
        Expr::Index { base, index } => format!(
            "Slice.at({}, {})",
            emit_expr(base, indent),
            match peel_identity(index) {
                Expr::Lit(lit @ Lit::Int { .. }) => bare_lit(lit),
                Expr::Call { callee: purecrate_ir::Callee::IntFrom { from: Some(from), to }, args }
                    if !from.is_big() && !to.is_big() =>
                {
                    emit_item(&args[0], indent)
                }
                // A length (`xs[ys.len()]`) without its `as Usize`.
                e => bare(e, indent).unwrap_or_else(|| emit_item(index, indent)),
            }
        ),
        Expr::Binary { .. } | Expr::Unary { .. } => unreachable!("`emit_tx` prints operators"),
        // `|x| f(x)`, which a function name passed to `map` or `all` becomes,
        // is `f` itself: its parameter and return types are the arrow's.
        Expr::Closure { params, body, .. } if forwards(params, body).is_some() => {
            forwards(params, body).expect("checked above").to_string()
        }
        Expr::Closure { params, ret, body } => {
            format!("({})", closure_arrow(params, ret.as_ref(), body, indent))
        }
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
                format!("{}{}", closed_ctor(ty.as_str()), emit_closed_update(ty.as_str(), fields, base))
            }
            (None, Some(base)) => emit_struct_update(fields, base),
            (Some(_), Some(_)) => unreachable!("enum variants have no struct update"),
        },
        Expr::Match { .. } | Expr::Let { .. } | Expr::If { .. } => unreachable!("`emit_tx` prints choices"),
        Expr::Try { .. }
        | Expr::Seq { .. }
        | Expr::Assign { .. }
        | Expr::For { .. }
        | Expr::ForEach { .. }
        | Expr::While { .. } => emit_iife(expr, indent),
        Expr::Break | Expr::Continue => {
            unreachable!("`check::accept` keeps `break` and `continue` in statement position")
        }
        // Variant names are identifiers other than `__proto__` (checked), so
        // an object literal is a plain table.
        Expr::Call { callee: purecrate_ir::Callee::Discriminant { to, table, of }, args } => {
            match typed_table(table, of, to.is_big(), &args[0], indent) {
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
                                    ty: Some(*to),
                                    byte: false,
                                    hex: false
                                }))
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("(({{ {entries} }} as Record<string, {t}>)[{}.kind] as {t})", emit_expr(&args[0], indent))
                }
            }
        }
        Expr::Call { callee, args } => {
            if let purecrate_ir::Callee::Consume { method, over } = callee {
                let source = iterable(*over, emit_tx(&args[0], indent));
                return match method {
                    purecrate_ir::Consume::Count => format!("Iter.count({source})"),
                    purecrate_ir::Consume::Sum(int) => format!(
                        "Iter.sum({source}, Int.{}.add, {})",
                        int.as_str(),
                        crate::tidy::strip_outer(&emit_lit(&Lit::Int {
                            value: 0,
                            ty: Some(*int),
                            byte: false,
                            hex: false
                        }))
                    ),
                    purecrate_ir::Consume::Max { text } | purecrate_ir::Consume::Min { text } => {
                        format!("Iter.{}({source}, {})", method.ts_name(), if *text { "Ord.cmpStr" } else { "Ord.cmp" })
                    }
                    purecrate_ir::Consume::MaxByKey { text } | purecrate_ir::Consume::MinByKey { text } => format!(
                        "Iter.{}({source}, {}, {})",
                        method.ts_name(),
                        emit_item(&args[1], indent),
                        if *text { "Ord.cmpStr" } else { "Ord.cmp" }
                    ),
                    m => format!("Iter.{}({source}, {})", m.ts_name(), emit_item(&args[1], indent)),
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
                purecrate_ir::Callee::StringNew => return "\"\"".into(),
                purecrate_ir::Callee::StrWellFormed => {
                    return format!("Str.wellFormed({})", emit_expr(&args[0], indent))
                }
                purecrate_ir::Callee::StrFromChars => {
                    return format!("{}.join(\"\")", receiver(emit_tx(&args[0], indent)));
                }
                purecrate_ir::Callee::StrConcat => unreachable!("`emit_tx` prints `+`"),
                purecrate_ir::Callee::VecLen
                | purecrate_ir::Callee::VecPush
                | purecrate_ir::Callee::VecInsert
                | purecrate_ir::Callee::VecRemove
                | purecrate_ir::Callee::VecSet
                | purecrate_ir::Callee::VecIsEmpty
                | purecrate_ir::Callee::OptionIsSome
                | purecrate_ir::Callee::OptionIsNone
                | purecrate_ir::Callee::StrBytes
                | purecrate_ir::Callee::StrSplit
                | purecrate_ir::Callee::Collect { .. }
                | purecrate_ir::Callee::IterMap { .. }
                | purecrate_ir::Callee::IterFilter { .. }
                | purecrate_ir::Callee::StringFrom
                | purecrate_ir::Callee::Slice { .. }
                | purecrate_ir::Callee::Str(_)
                | purecrate_ir::Callee::IntFrom { .. }
                | purecrate_ir::Callee::CharCode(_) => String::new(),
                // `cast` takes any `number` or `bigint`: a literal or a length
                // is read without its brand.
                purecrate_ir::Callee::IntCast { to, .. } => {
                    let x = bare(&args[0], indent).unwrap_or_else(|| emit_item(&args[0], indent));
                    return format!("Int.{}.cast({x})", to.as_str());
                }
                purecrate_ir::Callee::FloatToInt { to, .. } => {
                    return format!("Int.{}.castFloat({})", to.as_str(), emit_item(&args[0], indent));
                }
                purecrate_ir::Callee::FloatFrom(_) => unreachable!("`check::accept` rewrites `from`"),
                purecrate_ir::Callee::Float { .. }
                | purecrate_ir::Callee::FloatConst { .. }
                | purecrate_ir::Callee::IntToFloat { .. }
                | purecrate_ir::Callee::FloatToFloat { .. } => String::new(),
                purecrate_ir::Callee::CharFromU8 => "Char.fromU8".into(),
                purecrate_ir::Callee::CharFromU32 => "Char.fromU32".into(),
                purecrate_ir::Callee::Char(m) => format!("Char.{}", m.ts_name()),
                purecrate_ir::Callee::UuidParse => "Uuid.parseStr".into(),
                purecrate_ir::Callee::UuidNil => "Uuid.nil".into(),
                purecrate_ir::Callee::StrParse(t) => format!("Int.{}.parse", t.as_str()),
                purecrate_ir::Callee::StrCmp => "Str.cmp".into(),
                purecrate_ir::Callee::DeepEq => "Eq.deep".into(),
                purecrate_ir::Callee::VecSort(sort) => {
                    let by = |text: bool| if text { "Ord.cmpStr" } else { "Ord.cmp" };
                    let v = emit_expr(&args[0], indent);
                    return match sort {
                        purecrate_ir::Sort::Natural { text } => format!("Slice.sortBy({v}, {})", by(*text)),
                        purecrate_ir::Sort::By => format!("Slice.sortBy({v}, {})", emit_item(&args[1], indent)),
                        purecrate_ir::Sort::ByKey { text } => {
                            format!("Slice.sortByKey({v}, {}, {})", emit_item(&args[1], indent), by(*text))
                        }
                    };
                }
                purecrate_ir::Callee::OrdCmp { text: false } => "Ord.cmp".into(),
                purecrate_ir::Callee::OrdCmp { text: true } => "Ord.cmpStr".into(),
                purecrate_ir::Callee::OrdThen => "Ord.then".into(),
                purecrate_ir::Callee::OrdCmpList { .. } => "Ord.cmpList".into(),
                purecrate_ir::Callee::Consume { .. } => unreachable!("printed above"),
            };
            if let purecrate_ir::Callee::OrdCmpList { text } = callee {
                let by = if *text { "Ord.cmpStr" } else { "Ord.cmp" };
                return format!("Ord.cmpList({}, {}, {by})", emit_expr(&args[0], indent), emit_expr(&args[1], indent));
            }
            if let purecrate_ir::Callee::CharCode(to) = callee {
                let code = format!("Char.code({})", emit_expr(&args[0], indent));
                return if to.is_big() { format!("(globalThis.BigInt({code}) as {})", to.ts_name()) } else { code };
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
                let x = emit_tx(&args[0], indent);
                let from = from.expect("check::accept sets the source width");
                return match (from.is_big(), to.is_big()) {
                    _ if from == *to => unreachable!("`emit_tx` reads a widening to the same type as its value"),
                    (false, true) => format!("(globalThis.BigInt({}) as {})", uncast_default(&x.print()), to.ts_name()),
                    (true, true) => format!("({} as bigint as {})", cast_operand(x), to.ts_name()),
                    _ => format!("({} as number as {})", cast_operand(x), to.ts_name()),
                };
            }
            if matches!(callee, purecrate_ir::Callee::StrBytes) {
                return format!("Str.bytes({})", emit_expr(&args[0], indent));
            }
            if let purecrate_ir::Callee::Slice { of, start, end } = callee {
                return emit_slice(of.expect("check::accept sets what is sliced"), *start, *end, args, indent);
            }
            if matches!(callee, purecrate_ir::Callee::StrSplit)
                && matches!(peel_identity(&args[1]), Expr::Closure { .. })
            {
                return format!("Str.splitBy({}, {})", emit_expr(&args[0], indent), emit_item(&args[1], indent));
            }
            if matches!(callee, purecrate_ir::Callee::StrSplit) {
                // JS `split` takes a string; the `char` needs no brand.
                return format!(
                    "{}.split({})",
                    receiver(emit_tx(&args[0], indent)),
                    bare(&args[1], indent).unwrap_or_else(|| emit_expr(&args[1], indent))
                );
            }
            if let purecrate_ir::Callee::IterMap { over } | purecrate_ir::Callee::IterFilter { over } = callee {
                let stage = if matches!(callee, purecrate_ir::Callee::IterMap { .. }) { "map" } else { "filter" };
                let source = iterable(*over, emit_tx(&args[0], indent));
                return format!("Iter.{stage}({source}, {})", emit_item(&args[1], indent));
            }
            if let purecrate_ir::Callee::Collect { result, over } = callee {
                return emit_collect(*result, *over, args, indent);
            }
            if let purecrate_ir::Callee::Str(m) = callee {
                // The object of `.startsWith` and the other members; a call
                // argument (`Str.len(s)`) takes it as it is.
                let s = receiver(emit_tx(&args[0], indent));
                let needle = || bare(&args[1], indent).unwrap_or_else(|| emit_item(&args[1], indent));
                return match m {
                    purecrate_ir::StrMethod::Len => format!("Str.len({s})"),
                    purecrate_ir::StrMethod::IsEmpty => unreachable!("`emit_tx` prints a comparison"),
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
                    purecrate_ir::StrMethod::EqIgnoreAsciiCase => {
                        format!("Str.eqIgnoreAsciiCase({s}, {})", needle())
                    }
                    m if m.needles() == 0 => format!("Str.{}({s})", m.ts_name()),
                    m => format!("Str.{}({s}, {})", m.ts_name(), needle()),
                };
            }
            if matches!(callee, purecrate_ir::Callee::VecLen) {
                return format!("({}.length as Usize)", receiver(emit_tx(&args[0], indent)));
            }
            if matches!(callee, purecrate_ir::Callee::VecPush) {
                return format!("{}.push({})", receiver(emit_tx(&args[0], indent)), emit_item(&args[1], indent));
            }
            if matches!(callee, purecrate_ir::Callee::VecInsert) {
                return format!(
                    "Slice.insert({}, {}, {})",
                    emit_expr(&args[0], indent),
                    emit_item(&args[1], indent),
                    emit_item(&args[2], indent)
                );
            }
            if matches!(callee, purecrate_ir::Callee::VecRemove) {
                return format!("Slice.remove({}, {})", emit_expr(&args[0], indent), emit_item(&args[1], indent));
            }
            if matches!(callee, purecrate_ir::Callee::VecSet) {
                return format!(
                    "Slice.set({}, {}, {})",
                    emit_expr(&args[0], indent),
                    emit_item(&args[1], indent),
                    emit_item(&args[2], indent)
                );
            }
            if let purecrate_ir::Callee::Float { ty, m } = callee {
                use purecrate_ir::FloatMethod as F;
                let x = || float_arg(&args[0], indent);
                let t = ty.ts_name();
                return match m {
                    // `round` takes the branded float, so the value's cast stays.
                    F::Round => format!("Int.{}.round({})", ty.as_str(), emit_item(&args[0], indent)),
                    F::Floor | F::Ceil | F::Trunc | F::Abs => {
                        let f = match m {
                            F::Floor => "floor",
                            F::Ceil => "ceil",
                            F::Trunc => "trunc",
                            _ => "abs",
                        };
                        format!("(globalThis.Math.{f}({}) as {t})", x())
                    }
                    F::IsNan => format!("globalThis.Number.isNaN({})", x()),
                    F::IsFinite => format!("globalThis.Number.isFinite({})", x()),
                    // A call, not `Math.abs(x) === Infinity`: a `!` before it
                    // would take only the left side.
                    F::IsInfinite => format!("Int.{}.isInfinite({})", ty.as_str(), x()),
                };
            }
            if let purecrate_ir::Callee::FloatConst { ty, c } = callee {
                let v = match c {
                    purecrate_ir::FloatConst::Nan => "NaN",
                    purecrate_ir::FloatConst::Infinity => "POSITIVE_INFINITY",
                    purecrate_ir::FloatConst::NegInfinity => "NEGATIVE_INFINITY",
                };
                return format!("(globalThis.Number.{v} as {})", ty.ts_name());
            }
            if let purecrate_ir::Callee::IntToFloat { from, to } = callee {
                let x = emit_item(&args[0], indent);
                return match (from.is_big(), to) {
                    (false, purecrate_ir::FloatTy::F64) => {
                        format!("({} as number as F64)", cast_operand(emit_tx(&args[0], indent)))
                    }
                    (false, purecrate_ir::FloatTy::F32) => {
                        format!("(globalThis.Math.fround({}) as F32)", emit_item(&args[0], indent))
                    }
                    (true, purecrate_ir::FloatTy::F64) => format!("(globalThis.Number({x}) as F64)"),
                    (true, purecrate_ir::FloatTy::F32) => format!("Int.f32.ofBig({x})"),
                };
            }
            if let purecrate_ir::Callee::FloatToFloat { to } = callee {
                return match to {
                    purecrate_ir::FloatTy::F64 => {
                        format!("({} as number as F64)", cast_operand(emit_tx(&args[0], indent)))
                    }
                    purecrate_ir::FloatTy::F32 => {
                        format!("(globalThis.Math.fround({}) as F32)", emit_item(&args[0], indent))
                    }
                };
            }
            if matches!(callee, purecrate_ir::Callee::Fround) {
                return format!("(globalThis.Math.fround({}) as F32)", emit_expr(&args[0], indent));
            }
            if let purecrate_ir::Callee::AsFloat(ft) = callee {
                return format!("({} as {})", cast_operand(emit_tx(&args[0], indent)), ft.ts_name());
            }
            if matches!(callee, purecrate_ir::Callee::OptionNone) {
                return "null".into();
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

fn tx_op(op: BinOp) -> Op {
    match op {
        BinOp::Mul => Op::Mul,
        BinOp::Div => Op::Div,
        BinOp::Rem => Op::Rem,
        BinOp::Add => Op::Add,
        BinOp::Sub => Op::Sub,
        BinOp::Lt => Op::Lt,
        BinOp::Le => Op::Le,
        BinOp::Gt => Op::Gt,
        BinOp::Ge => Op::Ge,
        BinOp::Eq => Op::Eq,
        BinOp::Ne => Op::Ne,
        BinOp::And => Op::And,
        BinOp::Or => Op::Or,
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
            unreachable!("`check::accept` rewrites bitwise operators into `Int` calls")
        }
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
        let base = format!("{}Discriminants{}", purecrate_ir::lower_first(of.as_str()), if big && t.iter().any(|(k, ..)| k.0 == key.0) { "Big" } else { "" });
        let name = temp(&base);
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

/// A closed struct's update names every field, `{ sku: l.sku, qty }`, where
/// the base is a place: the brand is a class's `private` member, and
/// oxlint refuses a spread of what reads as a class instance (it would lose
/// the prototype, which this one does not have). A base that is no place is
/// read once, by the spread.
fn emit_closed_update(ty: &str, fields: &Fields, base: &Expr) -> String {
    let names = crate::CLOSED_FIELDS.with(|c| c.borrow().get(ty).cloned());
    let (Fields::Named(pairs), Some(names), true) = (fields, names, is_place(base)) else {
        return emit_struct_update(fields, base);
    };
    let base = emit_expr(base, 0);
    let parts: Vec<String> = names
        .iter()
        .map(|n| match pairs.iter().find(|(f, _)| f.as_str() == n) {
            Some((_, e)) => field_pair(n, emit_item(e, 0)),
            None => field_pair(n, format!("{}.{n}", receiver(Tx::atom(base.clone())))),
        })
        .collect();
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
            let inner = elems.iter().map(|e| emit_item(e, 0)).collect::<Vec<_>>().join(", ");
            format!("[{inner}]")
        }
        Fields::Named(pairs) => {
            let inner =
                pairs.iter().map(|(n, e)| field_pair(n.as_str(), emit_item(e, 0))).collect::<Vec<_>>().join(", ");
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
            let inner = elems.iter().map(|e| emit_item(e, 0)).collect::<Vec<_>>().join(", ");
            format!("{{ kind: \"{variant}\", content: [{inner}] }}")
        }
        Fields::Named(pairs) => {
            let inner =
                pairs.iter().map(|(n, e)| field_pair(n.as_str(), emit_item(e, 0))).collect::<Vec<_>>().join(", ");
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
    as_tx(expr, indent).map(|t| t.print())
}

pub(crate) fn as_tx(expr: &Expr, indent: usize) -> Option<Tx> {
    match expr {
        // A closure called by its name (`g(1)`) is not a read `subst` rewrites.
        Expr::Let { name, mutable: false, value, then, .. }
            if value.is_inlinable()
                && !then.any(|e| matches!(e, Expr::Call { callee: Callee::Local(n), .. } if n == name)) =>
        {
            as_tx(&subst(then, name, value), indent)
        }
        Expr::Match { scrutinee, arms } if is_place(scrutinee) => match_tx(scrutinee, arms, indent),
        // A `match` on a value that is not a place, read once: `call ?? d`,
        // or a test `call.kind === "A"`.
        Expr::Match { .. } if crate::stmt::coalesced_tx(expr, indent).is_some() => {
            crate::stmt::coalesced_tx(expr, indent)
        }
        Expr::Match { scrutinee, arms } if one_of(arms).is_some() => {
            let (lits, yes) = one_of(arms)?;
            let test = Tx::atom(format!("[{lits}].includes({})", emit_tx(scrutinee, indent).print()));
            Some(if yes { test } else { Tx::Not(Box::new(test)) })
        }
        Expr::If { cond, then, else_ } => {
            let (t, e) = (as_tx(then, indent)?, as_tx(else_, indent)?);
            Some(fold(emit_tx(cond, indent), (t, bool_lit(then)), (e, bool_lit(else_))))
        }
        e if !e.needs_statements() => Some(emit_tx(e, indent)),
        _ => None,
    }
}

/// `x` as the object of `.member`: parenthesized unless it binds tighter
/// than any operator (`(c ? a : b).length`).
fn receiver(x: Tx) -> String {
    match x {
        Tx::Atom(s) => s,
        other => format!("({})", other.print()),
    }
}

/// `collect()`, always a new array (design/01 §7.14): the pieces of a split
/// as they are; a `Vec`'s items copied (`[...xs]`); one `map` or `filter`
/// over an array as the array's method, which calls `f` in the same order;
/// anything longer, or over a string's chars or bytes, through the lazy
/// `Iter` stages and `Array.from`, so each item runs every stage before the
/// next, as in Rust.
fn emit_collect(result: bool, over: purecrate_ir::Over, args: &[Expr], indent: usize) -> String {
    use purecrate_ir::Callee;
    let stage = |e: &Expr| matches!(e, Expr::Call { callee: Callee::IterMap { .. } | Callee::IterFilter { .. }, .. });
    let split = |e: &Expr| matches!(e, Expr::Call { callee: Callee::StrSplit, .. });
    // `split` with a closure is `Str.splitBy`, a generator: not an array.
    let lazy = |e: &Expr| matches!(e, Expr::Call { callee: Callee::StrSplit, args } if matches!(peel_identity(&args[1]), Expr::Closure { .. }));
    let source = &args[0];
    let array = over == purecrate_ir::Over::Items && !stage(source) && !lazy(source);
    let f = args.get(1).map(|f| emit_item(f, indent));
    if result {
        let f = f.expect("a `Result` is collected through `map(f)`");
        return format!("Iter.tryCollect({}, {f})", iterable(over, emit_tx(source, indent)));
    }
    match (f, source) {
        (None, s) if split(s) && !lazy(s) => emit_expr(s, indent),
        // A choice or an operator under `...` is parenthesized, as oxfmt prints it.
        (None, s) if array => {
            let s = emit_tx(s, indent);
            if s.prec() < crate::tidy::PREC_UNARY {
                format!("[...({})]", s.print())
            } else {
                format!("[...{}]", s.print())
            }
        }
        // One `filter` over an array.
        (None, Expr::Call { callee: Callee::IterFilter { over: purecrate_ir::Over::Items }, args: inner })
            if !stage(&inner[0]) =>
        {
            format!("{}.filter({})", receiver(emit_tx(&inner[0], indent)), emit_item(&inner[1], indent))
        }
        (None, s) => format!("Array.from({})", iterable(over, emit_tx(s, indent))),
        (Some(f), s) if array => format!("{}.map({f})", receiver(emit_tx(s, indent))),
        (Some(f), s) => format!("Array.from(Iter.map({}, {f}))", iterable(over, emit_tx(s, indent))),
    }
}

/// `matches!(e, "a" | "b")` of literals, as the literals and whether the
/// match is `true` for them: `["a", "b"].includes(e)` reads `e` once, where
/// a scrutinee that is not a place would otherwise need an inline function.
fn one_of(arms: &[purecrate_ir::Arm]) -> Option<(String, bool)> {
    let [a, b] = arms else { return None };
    if a.guard.is_some() || b.guard.is_some() || b.pattern != Pattern::Wildcard {
        return None;
    }
    let (Some(yes), Some(no)) = (bool_lit(&a.body), bool_lit(&b.body)) else { return None };
    if yes == no {
        return None;
    }
    let lit = |p: &Pattern| match p {
        Pattern::Lit(l) if !matches!(l, Lit::Bool(_)) => Some(bare_lit(l)),
        _ => None,
    };
    let lits = match &a.pattern {
        Pattern::Or(alts) => alts.iter().map(lit).collect::<Option<Vec<_>>>()?,
        p => vec![lit(p)?],
    };
    Some((lits.join(", "), yes))
}

fn bool_lit(e: &Expr) -> Option<bool> {
    match e {
        Expr::Lit(Lit::Bool(b)) => Some(*b),
        _ => None,
    }
}

/// `test ? then : else_`, as `||` / `&&` when either side is a `bool`
/// literal, and as `x ?? d` for `x !== null ? x : d`. The tree sets the
/// parentheses (`crate::tx`).
fn fold(test: Tx, (then, then_lit): (Tx, Option<bool>), (else_, else_lit): (Tx, Option<bool>)) -> Tx {
    match (then_lit, else_lit) {
        (Some(true), Some(false)) => test,
        (Some(false), Some(true)) => test.not(),
        (Some(true), _) => Tx::bin(Op::Or, test, else_),
        (Some(false), _) => Tx::bin(Op::And, test.not(), else_),
        (_, Some(false)) => Tx::bin(Op::And, test, then),
        (_, Some(true)) => Tx::bin(Op::Or, test.not(), then),
        _ => {
            // `unwrap_or` of a name, literal, or field: `x ?? d` is the test
            // `x !== null` with the same `x` in the Some arm (`??` keeps
            // 0/false). `x ?? null` is `x`.
            if let Tx::Bin(Op::Ne, x, null) = &test {
                if **null == Tx::atom("null") && x.print() == then.print() && x.prec() > crate::tidy::PREC_EQ {
                    return if else_.print() == "null" {
                        (**x).clone()
                    } else {
                        Tx::bin(Op::Coalesce, (**x).clone(), else_)
                    };
                }
            }
            // `!c ? a : b` is `c ? b : a`, unless `b` is a `?:` that would
            // move into the middle.
            match test {
                Tx::Not(c) if !else_.is_cond() => Tx::Cond(CondStyle::Fold, c, Box::new(else_), Box::new(then)),
                test => Tx::Cond(CondStyle::Fold, Box::new(test), Box::new(then), Box::new(else_)),
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
                bind_in(e, Expr::Field { base: Box::new(read.clone()), name: Name::new(format!("[{i}]")) }, body)?;
            }
            Some(())
        }
        _ => None,
    }
}

/// The arms in order, each a test on `scrutinee` and its body with the
/// pattern's bindings read from `scrutinee`; the last is the `else`, as
/// `check::accept` has made the `match` exhaustive.
fn match_tx(scrutinee: &Expr, arms: &[purecrate_ir::Arm], indent: usize) -> Option<Tx> {
    if arms.iter().any(|a| a.guard.is_some()) {
        return None;
    }
    // The other variants first, then one (`A | C => false, B { f } => f`):
    // tested as the one, `s.kind === "B" && s.f`, not `!(A || C) && s.f`.
    let swapped;
    let arms = match arms {
        [rest, one]
            if matches!(one.pattern, Pattern::Variant { .. })
                && matches!(&rest.pattern, Pattern::Or(alts) if alts.iter().all(|a| matches!(a, Pattern::Variant { .. })))
                && rest.pattern.bindings().is_empty() =>
        {
            swapped = [one.clone(), rest.clone()];
            &swapped[..]
        }
        _ => arms,
    };
    let subject = emit_expr(scrutinee, indent);
    let field = |base: &Expr, name: &str| Expr::Field { base: Box::new(base.clone()), name: Name::new(name) };
    let mut parts: Vec<(Option<Tx>, Tx, Option<bool>)> = Vec::new();
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
                two_way_tx(&arm.pattern, &subject)
            }
            Pattern::ResultOk(p) => {
                bind(p, field(scrutinee, "value"), &mut body)?;
                two_way_tx(&arm.pattern, &subject)
            }
            Pattern::ResultErr(p) => {
                bind(p, field(scrutinee, "error"), &mut body)?;
                two_way_tx(&arm.pattern, &subject)
            }
            Pattern::OptionNone => two_way_tx(&arm.pattern, &subject),
            Pattern::Variant { variant, bind: vb, .. } => {
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
                Some(kind_test(&subject, variant.as_str()))
            }
            // Variants that bind nothing (`Cons(_, _)` counts).
            Pattern::Or(alts)
                if alts.iter().all(|a| matches!(a, Pattern::Variant { .. }) && a.bindings().is_empty()) =>
            {
                alts.iter()
                    .filter_map(|a| match a {
                        Pattern::Variant { variant, .. } => Some(kind_test(&subject, variant.as_str())),
                        _ => None,
                    })
                    .reduce(|a, b| Tx::bin(Op::Or, a, b))
            }
            p if p.is_lit_case() => Some(lit_tx(p, &subject)?),
            _ => return None,
        };
        let lit = bool_lit(&body);
        let text = as_tx(&body, indent)?;
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

/// `o ?? (1 as U32)` as `o ?? 1`: `BigInt` takes any number, and lint
/// refuses the cast it does not need.
fn uncast_default(x: &str) -> String {
    let Some((head, last)) = x.rsplit_once(" ?? ") else { return x.to_string() };
    let literal = last
        .strip_prefix('(')
        .and_then(|l| l.strip_suffix(')'))
        .and_then(|l| l.split_once(" as "))
        .filter(|(n, t)| n.bytes().all(|b| b.is_ascii_digit()) && t.bytes().all(|b| b.is_ascii_alphanumeric()));
    match literal {
        Some((n, _)) => format!("{head} ?? {n}"),
        None => x.to_string(),
    }
}

/// `subject.kind === "V"`.
fn kind_test(subject: &str, variant: &str) -> Tx {
    Tx::bin(Op::Eq, Tx::atom(format!("{subject}.kind")), Tx::atom(format!("\"{variant}\"")))
}

/// `expr` with each read of `name` replaced by `with`. Nested binders of
/// `name` hide it, so a reused name in another arm is not rewritten.
pub(crate) fn subst(expr: &Expr, name: &Name, with: &Expr) -> Expr {
    let mut out = expr.clone();
    fn go(e: &mut Expr, name: &Name, with: &Expr) {
        match e {
            Expr::Var(n) if n == name => *e = with.clone(),
            Expr::Let { name: n, value, then, .. } => {
                go(value, name, with);
                if n != name {
                    go(then, name, with);
                }
            }
            Expr::Match { scrutinee, arms } => {
                go(scrutinee, name, with);
                for arm in arms {
                    let bound = arm.pattern.bindings().contains(&name);
                    if !bound {
                        if let Some(g) = &mut arm.guard {
                            go(g, name, with);
                        }
                        go(&mut arm.body, name, with);
                    }
                }
            }
            Expr::For { var, start, end, body, .. } => {
                go(start, name, with);
                go(end, name, with);
                if var != name {
                    go(body, name, with);
                }
            }
            Expr::ForEach { var, source, body, .. } => {
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
        Lit::Int { value, ty, byte, hex } => {
            let digits = int_digits(*value, *hex);
            match ty {
                Some(t) if t.is_big() => format!("({digits}n as {})", t.ts_name()),
                Some(t) => format!("({}{digits} as {})", byte_note(*value, *byte), t.ts_name()),
                None => digits,
            }
        }
        Lit::Float { digits, ty } => match ty {
            Some(FloatTy::F32) => f32_literal(digits),
            Some(FloatTy::F64) => format!("({} as F64)", f64_digits(digits)),
            None => f64_digits(digits),
        },
        Lit::Str(s) => js_string(s),
        Lit::Char(c) => format!("({} as Char)", js_string(&c.to_string())),
        Lit::Unit => "undefined".into(),
        Lit::Null => "null".into(),
    }
}

/// The character a byte literal wrote, before its number: `b'.'` is
/// `/* '.' */ 46`, escaped as Rust escapes it (`b'\n'`, `b'\''`). Before,
/// as oxfmt leaves a comment there; one after moves past a `)` or `;`.
fn byte_note(value: i128, byte: bool) -> String {
    match u8::try_from(value) {
        Ok(b) if byte => format!("/* '{}' */ ", b.escape_ascii()),
        _ => String::new(),
    }
}

/// An `f64` literal as written where it has 15 significant digits or fewer,
/// which every such decimal keeps through a double; else the shortest text
/// of the same double, so lint does not read a precision the value lacks
/// (`9223372036854775807.0` is `9.223372036854776e18`).
pub(crate) fn f64_digits(digits: &str) -> String {
    let mantissa = digits.split(['e', 'E']).next().unwrap_or(digits);
    let significant = mantissa.trim_start_matches(['-', '+', '0', '.']).replace('.', "");
    match digits.parse::<f64>() {
        Ok(v) if v.is_finite() && significant.trim_end_matches('0').len() > 15 => format!("{v:?}"),
        _ => digits.to_string(),
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
            c if c < ' ' || c == '\u{2028}' || c == '\u{2029}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

pub(crate) fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}

/// `&x[a..b]` and its open forms. A string goes through `Str.slice`, which
/// counts UTF-8 bytes; a `Vec` or slice is checked in Rust's order (start,
/// end, then a reversed range) with Rust's messages.
fn emit_slice(of: purecrate_ir::SliceOf, start: bool, end: bool, args: &[Expr], indent: usize) -> String {
    let base = emit_item(&args[0], indent);
    let b = end.then(|| emit_item(&args[args.len() - 1], indent));
    match of {
        purecrate_ir::SliceOf::Str => {
            let a = if start { emit_item(&args[1], indent) } else { "0 as Usize".into() };
            match b {
                Some(b) => format!("Str.slice({base}, {a}, {b})"),
                None => format!("Str.slice({base}, {a})"),
            }
        }
        // `Slice.range` takes plain numbers: a literal or a length is read
        // without its brand, which would be an unnecessary assertion there.
        purecrate_ir::SliceOf::Items => {
            let bound = |e: &Expr| bare(e, indent).unwrap_or_else(|| emit_item(e, indent));
            let a = if start { bound(&args[1]) } else { "null".into() };
            let b = if end { bound(&args[args.len() - 1]) } else { "null".into() };
            format!("Slice.range({base}, {a}, {b})")
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
        Expr::Call { callee, .. } => {
            matches!(
                callee,
                purecrate_ir::Callee::VecLen
                    | purecrate_ir::Callee::AsFloat(_)
                    | purecrate_ir::Callee::Fround
                    | purecrate_ir::Callee::IntFrom { .. }
                    | purecrate_ir::Callee::Discriminant { .. }
                    | purecrate_ir::Callee::CharCode(_)
                    | purecrate_ir::Callee::FloatConst { .. }
                    | purecrate_ir::Callee::FloatToFloat { .. }
            ) || matches!(
                callee,
                purecrate_ir::Callee::Float { m, .. } if !m.is_test() && *m != purecrate_ir::FloatMethod::Round
            ) || matches!(
                callee,
                purecrate_ir::Callee::IntToFloat { from, to } if !(from.is_big() && *to == purecrate_ir::FloatTy::F32)
            )
        }
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
/// `?:` (`(a / b) as F64`), as oxfmt prints it; a cast already in its own
/// parentheses loses them (`x as number as U8`).
fn cast_operand(x: Tx) -> String {
    match x {
        Tx::Atom(s) => crate::tidy::strip_outer(&s).to_string(),
        x if x.prec() < crate::tidy::PREC_UNARY => format!("({})", x.print()),
        x => x.print(),
    }
}

/// An argument, an element, or a field value: a comma or a brace already
/// bounds it, so a cast or an arrow needs no parentheses of its own
/// (`f(1 as I32)`, `Iter.all(xs, (b: U8): boolean => b > 0)`).
pub(crate) fn emit_item(expr: &Expr, indent: usize) -> String {
    let s = emit_expr(expr, indent);
    if is_cast(expr) || matches!(peel_identity(expr), Expr::Closure { .. } | Expr::Construct { base: Some(_), .. }) {
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
            Some(format!("{}.length", receiver(emit_tx(&args[0], indent))))
        }
        _ => None,
    }
}

/// A literal as a comparison reads it: an integer or a `char` without its
/// brand (`48`, `2n`, `"a"`); anything else as `emit_lit` prints it.
/// `value` as the source wrote it: hexadecimal where it was (`0x` and
/// lowercase digits, in whole bytes: `0x0f`), else decimal.
fn int_digits(value: i128, hex: bool) -> String {
    if !hex {
        return value.to_string();
    }
    let digits = format!("{:x}", value.unsigned_abs());
    let digits = if digits.len() % 2 == 1 { format!("0{digits}") } else { digits };
    if value < 0 {
        format!("-0x{digits}")
    } else {
        format!("0x{digits}")
    }
}

pub(crate) fn bare_lit(lit: &Lit) -> String {
    match lit {
        Lit::Int { value, ty: Some(t), hex, .. } if t.is_big() => format!("{}n", int_digits(*value, *hex)),
        Lit::Int { value, byte, hex, .. } => format!("{}{}", byte_note(*value, *byte), int_digits(*value, *hex)),
        Lit::Char(c) => js_string(&c.to_string()),
        other => emit_lit(other),
    }
}

/// A float argument to a function that takes a plain `number` (`Math`'s,
/// `Number`'s, `isInfinite`): without the `as F64` / `as F32` that
/// arithmetic carries, which would be an unnecessary assertion there.
fn float_arg(e: &Expr, indent: usize) -> String {
    match peel_identity(e) {
        Expr::Call { callee: purecrate_ir::Callee::AsFloat(_), args } => {
            crate::tidy::strip_outer(&emit_expr(&args[0], indent)).to_string()
        }
        Expr::Call { callee: purecrate_ir::Callee::Fround, args } => {
            format!("globalThis.Math.fround({})", emit_expr(&args[0], indent))
        }
        _ => emit_item(e, indent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_literal_names_its_character() {
        let byte = |value| Lit::Int { value, ty: Some(purecrate_ir::IntTy::U8), byte: true, hex: false };
        assert_eq!(bare_lit(&byte(46)), "/* '.' */ 46");
        assert_eq!(bare_lit(&byte(39)), r"/* '\'' */ 39");
        assert_eq!(bare_lit(&byte(10)), r"/* '\n' */ 10");
        assert_eq!(bare_lit(&byte(32)), "/* ' ' */ 32");
        assert_eq!(emit_lit(&byte(48)), "(/* '0' */ 48 as U8)");
        let plain = Lit::Int { value: 48, ty: Some(purecrate_ir::IntTy::U8), byte: false, hex: false };
        assert_eq!(bare_lit(&plain), "48");
    }
}
