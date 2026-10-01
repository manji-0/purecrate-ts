//! Statements: `let`, `?`, loops and jumps, and `match` as `switch` or an
//! `if` chain, each handing its value to a `Sink`.

use super::*;

/// Whether a `break` or `continue` of this loop (not of a loop inside it)
/// is in `expr`.
pub(crate) fn jumps_out(expr: &Expr) -> bool {
    match expr {
        Expr::Break | Expr::Continue => true,
        Expr::For { .. } | Expr::ForEach { .. } | Expr::While { .. } | Expr::Closure { .. } => false,
        other => other.children().into_iter().any(jumps_out),
    }
}

/// Prints a loop's head (after its `label: `, if it needs one) and body.
/// Every jump names its loop: a bare JS `break` inside the `switch` a
/// `match` prints as would leave the `switch`, not the loop.
pub(crate) fn emit_loop(head: &str, body: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let label = jumps_out(body).then(|| format!("$l{indent}"));
    let prefix = label.as_ref().map(|l| format!("{l}: ")).unwrap_or_default();
    out.push_str(&format!("{pad}{prefix}{head} {{\n"));
    LOOPS.with(|l| l.borrow_mut().push(label));
    emit_stmts(body, indent + 1, Sink::Effect, out);
    LOOPS.with(|l| l.borrow_mut().pop());
    out.push_str(&format!("{pad}}}\n"));
}

/// Printed statements that never fall through: a `break;` after them would
/// be unreachable, which a consumer's `allowUnreachableCode: false` rejects.
pub(crate) fn ends_in_jump(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) | Expr::Break | Expr::Continue => true,
        Expr::Seq { then, .. } | Expr::Let { then, .. } => ends_in_jump(then),
        Expr::If { then, else_, .. } => expr.needs_statements() && ends_in_jump(then) && ends_in_jump(else_),
        // Printed as a `switch` whose `default` returns, or an `if` chain
        // ending in `else`: it jumps when every arm does.
        Expr::Match { arms, .. } => !arms.is_empty() && arms.iter().all(|a| ends_in_jump(&a.body)),
        _ => false,
    }
}

pub(crate) fn innermost_loop() -> String {
    LOOPS
        .with(|l| l.borrow().last().cloned().flatten())
        .expect("`check::accept` puts `break` and `continue` only inside a loop that is labelled for them")
}

/// Where the value of a statement-lowered expression goes.
#[derive(Clone, Copy)]
pub(crate) enum Sink<'a> {
    Return,
    /// A `let` declared without an initializer, or a `let mut` binding.
    Assign(&'a str),
    /// The value is dropped: the expression is a Rust statement.
    Effect,
}

impl Sink<'_> {
    pub(crate) fn finish(self, value: &str, pad: &str, out: &mut String) {
        match self {
            Sink::Return => out.push_str(&format!("{pad}return {value};\n")),
            Sink::Assign(target) => out.push_str(&format!("{pad}{target} = {value};\n")),
            // A variable or `()` has no effect; anything else may panic.
            Sink::Effect if value == "undefined" || is_ident(value) => {}
            Sink::Effect => out.push_str(&format!("{pad}{value};\n")),
        }
    }

    pub(crate) fn finish_expr(self, expr: &Expr, indent: usize, out: &mut String) {
        let pad = "  ".repeat(indent);
        let value = emit_expr(expr, indent);
        match self {
            Sink::Effect if leads_with_brace(expr) => out.push_str(&format!("{pad}({value});\n")),
            _ => self.finish(&value, &pad, out),
        }
    }

    /// Unique per nesting level, so an inner temporary never shadows one an
    /// arm prelude still reads.
    pub(crate) fn temp(self, indent: usize) -> String {
        match self {
            Sink::Return | Sink::Effect => format!("{MATCH_TEMP}{indent}"),
            Sink::Assign(target) => format!("{MATCH_TEMP}{indent}_{target}"),
        }
    }
}

/// `for (let i = start, $e = end; i < $e; i = i + 1)`: the bounds are
/// evaluated once, in order, as Rust evaluates the range. `i + 1` cannot
/// overflow below `end`. A brand does not survive `+`, so the step casts
/// back to the bounds' type, which `check::accept` records.
pub(crate) fn emit_for(var: &str, ty: IntTy, start: &Expr, end: &Expr, body: &Expr, indent: usize, out: &mut String) {
    let one = if ty.is_big() { "1n" } else { "1" };
    let bound = format!("{FOR_END}{indent}");
    let head = format!(
        "for (let {var} = {}, {bound} = {}; {var} < {bound}; {var} = ({var} + {one}) as {})",
        emit_expr(start, indent),
        emit_expr(end, indent),
        ty.ts_name()
    );
    emit_loop(&head, body, indent, out);
}

