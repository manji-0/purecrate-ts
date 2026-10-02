//! Statements: `let`, `?`, loops and jumps, and `match` as `switch` or an
//! `if` chain, each handing its value to a `Sink`.

use super::*;

/// Whether a `break` or `continue` of this loop (not of a loop inside it)
/// is in `expr`.
pub(crate) fn jumps_out(expr: &Expr) -> bool {
    match expr {
        Expr::Break | Expr::Continue => true,
        Expr::For { .. } | Expr::ForEach { .. } | Expr::While { .. } | Expr::Closure { .. } => {
            false
        }
        other => other.children().into_iter().any(jumps_out),
    }
}

/// Prints a loop's head (after its `label: `, if it needs one) and body.
/// Every jump names its loop: a bare JS `break` inside the `switch` a
/// `match` prints as would leave the `switch`, not the loop.
pub(crate) fn emit_loop(head: &str, body: &Expr, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let label = jumps_out(body).then(|| temp("loop", indent));
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
        Expr::If { then, else_, .. } => {
            expr.needs_statements() && ends_in_jump(then) && ends_in_jump(else_)
        }
        // Printed as a `switch` whose `default` returns, or an `if` chain
        // ending in `else`: it jumps when every arm does.
        Expr::Match { arms, .. } => !arms.is_empty() && arms.iter().all(|a| ends_in_jump(&a.body)),
        _ => false,
    }
}

pub(crate) fn innermost_loop() -> String {
    LOOPS.with(|l| l.borrow().last().cloned().flatten()).expect(
        "`check::accept` puts `break` and `continue` only inside a loop that is labelled for them",
    )
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
            Sink::Return => out.push_str(&format!(
                "{pad}return {};\n",
                crate::tidy::strip_outer(value)
            )),
            Sink::Assign(target) => out.push_str(&format!(
                "{pad}{target} = {};\n",
                crate::tidy::strip_outer(value)
            )),
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

}