/// Lower an expression into statements that hand its value to `sink`.
pub(crate) fn emit_stmts(expr: &Expr, indent: usize, sink: Sink, out: &mut String) {
    let pad = "  ".repeat(indent);
    match expr {
        Expr::Let {
            name,
            mutable,
            ty,
            value,
            then,
        } => {
            emit_let(name.as_str(), *mutable, ty.as_ref(), value, indent, out);
            emit_stmts(then, indent, sink, out);
        }
        // A guard: `if (c) return v;` on one line, when both fit on one.
        Expr::If { cond, then, else_ } if matches!(sink, Sink::Effect)
            && **else_ == Expr::Lit(Lit::Unit)
            && matches!(&**then, Expr::Return(v) if !v.needs_statements())
            && !emit_expr(cond, indent).contains('\n') =>
        {
            let Expr::Return(v) = &**then else { unreachable!("matched above") };
            out.push_str(&format!("{pad}if ({}) return {};\n", emit_expr(cond, indent), emit_expr(v, indent)));
        }
        Expr::If { cond, then, else_ } if expr.needs_statements() => {
            out.push_str(&format!("{pad}if ({}) {{\n", emit_expr(cond, indent)));
            emit_stmts(then, indent + 1, sink, out);
            if !(matches!(sink, Sink::Effect) && **else_ == Expr::Lit(Lit::Unit)) {
                out.push_str(&format!("{pad}}} else {{\n"));
                emit_stmts(else_, indent + 1, sink, out);
            }
            out.push_str(&format!("{pad}}}\n"));
        }
        Expr::Assign { name, value } => {
            if value.needs_statements() {
                emit_stmts(value, indent, Sink::Assign(name.as_str()), out);
            } else {
                out.push_str(&format!("{pad}{} = {};\n", name.as_str(), emit_expr(value, indent)));
            }
            sink.finish("undefined", &pad, out);
        }
        Expr::Seq { first, then } => {
            emit_stmts(first, indent, Sink::Effect, out);
            let dead = matches!(**first, Expr::Return(_)) && **then == Expr::Lit(Lit::Unit);
            if !dead {
                emit_stmts(then, indent, sink, out);
            }
        }
        // A predicate (`matches!`, or every arm `true` or `false`) on a
        // place reads better as the test than as a `switch`.
        Expr::Match { scrutinee, arms }
            if !matches!(sink, Sink::Effect)
                && is_place(scrutinee)
                && arms.iter().all(|a| matches!(a.body, Expr::Lit(Lit::Bool(_)))) =>
        {
            match as_expr(expr, indent) {
                Some(value) => sink.finish(&value, &pad, out),
                None => emit_switch(scrutinee, arms, indent, sink, out),
            }
        }
        Expr::Match { scrutinee, arms } => emit_switch(scrutinee, arms, indent, sink, out),
        Expr::For { var, ty, start, end, body } => {
            let ty = ty.expect("check::accept types the range");
            emit_for(var.as_str(), ty, start, end, body, indent, out);
            sink.finish("undefined", &pad, out);
        }
        Expr::ForEach { var, over, source: string, body } => {
            let iterable = iterable(*over, emit_expr(string, indent));
            emit_loop(&format!("for (const {} of {iterable})", var.as_str()), body, indent, out);
            sink.finish("undefined", &pad, out);
        }
        Expr::While { cond, body } => {
            emit_loop(&format!("while ({})", emit_expr(cond, indent)), body, indent, out);
            sink.finish("undefined", &pad, out);
        }
        Expr::Break => out.push_str(&format!("{pad}break {};\n", innermost_loop())),
        Expr::Continue => out.push_str(&format!("{pad}continue {};\n", innermost_loop())),
        Expr::Return(value) => out.push_str(&format!("{pad}return {};\n", emit_expr(value, indent))),
        // `x?;`: only the early return; there is no value to bind.
        // `x?;` on a binding: the test alone.
        Expr::Try { expr: inner, on } if matches!(sink, Sink::Effect) && matches!(**inner, Expr::Var(_)) => {
            emit_try_test(&emit_expr(inner, indent), *on, indent, out);
        }
        Expr::Try { expr: inner, on } if matches!(sink, Sink::Effect) => {
            emit_try_exit(&format!("{TRY_LET_TEMP}{TRY_TEMP}{indent}"), inner, *on, indent, out);
        }
        Expr::Try { .. } => {
            let tmp = format!("{TRY_TEMP}{indent}");
            emit_let(&tmp, false, None, expr, indent, out);
            sink.finish(&tmp, &pad, out);
        }
        other => sink.finish_expr(other, indent, out),
    }
}

/// What `for` and the consuming methods walk, from the source's text.
pub(crate) fn iterable(over: purecrate_ir::Over, source: String) -> String {
    match over {
        // A JS string iterates by code point, as `chars` does by scalar
        // value; the two agree on well-formed strings (design/01 §6).
        purecrate_ir::Over::Chars => format!("({source} as Iterable<Char>)"),
        // The UTF-8 bytes, as `as_bytes` reads them.
        purecrate_ir::Over::Bytes => format!("Str.bytes({source})"),
        purecrate_ir::Over::Items => source,
    }
}

pub(crate) fn emit_let(name: &str, mutable: bool, ty: Option<&Ty>, value: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let keyword = if mutable { "let" } else { "const" };
    let annotation = ty.map(|t| format!(": {}", emit_ty(t))).unwrap_or_default();
    match value {
        Expr::Try { expr, on } => {
            let tmp = format!("{TRY_LET_TEMP}{name}");
            emit_try_exit(&tmp, expr, *on, indent, out);
            let payload = match on {
                Some(TryOn::Option) => tmp,
                Some(TryOn::Result) | None => format!("{tmp}.value"),
            };
            out.push_str(&format!("{pad}{keyword} {name}{annotation} = {payload};\n"));
        }
        v if v.needs_statements() => {
            out.push_str(&format!("{pad}let {name}{annotation};\n"));
            if matches!(v, Expr::Let { .. } | Expr::Seq { .. }) {
                // A Rust block: its bindings end with it.
                out.push_str(&format!("{pad}{{\n"));
                emit_stmts(v, indent + 1, Sink::Assign(name), out);
                out.push_str(&format!("{pad}}}\n"));
            } else {
                emit_stmts(v, indent, Sink::Assign(name), out);
            }
        }
        // An annotation would narrow the union to this variant, or to what
        // an enclosing `switch` left of the place, and a later `switch` or
        // `=== null` on the binding could not name the rest.
        v if ty.is_some_and(is_union) && (matches!(v, Expr::Construct { variant: Some(_), .. }) || is_place(v)) => out.push_str(&format!(
            "{pad}{keyword} {name} = {} as {};\n",
            emit_expr(v, indent),
            emit_ty(ty.expect("checked"))
        )),
        v => out.push_str(&format!(
            "{pad}{keyword} {name}{annotation} = {};\n",
            emit_expr(v, indent)
        )),
    }
}

/// `const tmp = expr;` and the early return of `expr?` when it holds `None`
/// or an `Err`.
pub(crate) fn emit_try_exit(tmp: &str, expr: &Expr, on: Option<TryOn>, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    out.push_str(&format!("{pad}const {tmp} = {};\n", emit_expr(expr, indent)));
    emit_try_test(tmp, on, indent, out);
}

/// The early return of `tmp?` when `tmp` holds `None` or an `Err`.
fn emit_try_test(tmp: &str, on: Option<TryOn>, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match on {
        Some(TryOn::Option) => out.push_str(&format!("{pad}if ({tmp} === null) return null;\n")),
        Some(TryOn::Result) | None => out.push_str(&format!("{pad}if ({tmp}.kind === \"Err\") return {tmp};\n")),
    }
}

/// A type TS narrows: an enum (or any named type, which may be one),
/// `Option`, or `Result`.
pub(crate) fn is_union(ty: &Ty) -> bool {
    matches!(ty, Ty::Named(_) | Ty::Option(_) | Ty::Result { .. })
}

/// `x`, `x.a.b`: references TS can narrow through `switch (x.kind)`.
pub(crate) fn is_place(expr: &Expr) -> bool {
    match expr {
        Expr::Var(_) => true,
        Expr::Field { base, .. } => is_place(base),
        _ => false,
    }
}

pub(crate) fn peel(expr: &Expr) -> &Expr {
    match expr {
        Expr::Ignored { expr, .. } => peel(expr),
        other => other,
    }
}