/// The matched value of a `match` on something that is not a place, named
/// for what the arms test: the enum (`otpCheck`), `result`, `option`, or
/// `value`.
pub(crate) fn match_temp(arms: &[purecrate_ir::Arm], indent: usize) -> String {
    let base = match scrutinee_ty(arms) {
        Some(ty) => purecrate_ir::to_camel(&lower_first(ty.as_str())),
        None => match arms.iter().map(|a| &a.pattern).find(|p| !matches!(p, Pattern::Wildcard)) {
            Some(Pattern::ResultOk(_) | Pattern::ResultErr(_)) => "result".to_string(),
            Some(Pattern::OptionSome(_) | Pattern::OptionNone) => "option".to_string(),
            _ => "value".to_string(),
        },
    };
    temp(&base, indent)
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// `for (let i = start, $e = end; i < $e; i = i + 1)`: the bounds are
/// evaluated once, in order, as Rust evaluates the range. `i + 1` cannot
/// overflow below `end`. A brand does not survive `+`, so the step casts
/// back to the bounds' type, which `check::accept` records.
pub(crate) fn emit_for(
    var: &str,
    ty: IntTy,
    start: &Expr,
    end: &Expr,
    body: &Expr,
    indent: usize,
    out: &mut String,
) {
    let one = if ty.is_big() { "1n" } else { "1" };
    // A literal end is the same value however often it is read.
    let head = if matches!(peel(end), Expr::Lit(_)) {
        format!(
            "for (let {var} = {}; {var} < {}; {var} = ({var} + {one}) as {})",
            emit_item(start, indent),
            match peel(end) {
                Expr::Lit(lit) => bare_lit(lit),
                _ => unreachable!("matched above"),
            },
            ty.ts_name()
        )
    } else {
        let bound = temp("end", indent);
        format!(
            "for (let {var} = {}, {bound} = {}; {var} < {bound}; {var} = ({var} + {one}) as {})",
            emit_item(start, indent),
            emit_item(end, indent),
            ty.ts_name()
        )
    };
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
            // `let $x: T = e; $x` names `T` for `collect::<T>()` / `sum::<T>()`.
            // `e` is the result; the binding would only be copied into the sink.
            if !*mutable
                && matches!(then.as_ref(), Expr::Var(n) if n == name)
                && !peel_identity(value).needs_statements()
            {
                emit_stmts(peel_identity(value), indent, sink, out);
            } else if let Some((slots, rest)) = (!*mutable && name.as_str().starts_with('$'))
                .then(|| destructured(name, then))
                .flatten()
                .filter(|_| value_expr(value, indent).is_some())
            {
                // `let (a, b) = v`: the tuple is only taken apart.
                let (prelude, s) = value_expr(value, indent).expect("checked above");
                let pattern = slots.iter().map(|n| n.map_or("", |n| n.as_str())).collect::<Vec<_>>().join(", ");
                // The pattern types an array literal as a tuple; only an
                // object literal would widen (`kind: string`) without one.
                let annotation = ty
                    .as_ref()
                    .filter(|t| holds_object(t))
                    .map(|t| format!(": {}", emit_ty(t)))
                    .unwrap_or_default();
                out.push_str(&prelude);
                out.push_str(&format!("{pad}const [{pattern}]{annotation} = {};\n", crate::tidy::strip_outer(&s)));
                emit_stmts(rest, indent, sink, out);
            } else if !*mutable && name.as_str().starts_with('$') && value.is_inlinable() {
                // `$opt = x` / `$optOr = d` around `unwrap_or` / `ok_or`: the
                // names and variant literals cannot panic, so the uses read
                // them directly (eager evaluation of a call still binds).
                emit_stmts(&subst(then, name, value), indent, sink, out);
            } else {
                emit_let(name.as_str(), *mutable, ty.as_ref(), value, indent, out);
                emit_stmts(then, indent, sink, out);
            }
        }
        // A guard: `if (c) return v;` on one line, when both fit on one.
        Expr::If { cond, then, else_ }
            if matches!(sink, Sink::Effect)
                && **else_ == Expr::Lit(Lit::Unit)
                && matches!(&**then, Expr::Return(v) if !v.needs_statements())
                && !emit_expr(cond, indent).contains('\n') =>
        {
            let Expr::Return(v) = &**then else {
                unreachable!("matched above")
            };
            out.push_str(&format!(
                "{pad}if ({}) return {};\n",
                crate::tidy::strip_outer(&emit_expr(cond, indent)),
                crate::tidy::strip_outer(&emit_expr(v, indent))
            ));
        }
        // `if matches!(f(x), p) ..` (or `!matches!`): the matched value is
        // the first thing the condition evaluates, so it is bound before the
        // `if` and the test reads the binding, not an inline function.
        Expr::If { cond, then, else_ } if hoistable_test(cond).is_some() => {
            let (scrutinee, arms, negated) = hoistable_test(cond).expect("checked above");
            let tmp = match_temp(arms, indent);
            let on_tmp = Expr::Match { scrutinee: Box::new(Expr::Var(Name::new(tmp.clone()))), arms: arms.to_vec() };
            // A test that does not read the value (every arm `true`) still
            // evaluates it, for what it may panic on.
            if emit_expr(&on_tmp, indent).contains(tmp.as_str()) {
                bind_scrutinee(&tmp, scrutinee, arms, indent, out);
            } else {
                Sink::Effect.finish_expr(scrutinee, indent, out);
            }
            let test = if negated {
                Expr::Unary { op: purecrate_ir::UnOp::Not, expr: Box::new(on_tmp) }
            } else {
                on_tmp
            };
            let rewritten = Expr::If { cond: Box::new(test), then: then.clone(), else_: else_.clone() };
            emit_stmts(&rewritten, indent, sink, out);
        }
        // A value of one side, the other returning: the exit, then the
        // value. Only an expression stays, so no binding joins the block.
        Expr::If { cond, then, else_ }
            if matches!(sink, Sink::Assign(_))
                && (ends_in_jump(then) && !else_.needs_statements()
                    || ends_in_jump(else_) && !then.needs_statements()) =>
        {
            let c = crate::tidy::strip_outer(&emit_expr(cond, indent)).to_string();
            let (test, exit, keep) = if ends_in_jump(then) { (c, then, else_) } else { (not(&c), else_, then) };
            emit_guard(&test, "", exit, indent, Sink::Effect, out);
            sink.finish_expr(keep, indent, out);
        }
        // `if c {} else { .. }` as a statement: the test turned round.
        Expr::If { cond, then, else_ }
            if matches!(sink, Sink::Effect) && **then == Expr::Lit(Lit::Unit) && **else_ != Expr::Lit(Lit::Unit) =>
        {
            let test = not(crate::tidy::strip_outer(&emit_expr(cond, indent)));
            emit_guard(&test, "", else_, indent, sink, out);
        }
        Expr::If { cond, then, else_ } if expr.needs_statements() => {
            let test = crate::tidy::strip_outer(&emit_expr(cond, indent)).to_string();
            let mut branches = vec![Branch { test: Some(test), prelude: String::new(), body: then }];
            // An `else` that is one more `if` with statements is `else if`.
            let mut rest = &**else_;
            while let Expr::If { cond, then, else_ } = rest {
                if !rest.needs_statements() {
                    break;
                }
                let test = crate::tidy::strip_outer(&emit_expr(cond, indent)).to_string();
                branches.push(Branch { test: Some(test), prelude: String::new(), body: then });
                rest = else_;
            }
            if !(matches!(sink, Sink::Effect) && *rest == Expr::Lit(Lit::Unit)) {
                branches.push(Branch { test: None, prelude: String::new(), body: rest });
            }
            emit_branches(&branches, indent, sink, out);
        }
        Expr::Assign { name, value } => {
            if value.needs_statements() {
                emit_stmts(value, indent, Sink::Assign(name.as_str()), out);
            } else {
                out.push_str(&format!(
                    "{pad}{} = {};\n",
                    name.as_str(),
                    emit_expr(value, indent)
                ));
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
        // A predicate (`matches!`) or `unwrap_or` / `ok_or` on a place is one
        // expression (`x ?? d`, a `?:`); anything larger is a `switch`.
        Expr::Match { scrutinee, arms }
            if !matches!(sink, Sink::Effect)
                && is_place(scrutinee)
                && (arms.iter().all(|a| matches!(a.body, Expr::Lit(Lit::Bool(_))))
                    || (arms.len() == 2 && two_way_test(&arms[0].pattern, "").is_some())) =>
        {
            match as_expr(expr, indent) {
                Some(value) => sink.finish(&value, &pad, out),
                None => emit_switch(scrutinee, arms, indent, sink, out),
            }
        }
        Expr::Match { scrutinee, arms } => emit_switch(scrutinee, arms, indent, sink, out),
        Expr::For {
            var,
            ty,
            start,
            end,
            body,
        } => {
            let ty = ty.expect("check::accept types the range");
            emit_for(var.as_str(), ty, start, end, body, indent, out);
            sink.finish("undefined", &pad, out);
        }
        Expr::ForEach {
            var,
            over,
            source: string,
            body,
        } => {
            let iterable = iterable(*over, emit_expr(string, indent));
            emit_loop(
                &format!("for (const {} of {iterable})", var.as_str()),
                body,
                indent,
                out,
            );
            sink.finish("undefined", &pad, out);
        }
        Expr::While { cond, body } => {
            emit_loop(
                &format!(
                    "while ({})",
                    crate::tidy::strip_outer(&emit_expr(cond, indent))
                ),
                body,
                indent,
                out,
            );
            sink.finish("undefined", &pad, out);
        }
        Expr::Break => out.push_str(&format!("{pad}break {};\n", innermost_loop())),
        Expr::Continue => out.push_str(&format!("{pad}continue {};\n", innermost_loop())),
        // `return match ..`: the `match` returns from each arm.
        Expr::Return(value) if value.needs_statements() && as_expr(value, indent).is_none() => {
            emit_stmts(value, indent, Sink::Return, out)
        }
        Expr::Return(value) => out.push_str(&format!(
            "{pad}return {};\n",
            crate::tidy::strip_outer(&emit_expr(value, indent))
        )),
        // `x?;`: only the early return; there is no value to bind.
        // `x?;` on a binding: the test alone.
        Expr::Try { expr: inner, on }
            if matches!(sink, Sink::Effect) && matches!(**inner, Expr::Var(_)) =>
        {
            emit_try_test(&emit_expr(inner, indent), *on, indent, out);
        }
        Expr::Try { expr: inner, on } if matches!(sink, Sink::Effect) => {
            emit_try_exit(&try_temp(None, *on, indent), inner, *on, indent, out);
        }
        Expr::Try { .. } => {
            let tmp = temp("value", indent);
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

pub(crate) fn emit_let(
    name: &str,
    mutable: bool,
    ty: Option<&Ty>,
    value: &Expr,
    indent: usize,
    out: &mut String,
) {
    // `collect::<T>()` and `sum::<T>()` are a typed `let` whose body is the
    // binding. Nested under another `let`, that binding is only a copy.
    let value = peel_identity(value);
    let pad = "  ".repeat(indent);
    let keyword = if mutable { "let" } else { "const" };
    // An arrow states its own type, and a `let` of `true` or `false` is a
    // `boolean` without one.
    let inferred = matches!(value, Expr::Closure { .. })
        || (mutable && matches!(value, Expr::Lit(Lit::Bool(_))) && matches!(ty, Some(t) if *t == Ty::bool()));
    let annotation = ty.filter(|_| !inferred).map(|t| format!(": {}", emit_ty(t))).unwrap_or_default();
    match value {
        Expr::Var(n) if n.as_str() == name => {}
        Expr::Try { expr, on } => {
            let tmp = try_temp(Some(name), *on, indent);
            emit_try_exit(&tmp, expr, *on, indent, out);
            let payload = match on {
                Some(TryOn::Option) => tmp,
                Some(TryOn::Result) | None => format!("{tmp}.value"),
            };
            out.push_str(&format!("{pad}{keyword} {name}{annotation} = {payload};\n"));
        }
        // A place in a `switch` arm is a narrowed union; the annotation
        // alone does not widen it for TS, so `as T` gives back the type. A
        // struct is no union: narrowing only drops a `null`, which the
        // annotation already says.
        v if is_place(v) && matches!(ty, Some(Ty::Named(n)) if !crate::is_struct(n.as_str())) => {
            let t = emit_ty(ty.unwrap());
            out.push_str(&format!(
                "{pad}{keyword} {name}{annotation} = {} as {t};\n",
                emit_expr(v, indent)
            ));
        }
        v if v.needs_statements() && let_else(name, mutable, ty, v, indent, out) => {}
        v if let Some((prelude, s)) = value_expr(v, indent) => {
            out.push_str(&prelude);
            out.push_str(&format!("{pad}{keyword} {name}{annotation} = {};\n", crate::tidy::strip_outer(&s)));
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
        v => out.push_str(&format!(
            "{pad}{keyword} {name}{annotation} = {};\n",
            crate::tidy::strip_outer(&emit_expr(v, indent))
        )),
    }
}

/// What `x?` is held in before its test: `<name>Result` or `<name>Option`
/// for `let name = x?`, else `result` or `option`.
fn try_temp(name: Option<&str>, on: Option<TryOn>, indent: usize) -> String {
    let what = if on == Some(TryOn::Option) { "option" } else { "result" };
    // A temporary's own name (`$value_2`) says nothing; its `_<depth>` is kept.
    match name.filter(|n| !n.starts_with('$')) {
        Some(n) => {
            // A shadow `n$1` prints as `n2`: its temporary is `n2Result`.
            let n = match n.rsplit_once('$') {
                Some((b, k)) if k.bytes().all(|c| c.is_ascii_digit()) => {
                    format!("{b}{}", k.parse::<usize>().map_or(0, |k| k + 1))
                }
                _ => n.to_string(),
            };
            format!("${n}{}{}", what[..1].to_uppercase(), &what[1..])
        }
        None => temp(what, indent),
    }
}

/// `const tmp = expr;` and the early return of `expr?` when it holds `None`
/// or an `Err`.
pub(crate) fn emit_try_exit(
    tmp: &str,
    expr: &Expr,
    on: Option<TryOn>,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    out.push_str(&format!(
        "{pad}const {tmp} = {};\n",
        emit_expr(expr, indent)
    ));
    emit_try_test(tmp, on, indent, out);
}

/// The early return of `tmp?` when `tmp` holds `None` or an `Err`.
fn emit_try_test(tmp: &str, on: Option<TryOn>, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match on {
        Some(TryOn::Option) => out.push_str(&format!("{pad}if ({tmp} === null) return null;\n")),
        Some(TryOn::Result) | None => {
            out.push_str(&format!("{pad}if ({tmp}.kind === \"Err\") return {tmp};\n"))
        }
    }
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

/// `let $x = e; $x` when `e` is already an expression: the binding names a
/// type (`collect::<T>()`, `sum::<T>()`) and is not a second evaluation.
pub(crate) fn peel_identity(expr: &Expr) -> &Expr {
    match expr {
        Expr::Let {
            name,
            mutable: false,
            value,
            then,
            ..
        } if matches!(then.as_ref(), Expr::Var(n) if n == name) && !value.needs_statements() => {
            peel_identity(value)
        }
        _ => expr,
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

/// `const tmp = scrutinee;` before a `match` on a value that is not a place.
fn bind_scrutinee(tmp: &str, scrutinee: &Expr, arms: &[purecrate_ir::Arm], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let value = emit_expr(scrutinee, indent);
    // An annotated literal initializer narrows the union and makes the
    // other cases unreachable; a cast keeps the full union.
    let decl = match (scrutinee_ty(arms), peel(scrutinee)) {
        (Some(ty), Expr::Construct { .. }) => format!("{tmp} = {value} as {}", ty.as_str()),
        (Some(ty), _) => format!("{tmp}: {} = {value}", ty.as_str()),
        (None, _) => format!("{tmp} = {}", crate::tidy::strip_outer(&value)),
    };
    out.push_str(&format!("{pad}const {decl};\n"));
}

/// `value` as one expression, after the statements (`prelude`) that bind
/// what it reads once: a `match` on a value that is not a place, each arm an
/// expression, binds the value and is a `?:` on that binding.
pub(crate) fn value_expr(value: &Expr, indent: usize) -> Option<(String, String)> {
    let value = peel_identity(value);
    if let Some(s) = as_expr(value, indent) {
        return Some((String::new(), s));
    }
    let Expr::Match { scrutinee, arms } = value else { return None };
    if is_place(scrutinee) {
        return None;
    }
    let tmp = match_temp(arms, indent);
    let on_tmp = Expr::Match { scrutinee: Box::new(Expr::Var(Name::new(tmp.clone()))), arms: arms.clone() };
    let s = as_expr(&on_tmp, indent)?;
    let mut prelude = String::new();
    bind_scrutinee(&tmp, scrutinee, arms, indent, &mut prelude);
    Some((prelude, s))
}

/// `let t = v; let a = t[0]; let b = t[1]; ..` where nothing else reads
/// `t`: the names by element (`None` for one no name takes), and the rest.
fn destructured<'e>(t: &Name, mut then: &'e Expr) -> Option<(Vec<Option<&'e Name>>, &'e Expr)> {
    let mut slots: Vec<Option<&Name>> = Vec::new();
    while let Expr::Let { name, mutable: false, value, then: rest, .. } = then {
        let Expr::Field { base, name: field } = value.as_ref() else { break };
        let Some(i) = field.as_str().strip_prefix('[').and_then(|f| f.strip_suffix(']')).and_then(|f| f.parse::<usize>().ok())
        else {
            break;
        };
        if !matches!(base.as_ref(), Expr::Var(n) if n == t) {
            break;
        }
        if slots.len() <= i {
            slots.resize(i + 1, None);
        }
        if slots[i].is_some() {
            break;
        }
        slots[i] = Some(name);
        then = rest;
    }
    (slots.iter().filter(|s| s.is_some()).count() >= 2 && !mentions(then, t)).then_some((slots, then))
}

/// `matches!(v, p)` or `!matches!(v, p)` on a value that is not a place,
/// each arm an expression (a guard's included): the value, the arms, and
/// whether it is negated.
fn hoistable_test(cond: &Expr) -> Option<(&Expr, &[purecrate_ir::Arm], bool)> {
    let (inner, negated) = match peel_identity(cond) {
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => (peel_identity(expr), true),
        other => (other, false),
    };
    match inner {
        Expr::Match { scrutinee, arms }
            if !is_place(scrutinee) && arms.iter().all(|a| !a.body.needs_statements()) =>
        {
            Some((scrutinee, arms, negated))
        }
        _ => None,
    }
}

/// A type whose values may be object literals: a crate type or a `Result`.
fn holds_object(ty: &Ty) -> bool {
    match ty {
        Ty::Named(_) | Ty::Result { .. } | Ty::Fn { .. } => true,
        Ty::Option(t) | Ty::Vec(t) | Ty::Ignored { inner: t, .. } => holds_object(t),
        Ty::Tuple(ts) => ts.iter().any(holds_object),
        Ty::Prim(_) | Ty::Never => false,
    }
}

/// Whether `name` is read or assigned anywhere in `expr`.
fn mentions(expr: &Expr, name: &Name) -> bool {
    match expr {
        Expr::Var(n) | Expr::Assign { name: n, .. } if n == name => true,
        Expr::Call { callee: purecrate_ir::Callee::Local(n), .. } if n == name => true,
        other => other.children().into_iter().any(|c| mentions(c, name)),
    }
}

pub(crate) fn emit_switch_in(
    scrutinee: &Expr,
    arms: &[purecrate_ir::Arm],
    indent: usize,
    sink: Sink,
    out: &mut String,
) {
    assert!(
        arms.iter().all(|a| a.guard.is_none()),
        "guards are lowered in check::accept"
    );
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    let subject = if is_place(scrutinee) {
        emit_expr(scrutinee, indent)
    } else {
        let tmp = match_temp(arms, indent);
        bind_scrutinee(&tmp, scrutinee, arms, indent, out);
        tmp
    };
    if let [a, b] = arms {
        if let (Some(test), Some(_)) = (
            two_way_test(&a.pattern, &subject),
            two_way_test(&b.pattern, &subject),
        ) {
            let pad2 = "  ".repeat(indent + 1);
            let (a_prelude, a_body) = read_in_place(&a.pattern, scrutinee, &a.body, &subject, &pad2);
            let (b_prelude, b_body) = read_in_place(&b.pattern, scrutinee, &b.body, &subject, &pad2);
            let mut branches = vec![Branch { test: Some(test), prelude: a_prelude, body: &a_body }];
            if !(matches!(sink, Sink::Effect) && b.body == Expr::Lit(Lit::Unit)) {
                branches.push(Branch { test: None, prelude: b_prelude, body: &b_body });
            }
            emit_branches(&branches, indent, sink, out);
            return;
        }
    }
    if arms.iter().any(|a| a.pattern.is_lit_case()) {
        emit_lit_chain(&subject, arms, indent, sink, out);
        return;
    }
    // An enum of one variant: TS does not narrow a type that is not a union,
    // so a `switch` would leave its `default` reachable to `assertNever`.
    // The one arm is all there is.
    if let [arm] = arms {
        if let Pattern::Variant { bind, .. } = &arm.pattern {
            out.push_str(&bind_prelude(bind, &subject, &pad));
            emit_stmts(&arm.body, indent, sink, out);
            return;
        }
    }
    out.push_str(&format!("{pad}switch ({subject}.kind) {{\n"));
    let remainder = remainder_arm(arms);
    for arm in arms {
        if remainder.is_some_and(|r| std::ptr::eq(r, arm)) {
            continue;
        }
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
    if let Some(arm) = remainder {
        let braced = declares_at_top(&arm.body);
        if braced {
            out.push_str(&format!("{pad1}default: {{\n"));
        } else {
            out.push_str(&format!("{pad1}default:\n"));
        }
        emit_stmts(&arm.body, indent + 2, sink, out);
        if !matches!(sink, Sink::Return) && !ends_in_jump(&arm.body) {
            out.push_str(&format!("{pad1}  break;\n"));
        }
        if braced {
            out.push_str(&format!("{pad1}}}\n"));
        }
        out.push_str(&format!("{pad}}}\n"));
    } else {
        out.push_str(&format!(
            "{pad1}default:\n{pad1}  return assertNever({subject});\n{pad}}}\n"
        ));
    }
}

/// The one `A | B | …` (or `_`) that binds nothing, when other arms name
/// variants: it is the Rust `_`, printed as `default` instead of listing
/// every remaining case and `assertNever`.
fn remainder_arm(arms: &[purecrate_ir::Arm]) -> Option<&purecrate_ir::Arm> {
    let named = arms
        .iter()
        .any(|a| matches!(a.pattern, Pattern::Variant { .. }));
    if !named {
        return None;
    }
    let mut found = None;
    for a in arms {
        let catch_all = a.pattern.bindings().is_empty()
            && matches!(a.pattern, Pattern::Or(_) | Pattern::Wildcard)
            && !declares_at_top(&a.body);
        if catch_all {
            if found.is_some() {
                return None;
            }
            found = Some(a);
        }
    }
    found
}

/// A `match` on an integer or a `&str`: arms tried in order, the last (`_`)
/// as `else`. Well-formed strings are equal in UTF-8 exactly when they are in
/// UTF-16, so `===` is `str` equality.
/// One branch of an `if` chain: its test (`None` for the last `else`), the
/// bindings its pattern reads, and its code.
pub(crate) struct Branch<'e> {
    pub test: Option<String>,
    pub prelude: String,
    pub body: &'e Expr,
}

/// `if (a) { .. } else if (b) { .. } else { .. }`. Where every branch
/// before the last jumps (each returns, where the value is returned), a
/// branch follows the `if` before it instead of an `else`, and a branch that
/// is one `return` sits on the `if`'s line: `if (s === "") return
/// Result.err(..);`. A returned value is the last statement of its block,
/// and `rename` already kept each branch's bindings apart from the block's;
/// elsewhere statements follow, so the last branch moves out of its `else`
/// only when it declares nothing they could meet.
pub(crate) fn emit_branches(branches: &[Branch], indent: usize, sink: Sink, out: &mut String) {
    let pad = "  ".repeat(indent);
    let last = match branches.split_last() {
        Some((last, _)) if last.test.is_some() => None,
        Some((last, _)) if matches!(sink, Sink::Return) => Some(last),
        Some((last, rest)) if last.prelude.is_empty() && rest.iter().all(|b| ends_in_jump(b.body)) => Some(last),
        _ => None,
    };
    let flat_last = last.and_then(|last| {
        let mut body = String::new();
        emit_stmts(last.body, indent, sink, &mut body);
        let declares = body.lines().any(|l| {
            let l = l.strip_prefix(pad.as_str()).unwrap_or(l);
            l.starts_with("const ") || l.starts_with("let ")
        });
        let prelude: String =
            last.prelude.lines().map(|l| format!("{}\n", l.strip_prefix("  ").unwrap_or(l))).collect();
        (matches!(sink, Sink::Return) || !declares).then(|| format!("{prelude}{body}"))
    });
    let Some(flat_last) = flat_last else {
        for (i, b) in branches.iter().enumerate() {
            let head = match (i, &b.test) {
                (0, Some(test)) => format!("{pad}if ({test}) {{\n"),
                (_, Some(test)) => format!("{pad}}} else if ({test}) {{\n"),
                (_, None) => format!("{pad}}} else {{\n"),
            };
            out.push_str(&head);
            out.push_str(&b.prelude);
            emit_stmts(b.body, indent + 1, sink, out);
        }
        out.push_str(&format!("{pad}}}\n"));
        return;
    };
    for b in &branches[..branches.len() - 1] {
        let test = b.test.as_ref().expect("only the last branch has no test");
        emit_guard(test, &b.prelude, b.body, indent, sink, out);
    }
    // The last branch: its prelude was printed one level in.
    out.push_str(&flat_last);
}

/// `if (test) { prelude body }`, on one line when the body is one
/// `return` or jump and there is no prelude.
fn emit_guard(test: &str, prelude: &str, body: &Expr, indent: usize, sink: Sink, out: &mut String) {
    let pad = "  ".repeat(indent);
    let mut inner = String::new();
    emit_stmts(body, indent + 1, sink, &mut inner);
    let one = inner.trim_start();
    let jump = ["return ", "break ", "continue "].iter().any(|k| one.starts_with(k));
    if prelude.is_empty() && jump && inner.lines().count() == 1 {
        out.push_str(&format!("{pad}if ({test}) {one}"));
    } else {
        out.push_str(&format!("{pad}if ({test}) {{\n{prelude}{inner}{pad}}}\n"));
    }
}

/// `!c`, as `a !== b` for `a === b` and the like.
fn not(cond: &str) -> String {
    use crate::tidy::{group, Assoc, Side, PREC_UNARY};
    crate::tidy::negate(cond).unwrap_or_else(|| format!("!{}", group(cond, PREC_UNARY, Assoc::Right, Side::Right)))
}

/// `let x = match o { Some(v) => e, None => return .. }`, or the `if` that
/// returns on one side: the exit first, then `x` from the side that stays,
/// its payload read in place. Prints and returns `true` when `value` is
/// one; a side that stays and still needs statements goes round again.
fn let_else(name: &str, mutable: bool, ty: Option<&Ty>, value: &Expr, indent: usize, out: &mut String) -> bool {
    let pad = "  ".repeat(indent);
    let (test, prelude, exit, keep) = match peel_identity(value) {
        Expr::If { cond, then, else_ } => {
            let c = crate::tidy::strip_outer(&emit_expr(cond, indent)).to_string();
            match (ends_in_jump(then), ends_in_jump(else_)) {
                (true, false) => (c, String::new(), (**then).clone(), (**else_).clone()),
                (false, true) => (not(&c), String::new(), (**else_).clone(), (**then).clone()),
                _ => return false,
            }
        }
        // `let e = match Email::parse(raw) { Ok(e) => e, Err(e) => return .. }`:
        // the call is bound first, as Rust evaluates it first, and the
        // binding goes round as a place.
        Expr::Match { scrutinee, arms } if !is_place(scrutinee) => {
            if exit_arms(arms).is_none() {
                return false;
            }
            let tmp = match_temp(arms, indent);
            bind_scrutinee(&tmp, scrutinee, arms, indent, out);
            let on_tmp = Expr::Match { scrutinee: Box::new(Expr::Var(Name::new(tmp))), arms: arms.clone() };
            return let_else(name, mutable, ty, &on_tmp, indent, out);
        }
        Expr::Match { scrutinee, arms } => {
            let Some((exit, keep)) = exit_arms(arms) else { return false };
            let subject = emit_expr(scrutinee, indent);
            let test = two_way_test(&exit.pattern, &subject).expect("exit_arms checked");
            let read = match &keep.pattern {
                Pattern::ResultOk(_) => field_of(scrutinee, "value"),
                Pattern::ResultErr(_) => field_of(scrutinee, "error"),
                _ => (**scrutinee).clone(),
            };
            let kept = match payload(&keep.pattern) {
                Some(Pattern::Var(n)) => subst(&keep.body, n, &read),
                _ => keep.body.clone(),
            };
            // The exit reads its payload in place too (`return Result.err(
            // { kind: "Email", value: result.error })`), unless it takes the
            // payload apart.
            match (payload(&exit.pattern), &exit.pattern) {
                (Some(Pattern::Var(n)), Pattern::ResultErr(_)) => {
                    (test, String::new(), subst(&exit.body, n, &field_of(scrutinee, "error")), kept)
                }
                (Some(Pattern::Var(n)), Pattern::ResultOk(_)) => {
                    (test, String::new(), subst(&exit.body, n, &field_of(scrutinee, "value")), kept)
                }
                _ => {
                    let prelude = two_way_prelude(&exit.pattern, &subject, &"  ".repeat(indent + 1));
                    (test, prelude, exit.body.clone(), kept)
                }
            }
        }
        _ => return false,
    };
    emit_guard(&test, &prelude, &exit, indent, Sink::Effect, out);
    if !let_else(name, mutable, ty, &keep, indent, out) {
        emit_let(name, mutable, ty, &keep, indent, out);
    }
    let _ = pad;
    true
}

/// The arm that jumps and the arm that stays of a two-way `match` on an
/// `Option` or `Result` (`let_else`), when the one that stays binds its
/// payload to a name or nothing.
fn exit_arms(arms: &[purecrate_ir::Arm]) -> Option<(&purecrate_ir::Arm, &purecrate_ir::Arm)> {
    let [a, b] = arms else { return None };
    let (exit, keep) = match (ends_in_jump(&a.body), ends_in_jump(&b.body)) {
        (true, false) => (a, b),
        (false, true) => (b, a),
        _ => return None,
    };
    two_way_test(&exit.pattern, "")?;
    two_way_test(&keep.pattern, "")?;
    match payload(&keep.pattern) {
        None | Some(Pattern::Var(_) | Pattern::Wildcard) => Some((exit, keep)),
        Some(_) => None,
    }
}

/// The pattern inside `Some(..)`, `Ok(..)`, or `Err(..)`.
fn payload(pattern: &Pattern) -> Option<&Pattern> {
    match pattern {
        Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => Some(p),
        _ => None,
    }
}

fn field_of(base: &Expr, name: &str) -> Expr {
    Expr::Field { base: Box::new(base.clone()), name: Name::new(name) }
}

pub(crate) fn emit_lit_chain(
    subject: &str,
    arms: &[purecrate_ir::Arm],
    indent: usize,
    sink: Sink,
    out: &mut String,
) {
    let branches: Vec<Branch> = arms
        .iter()
        .map(|arm| Branch { test: lit_test(&arm.pattern, subject), prelude: String::new(), body: &arm.body })
        .collect();
    emit_branches(&branches, indent, sink, out);
}

/// The test for an integer, `char`, `bool`, or string arm; `None` for `_`.
pub(crate) fn lit_test(pattern: &Pattern, subject: &str) -> Option<String> {
    match pattern {
        // A `bool` reads as itself: `x`, `!x`.
        Pattern::Lit(Lit::Bool(true)) => Some(subject.to_string()),
        Pattern::Lit(Lit::Bool(false)) => Some(format!("!{subject}")),
        Pattern::Lit(lit) => Some(format!("{subject} === {}", bare_lit(lit))),
        // By code point: JS orders strings by UTF-16 unit.
        Pattern::Range {
            lo: Lit::Char(lo),
            hi: Lit::Char(hi),
            inclusive,
        } => Some(format!(
            "Char.code({subject}) >= {} && Char.code({subject}) {} {}",
            u32::from(*lo),
            if *inclusive { "<=" } else { "<" },
            u32::from(*hi)
        )),
        Pattern::Range { lo, hi, inclusive } => Some(format!(
            "{subject} >= {} && {subject} {} {}",
            bare_lit(lo),
            if *inclusive { "<=" } else { "<" },
            bare_lit(hi)
        )),
        Pattern::Or(alts) => Some(
            alts.iter()
                .filter_map(|a| match a {
                    Pattern::Range { .. } => lit_test(a, subject),
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

/// An arm's binding and body. `Some(m)` on a variable is the variable
/// itself, narrowed: the body reads it in place of `m` (`if (method !==
/// null)` then `method`, not `const m = method`) where nothing in the body
/// binds or assigns the variable or captures it in a closure, which TS may
/// not narrow. A field or a `Result` payload is read once, into the name.
fn read_in_place(pattern: &Pattern, scrutinee: &Expr, body: &Expr, subject: &str, pad: &str) -> (String, Expr) {
    if let (Pattern::OptionSome(inner), Expr::Var(var)) = (pattern, scrutinee) {
        if let Pattern::Var(n) = &**inner {
            if !touches(body, var) {
                return (String::new(), subst(body, n, scrutinee));
            }
        }
    }
    (two_way_prelude(pattern, subject, pad), body.clone())
}

/// Whether `expr` binds or assigns `name`, or holds a closure that reads it.
fn touches(expr: &Expr, name: &Name) -> bool {
    let here = match expr {
        Expr::Let { name: n, .. } | Expr::Assign { name: n, .. } => n == name,
        Expr::For { var, .. } | Expr::ForEach { var, .. } => var == name,
        Expr::Match { arms, .. } => arms.iter().any(|a| a.pattern.bindings().iter().any(|b| *b == name)),
        Expr::Closure { body, .. } => reads(body, name),
        _ => false,
    };
    here || expr.children().into_iter().any(|c| touches(c, name))
}

fn reads(expr: &Expr, name: &Name) -> bool {
    matches!(expr, Expr::Var(n) if n == name) || expr.children().into_iter().any(|c| reads(c, name))
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
        Pattern::Tuple(elems) => tuple_names(elems, &read, pad),
        _ => String::new(),
    }
}

/// `const a = read[0]` for each name in a tuple of names and `_`.
fn tuple_names(elems: &[Pattern], read: &str, pad: &str) -> String {
    elems
        .iter()
        .enumerate()
        .filter_map(|(i, p)| match p {
            Pattern::Var(n) => Some(format!("{pad}const {} = {read}[{i}];\n", n.as_str())),
            _ => None,
        })
        .collect()
}

/// Field `i` of a tuple variant of `n` fields: `.value` when it is the
/// only one, else `.content[i]`.
pub(crate) fn tuple_variant_field(subject: &str, i: usize, n: usize) -> String {
    if n == 1 {
        format!("{subject}.value")
    } else {
        format!("{subject}.content[{i}]")
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
                    "{pad}const {name} = {};\n",
                    tuple_variant_field(subject, i, pats.len()),
                    name = n.as_str()
                )),
                Pattern::Tuple(elems) => {
                    Some(tuple_names(elems, &tuple_variant_field(subject, i, pats.len()), pad))
                }
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