/// Without an annotation a variant literal widens to `{ kind: string }`.
pub(crate) fn scrutinee_ty(arms: &[purecrate_ir::Arm]) -> Option<&purecrate_ir::Name> {
    arms.iter().find_map(|arm| match &arm.pattern {
        Pattern::Variant { ty, .. } => Some(ty),
        Pattern::Or(alts) => alts.iter().find_map(|alt| match alt {
            Pattern::Variant { ty, .. } => Some(ty),
            _ => None,
        }),
        _ => None,
    })
}

pub(crate) fn declares_at_top(expr: &Expr) -> bool {
    match expr {
        Expr::Let { .. } | Expr::Try { .. } | Expr::Seq { .. } | Expr::Assign { .. } => true,
        Expr::Match { scrutinee, .. } => !is_place(scrutinee),
        _ => false,
    }
}

pub(crate) fn emit_switch(
    scrutinee: &Expr,
    arms: &[purecrate_ir::Arm],
    indent: usize,
    sink: Sink,
    out: &mut String,
) {
    // Statements after this one may hold another `match` at the same depth;
    // a block keeps the two temporaries apart.
    if !is_place(scrutinee) && !matches!(sink, Sink::Return) {
        let pad = "  ".repeat(indent);
        out.push_str(&format!("{pad}{{\n"));
        emit_switch_in(scrutinee, arms, indent + 1, sink, out);
        out.push_str(&format!("{pad}}}\n"));
    } else {
        emit_switch_in(scrutinee, arms, indent, sink, out);
    }
}

pub(crate) fn emit_switch_in(
    scrutinee: &Expr,
    arms: &[purecrate_ir::Arm],
    indent: usize,
    sink: Sink,
    out: &mut String,
) {
    assert!(arms.iter().all(|a| a.guard.is_none()), "guards are lowered in check::accept");
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    let subject = if is_place(scrutinee) {
        emit_expr(scrutinee, indent)
    } else {
        let tmp = sink.temp(indent);
        let value = emit_expr(scrutinee, indent);
        // An annotated literal initializer narrows the union and makes the
        // other cases unreachable; a cast keeps the full union.
        let decl = match (scrutinee_ty(arms), peel(scrutinee)) {
            (Some(ty), Expr::Construct { .. }) => format!("{tmp} = {value} as {}", ty.as_str()),
            (Some(ty), _) => format!("{tmp}: {} = {value}", ty.as_str()),
            (None, _) => format!("{tmp} = {value}"),
        };
        out.push_str(&format!("{pad}const {decl};\n"));
        tmp
    };
    if let [a, b] = arms {
        if let (Some(test), Some(_)) = (two_way_test(&a.pattern, &subject), two_way_test(&b.pattern, &subject)) {
            let pad2 = "  ".repeat(indent + 1);
            out.push_str(&format!("{pad}if ({test}) {{\n"));
            out.push_str(&two_way_prelude(&a.pattern, &subject, &pad2));
            emit_stmts(&a.body, indent + 1, sink, out);
            if !(matches!(sink, Sink::Effect) && b.body == Expr::Lit(Lit::Unit)) {
                out.push_str(&format!("{pad}}} else {{\n"));
                out.push_str(&two_way_prelude(&b.pattern, &subject, &pad2));
                emit_stmts(&b.body, indent + 1, sink, out);
            }
            out.push_str(&format!("{pad}}}\n"));
            return;
        }
    }
    if arms.iter().any(|a| a.pattern.is_lit_case()) {
        emit_lit_chain(&subject, arms, indent, sink, out);
        return;
    }
    out.push_str(&format!("{pad}switch ({subject}.kind) {{\n"));
    for arm in arms {
        // `A | B` binds nothing: its cases share one body.
        let (variants, bind) = match &arm.pattern {
            Pattern::Variant { variant, bind, .. } => (vec![variant], Some(bind)),
            Pattern::Or(alts) => (
                alts.iter()
                    .filter_map(|alt| match alt {
                        Pattern::Variant { variant, .. } => Some(variant),
                        _ => None,
                    })
                    .collect(),
                None,
            ),
            _ => continue,
        };
        if let Some((last, first)) = variants.split_last() {
            for v in first {
                out.push_str(&format!("{pad1}case \"{v}\":\n", v = v.as_str()));
            }
            out.push_str(&format!("{pad1}case \"{v}\":", v = last.as_str()));
            let prelude = bind
                .map(|bind| bind_prelude(bind, &subject, &"  ".repeat(indent + 2)))
                .unwrap_or_default();
            let braced = !prelude.is_empty() || declares_at_top(&arm.body);
            out.push_str(if braced { " {\n" } else { "\n" });
            out.push_str(&prelude);
            emit_stmts(&arm.body, indent + 2, sink, out);
            if !matches!(sink, Sink::Return) && !ends_in_jump(&arm.body) {
                out.push_str(&format!("{pad1}  break;\n"));
            }
            if braced {
                out.push_str(&format!("{pad1}}}\n"));
            }
        }
    }
    out.push_str(&format!(
        "{pad1}default:\n{pad1}  return assertNever({subject});\n{pad}}}\n"
    ));
}

/// A `match` on an integer or a `&str`: arms tried in order, the last (`_`)
/// as `else`. Well-formed strings are equal in UTF-8 exactly when they are in
/// UTF-16, so `===` is `str` equality.
pub(crate) fn emit_lit_chain(subject: &str, arms: &[purecrate_ir::Arm], indent: usize, sink: Sink, out: &mut String) {
    let pad = "  ".repeat(indent);
    for (i, arm) in arms.iter().enumerate() {
        let head = match (i, lit_test(&arm.pattern, subject)) {
            (0, Some(test)) => format!("{pad}if ({test}) {{\n"),
            (_, Some(test)) => format!("{pad}}} else if ({test}) {{\n"),
            (_, None) => format!("{pad}}} else {{\n"),
        };
        out.push_str(&head);
        emit_stmts(&arm.body, indent + 1, sink, out);
    }
    out.push_str(&format!("{pad}}}\n"));
}

/// The test for an integer, `char`, or string arm; `None` for `_`.
pub(crate) fn lit_test(pattern: &Pattern, subject: &str) -> Option<String> {
    match pattern {
        Pattern::Lit(lit) => Some(format!("{subject} === {}", emit_lit(lit))),
        // By code point: JS orders strings by UTF-16 unit.
        Pattern::Range { lo: Lit::Char(lo), hi: Lit::Char(hi), inclusive } => Some(format!(
            "Char.code({subject}) >= {} && Char.code({subject}) {} {}",
            u32::from(*lo),
            if *inclusive { "<=" } else { "<" },
            u32::from(*hi)
        )),
        Pattern::Range { lo, hi, inclusive } => Some(format!(
            "{subject} >= {} && {subject} {} {}",
            emit_lit(lo),
            if *inclusive { "<=" } else { "<" },
            emit_lit(hi)
        )),
        Pattern::Or(alts) => Some(
            alts.iter()
                .filter_map(|a| match a {
                    Pattern::Range { .. } => lit_test(a, subject).map(|t| format!("({t})")),
                    _ => lit_test(a, subject),
                })
                .collect::<Vec<_>>()
                .join(" || "),
        ),
        _ => None,
    }
}

/// `Option` is `T | null` and `Result` is `kind`-tagged, so both narrow
/// with a single test.
pub(crate) fn two_way_test(pattern: &Pattern, subject: &str) -> Option<String> {
    match pattern {
        Pattern::OptionSome(_) => Some(format!("{subject} !== null")),
        Pattern::OptionNone => Some(format!("{subject} === null")),
        Pattern::ResultOk(_) => Some(format!("{subject}.kind === \"Ok\"")),
        Pattern::ResultErr(_) => Some(format!("{subject}.kind === \"Err\"")),
        _ => None,
    }
}

pub(crate) fn two_way_prelude(pattern: &Pattern, subject: &str, pad: &str) -> String {
    let (inner, read) = match pattern {
        Pattern::OptionSome(p) => (p, subject.to_string()),
        Pattern::ResultOk(p) => (p, format!("{subject}.value")),
        Pattern::ResultErr(p) => (p, format!("{subject}.error")),
        _ => return String::new(),
    };
    match &**inner {
        Pattern::Var(n) => format!("{pad}const {} = {read};\n", n.as_str()),
        _ => String::new(),
    }
}

pub(crate) fn bind_prelude(bind: &VariantBind, subject: &str, pad: &str) -> String {
    match bind {
        VariantBind::Unit => String::new(),
        VariantBind::Tuple(pats) => pats
            .iter()
            .enumerate()
            .filter_map(|(i, p)| match p {
                Pattern::Var(n) => Some(format!(
                    "{pad}const {name} = {subject}.content[{i}];\n",
                    name = n.as_str()
                )),
                _ => None,
            })
            .collect(),
        VariantBind::Struct(pairs) => pairs
            .iter()
            .filter_map(|(field, p)| match p {
                Pattern::Var(n) => Some(format!(
                    "{pad}const {name} = {subject}.{f};\n",
                    name = n.as_str(),
                    f = field.as_str()
                )),
                _ => None,
            })
            .collect(),
    }
}
