//! Statements: `let`, `?`, loops and jumps, and `match` as `switch` or an
//! `if` chain, each handing its value to a `Sink`.

use super::*;

mod branches;
mod let_else;
mod loops;
mod patterns;
mod query;
mod switch;

pub(crate) use branches::*;
use let_else::*;
pub(crate) use loops::*;
pub(crate) use patterns::*;
pub(crate) use query::*;
pub(crate) use switch::*;

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
            Sink::Return => out.push_str(&format!("{pad}return {};\n", crate::tidy::strip_outer(value))),
            Sink::Assign(target) => out.push_str(&format!("{pad}{target} = {};\n", crate::tidy::strip_outer(value))),
            // A variable or `()` has no effect; anything else may panic.
            Sink::Effect if value == "undefined" || is_ident(value) => {}
            Sink::Effect => out.push_str(&format!("{pad}{value};\n")),
        }
    }

    pub(crate) fn finish_expr(self, expr: &Expr, indent: usize, out: &mut String) {
        let pad = "  ".repeat(indent);
        // An `if` run for what one side does (what an unread `a && f()` or
        // a decided loop test leaves) is a statement: as `c ? undefined :
        // f()`, lint refuses an expression nothing reads.
        if let (Sink::Effect, Expr::If { cond, then, else_ }) = (self, expr) {
            let unit = Expr::Lit(Lit::Unit);
            if (**then == unit) != (**else_ == unit) {
                let (test, side) = if **else_ == unit {
                    (emit_tx(cond, indent).print(), then)
                } else {
                    (emit_tx(cond, indent).not_all().print(), else_)
                };
                out.push_str(&format!("{pad}if ({}) {{\n", crate::tidy::strip_outer(&test)));
                Sink::Effect.finish_expr(side, indent + 1, out);
                out.push_str(&format!("{pad}}}\n"));
                return;
            }
        }
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
pub(crate) fn match_temp(arms: &[purecrate_ir::Arm]) -> String {
    let base = match scrutinee_ty(arms) {
        Some(ty) => purecrate_ir::to_camel(&purecrate_ir::lower_first(ty.as_str())),
        None => match arms.iter().map(|a| &a.pattern).find(|p| !matches!(p, Pattern::Wildcard)) {
            Some(Pattern::ResultOk(_) | Pattern::ResultErr(_)) => "result".to_string(),
            Some(Pattern::OptionSome(_) | Pattern::OptionNone) => "option".to_string(),
            _ => "value".to_string(),
        },
    };
    temp(&base)
}

/// Whether `expr` prints as a `?:` of its own (not `&&` / `||` with a
/// `bool` literal): one inside a returned `?:` is a nested choice, which
/// reads better as `if (..) return ..;` lines.
fn chooses(expr: &Expr) -> bool {
    let lit = |e: &Expr| matches!(e, Expr::Lit(Lit::Bool(_)));
    match peel(expr) {
        Expr::If { then, else_, .. } => !lit(then) && !lit(else_),
        Expr::Match { arms, .. } => !arms.iter().any(|a| lit(&a.body)),
        _ => false,
    }
}

/// Whether `return expr;` is one line within the width.
fn fits_returned(expr: &Expr, indent: usize) -> bool {
    as_expr(expr, indent).is_some_and(|v| {
        !v.contains('\n') && 2 * indent + "return ;".len() + crate::tidy::strip_outer(&v).len() <= crate::WIDTH
    })
}

/// `emit_stmts`, told whether `expr` is the last statement of its block.
fn emit_tail(expr: &Expr, indent: usize, sink: Sink, out: &mut String, tail: bool) {
    crate::TAIL.with(|t| t.set(tail));
    emit_stmts(expr, indent, sink, out);
}

/// Lower an expression into statements that hand its value to `sink`.
pub(crate) fn emit_stmts(expr: &Expr, indent: usize, sink: Sink, out: &mut String) {
    let pad = "  ".repeat(indent);
    let tail = crate::TAIL.with(|t| t.replace(false));
    match expr {
        Expr::Comment(lines) => {
            for line in lines {
                match line.as_str() {
                    "" => out.push_str(&format!("{pad}//\n")),
                    line => out.push_str(&format!("{pad}// {line}\n")),
                }
            }
            sink.finish("undefined", &pad, out);
        }
        Expr::Let { name, mutable, ty, value, then } => {
            // `let $x: T = e; $x` names `T` for `collect::<T>()` / `sum::<T>()`.
            // `e` is the result; the binding would only be copied into the sink.
            if !*mutable
                && matches!(then.as_ref(), Expr::Var(n) if n == name)
                && !peel_identity(value).needs_statements()
            {
                emit_tail(peel_identity(value), indent, sink, out, tail);
            } else if let Some((slots, rest)) = (!*mutable && name.as_str().starts_with('$'))
                .then(|| destructured(name, then))
                .flatten()
                .filter(|_| matches!(**value, Expr::Try { .. }) || value_expr(value, indent).is_some())
            {
                // `let (a, b) = v`: the tuple is only taken apart. From `v?`,
                // the payload once the exit is past.
                let (prelude, s) = match &**value {
                    Expr::Try { expr, on } => {
                        let tmp = try_temp(None, *on);
                        let mut exit = String::new();
                        emit_try_exit(&tmp, expr, *on, indent, &mut exit);
                        let payload = if *on == Some(TryOn::Option) { tmp } else { format!("{tmp}.value") };
                        (exit, payload)
                    }
                    _ => value_expr(value, indent).expect("checked above"),
                };
                let pattern = slots.iter().map(|n| n.map_or("", |n| n.as_str())).collect::<Vec<_>>().join(", ");
                // The pattern types an array literal as a tuple; only an
                // object literal would widen (`kind: string`) without one. A
                // call or a place already has its type.
                let typed = matches!(
                    peel_identity(value),
                    Expr::Try { .. }
                        | Expr::Call { .. }
                        | Expr::MethodCall { .. }
                        | Expr::Var(_)
                        | Expr::Field { .. }
                        | Expr::Index { .. }
                );
                let annotation = ty
                    .as_ref()
                    .filter(|t| holds_object(t) && !typed)
                    .map(|t| format!(": {}", emit_ty(t)))
                    .unwrap_or_default();
                out.push_str(&prelude);
                out.push_str(&format!("{pad}const [{pattern}]{annotation} = {};\n", crate::tidy::strip_outer(&s)));
                emit_tail(rest, indent, sink, out, tail);
            } else if !*mutable && name.as_str().starts_with('$') && value.is_inlinable() {
                // `$opt = x` / `$optOr = d` around `unwrap_or` / `ok_or`: the
                // names and variant literals cannot panic, so the uses read
                // them directly (eager evaluation of a call still binds).
                emit_tail(&subst(then, name, value), indent, sink, out, tail);
            } else if let Some((var, test, exit)) = (!*mutable)
                .then(|| unwrapped_var(value))
                .flatten()
                .filter(|(var, ..)| same_source(name, var) && !touches(then, var))
            {
                // `let Some(c) = client else { return .. }`: after the exit
                // TS has narrowed `client`, so the rest reads it as is.
                emit_guard(&test, "", exit, indent, Sink::Effect, out);
                emit_tail(&subst(then, name, &Expr::Var(var.clone())), indent, sink, out, tail);
            } else {
                emit_let(name.as_str(), *mutable, ty.as_ref(), value, indent, out);
                // An element read right after its tuple is made: nothing has
                // narrowed it, so it needs no `as T` to widen it back.
                let mut then = &**then;
                while let Expr::Let { name: n, mutable: false, value: v, then: rest, .. } = then {
                    let fresh = name.as_str().starts_with('$')
                        && !n.as_str().starts_with('$')
                        && matches!(&**v, Expr::Field { base, name: f } if f.as_str().starts_with('[') && **base == Expr::Var(name.clone()));
                    if !fresh {
                        break;
                    }
                    emit_let(n.as_str(), false, None, v, indent, out);
                    then = rest;
                }
                emit_tail(then, indent, sink, out, tail);
            }
        }
        // `if true { .. }` as a statement (the side a decided test keeps,
        // which declares names a later `let` may declare again): a block of
        // its own. Lint refuses a constant test; a block that declares is
        // no lone block.
        Expr::If { cond, then, else_ }
            if matches!(sink, Sink::Effect)
                && **cond == Expr::Lit(Lit::Bool(true))
                && **else_ == Expr::Lit(Lit::Unit) =>
        {
            out.push_str(&format!("{pad}{{\n"));
            emit_stmts(then, indent + 1, sink, out);
            out.push_str(&format!("{pad}}}\n"));
        }
        // A guard: `if (c) return v;` on one line, when both fit on one, and
        // `if (c) break;` / `continue;`.
        Expr::If { cond, then, else_ }
            if matches!(sink, Sink::Effect)
                && **else_ == Expr::Lit(Lit::Unit)
                && matches!(lone_jump(then), Expr::Return(v) if !v.needs_statements())
                    | matches!(lone_jump(then), Expr::Break | Expr::Continue)
                && !emit_expr(cond, indent).contains('\n') =>
        {
            let jump = match lone_jump(then) {
                Expr::Return(v) => format!("return {}", crate::tidy::strip_outer(&emit_expr(v, indent))),
                Expr::Break => format!("break{}", jump_label()),
                _ => format!("continue{}", jump_label()),
            };
            out.push_str(&format!("{pad}if ({}) {jump};\n", crate::tidy::strip_outer(&emit_expr(cond, indent))));
        }
        // `if matches!(f(x), p) ..` (or `!matches!`): the matched value is
        // the first thing the condition evaluates, so it is bound before the
        // `if` and the test reads the binding, not an inline function.
        Expr::If { cond, then, else_ } if hoistable_test(cond).is_some() => {
            let (scrutinee, arms, negated) = hoistable_test(cond).expect("checked above");
            let tmp = match_temp(arms);
            let on_tmp = Expr::Match { scrutinee: Box::new(Expr::Var(Name::new(tmp.clone()))), arms: arms.to_vec() };
            // A test that does not read the value (every arm `true`) still
            // evaluates it, for what it may panic on.
            if emit_expr(&on_tmp, indent).contains(tmp.as_str()) {
                bind_scrutinee(&tmp, scrutinee, arms, indent, out);
            } else {
                Sink::Effect.finish_expr(scrutinee, indent, out);
            }
            let test =
                if negated { Expr::Unary { op: purecrate_ir::UnOp::Not, expr: Box::new(on_tmp) } } else { on_tmp };
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
            let c = emit_tx(cond, indent);
            let (test, exit, keep) = if ends_in_jump(then) { (c, then, else_) } else { (c.not_all(), else_, then) };
            emit_guard(&test.print(), "", exit, indent, Sink::Effect, out);
            sink.finish_expr(keep, indent, out);
        }
        // `if c {} else { .. }` as a statement: the test turned round.
        Expr::If { cond, then, else_ }
            if matches!(sink, Sink::Effect) && **then == Expr::Lit(Lit::Unit) && **else_ != Expr::Lit(Lit::Unit) =>
        {
            let test = emit_tx(cond, indent).not_all().print();
            emit_guard(&test, "", else_, indent, sink, out);
        }
        Expr::If { cond, then, else_ }
            if expr.needs_statements()
                || matches!(sink, Sink::Return)
                    && (chooses(then) || chooses(else_))
                    && !fits_returned(expr, indent) =>
        {
            let test = emit_tx(cond, indent);
            let mut branches = vec![Branch { test: Some(test), prelude: String::new(), body: then }];
            // An `else` that is one more `if` with statements is `else if`,
            // as is any where the value is returned.
            let mut rest = &**else_;
            while let Expr::If { cond, then, else_ } = rest {
                if !rest.needs_statements()
                    && !(matches!(sink, Sink::Return) && chooses(rest) && !fits_returned(rest, indent))
                {
                    break;
                }
                let test = emit_tx(cond, indent);
                branches.push(Branch { test: Some(test), prelude: String::new(), body: then });
                rest = else_;
            }
            if !(matches!(sink, Sink::Effect) && *rest == Expr::Lit(Lit::Unit)) {
                branches.push(Branch { test: None, prelude: String::new(), body: rest });
            }
            emit_branches(&branches, indent, sink, tail, out);
        }
        // `s.push(x)` / `s.push_str(x)`, as `check` writes them. JS `+=`
        // reads `s` before the piece, as `s = s + x` does, so a piece that
        // needs statements is the same `(() => { .. })()` either way; one
        // that leaves the function is printed by the general path.
        Expr::Assign { name, value }
            if matches!(&**value, Expr::Call { callee: purecrate_ir::Callee::StrConcat, args }
                if args[0] == Expr::Var(name.clone()) && !args[1].exits()) =>
        {
            let Expr::Call { args, .. } = &**value else { unreachable!("matched above") };
            let piece = match &args[1] {
                Expr::Lit(Lit::Char(c)) => crate::expr::js_string(&c.to_string()),
                other => crate::tidy::strip_outer(&crate::expr::emit_item(other, indent)).to_string(),
            };
            out.push_str(&format!("{pad}{} += {piece};\n", name.as_str()));
            sink.finish("undefined", &pad, out);
        }
        Expr::Assign { name, value } => {
            if value.needs_statements() {
                emit_stmts(value, indent, Sink::Assign(name.as_str()), out);
            } else {
                // `x = 3 as I32;`: an assignment's value needs no parentheses
                // of its own (`Some(3)` prints as a parenthesized `3`).
                let item = crate::expr::emit_item(value, indent);
                out.push_str(&format!("{pad}{} = {};\n", name.as_str(), crate::tidy::strip_outer(&item)));
            }
            sink.finish("undefined", &pad, out);
        }
        // `if (o === null) return ..;` then `let x = o` (what `ok_or(e)?`
        // lowers to): `o` is narrowed, so the rest reads it as is.
        Expr::Seq { first, then }
            if matches!(&**then, Expr::Let { name, mutable: false, value, then: rest, .. }
                if null_guarded(first).is_some_and(|v| **value == Expr::Var(v.clone()) && same_source(name, v) && !touches(rest, v))) =>
        {
            let Expr::Let { name, value, then: rest, .. } = &**then else { unreachable!("matched above") };
            emit_stmts(first, indent, Sink::Effect, out);
            emit_tail(&subst(rest, name, value), indent, sink, out, tail);
        }
        Expr::Seq { first, then } => {
            emit_stmts(first, indent, Sink::Effect, out);
            let dead = ends_in_jump(first) && **then == Expr::Lit(Lit::Unit);
            if !dead {
                emit_tail(then, indent, sink, out, tail);
            }
        }
        // A predicate (`matches!`) or `unwrap_or` / `ok_or` on a place is one
        // expression (`x ?? d`, a `?:`); anything larger is a `switch`.
        Expr::Match { scrutinee, arms }
            if !matches!(sink, Sink::Effect)
                && is_place(scrutinee)
                && (arms.iter().all(|a| matches!(a.body, Expr::Lit(Lit::Bool(_))))
                    || (arms.len() == 2 && two_way_test(&arms[0].pattern, "").is_some()))
                && !(matches!(sink, Sink::Return)
                    && arms.iter().any(|a| chooses(&a.body))
                    && !fits_returned(expr, indent)) =>
        {
            match as_expr(expr, indent) {
                Some(value) => sink.finish(&value, &pad, out),
                None => emit_switch(scrutinee, arms, indent, sink, tail, out),
            }
        }
        // A `match` on a call that is one `call ?? d` needs no binding.
        Expr::Match { .. } if !matches!(sink, Sink::Effect) && coalesced(expr, indent).is_some() => {
            let value = coalesced(expr, indent).expect("matched above");
            sink.finish(&value, &pad, out);
        }
        Expr::Match { scrutinee, arms } => emit_switch(scrutinee, arms, indent, sink, tail, out),
        Expr::For { var, ty, start, end, body } => {
            let ty = ty.expect("check::accept types the range");
            emit_for(var.as_str(), ty, start, end, body, indent, out);
            sink.finish("undefined", &pad, out);
        }
        Expr::ForEach { var, over, source: string, body } => {
            // `for x in v.clone()` iterates a copy only where the body grows
            // `v`: JS `for..of` would meet what the body pushes, Rust's loop
            // does not. Otherwise nothing writes the array while the loop
            // reads it, and the copy is a useless spread.
            // The copy a loop needs is named first: oxlint refuses a spread
            // as the iterable of `for..of` whatever the body does.
            let iterable = match string.as_ref() {
                Expr::Call { callee: Callee::Collect { result: false, over: purecrate_ir::Over::Items }, args } => {
                    if grows(body, &args[0]) {
                        let copy = temp("copy");
                        out.push_str(&format!("{pad}const {copy} = {};\n", emit_expr(string, indent)));
                        copy
                    } else {
                        iterable(*over, emit_tx(&args[0], indent))
                    }
                }
                s => iterable(*over, emit_tx(s, indent)),
            };
            emit_loop(&format!("for (const {} of {iterable})", var.as_str()), body, indent, out);
            sink.finish("undefined", &pad, out);
        }
        Expr::While { cond, body } => {
            // `loop` (and a `while` whose test runs in the body) is `for (;;)`,
            // which no lint takes for a constant condition.
            let head = match **cond {
                Expr::Lit(Lit::Bool(true)) => "for (;;)".to_string(),
                _ => {
                    let test = crate::tidy::strip_outer(&emit_expr(cond, indent + 1)).to_string();
                    if test.contains('\n') {
                        // A test of several lines (a function called in
                        // place) opens, one operand a line, as oxfmt lays it.
                        let pad1 = "  ".repeat(indent + 1);
                        let parts = crate::tidy::split_at_op(&test, " || ", 0)
                            .or_else(|| crate::tidy::split_at_op(&test, " && ", 0))
                            .unwrap_or_else(|| vec![test.clone()]);
                        let lines: Vec<String> = parts.iter().map(|p| format!("{pad1}{}", p.trim_end())).collect();
                        format!("while (\n{}\n{pad})", lines.join("\n"))
                    } else {
                        format!("while ({test})")
                    }
                }
            };
            emit_loop(&head, body, indent, out);
            // Nothing follows a `for (;;)` that no `break` leaves.
            if !ends_in_jump(expr) {
                sink.finish("undefined", &pad, out);
            }
        }
        Expr::Break => out.push_str(&format!("{pad}break{};\n", jump_label())),
        Expr::Continue => out.push_str(&format!("{pad}continue{};\n", jump_label())),
        // `return match ..`: the `match` returns from each arm.
        Expr::Return(value) if value.needs_statements() && as_expr(value, indent).is_none() => {
            emit_stmts(value, indent, Sink::Return, out)
        }
        Expr::Return(value) => {
            out.push_str(&format!("{pad}return {};\n", crate::tidy::strip_outer(&emit_expr(value, indent))))
        }
        // `x?;`: only the early return; there is no value to bind.
        // `x?;` on a binding: the test alone.
        Expr::Try { expr: inner, on } if matches!(sink, Sink::Effect) && matches!(**inner, Expr::Var(_)) => {
            emit_try_test(&emit_expr(inner, indent), *on, indent, out);
        }
        Expr::Try { expr: inner, on } if matches!(sink, Sink::Effect) => {
            emit_try_exit(&try_temp(None, *on), inner, *on, indent, out);
        }
        Expr::Try { .. } => {
            let tmp = temp("value");
            emit_let(&tmp, false, None, expr, indent, out);
            sink.finish(&tmp, &pad, out);
        }
        other => sink.finish_expr(other, indent, out),
    }
}

pub(crate) fn emit_let(name: &str, mutable: bool, ty: Option<&Ty>, value: &Expr, indent: usize, out: &mut String) {
    // `collect::<T>()` and `sum::<T>()` are a typed `let` whose body is the
    // binding. Nested under another `let`, that binding is only a copy.
    let value = peel_identity(value);
    let pad = "  ".repeat(indent);
    let keyword = if mutable { "let" } else { "const" };
    // An arrow states its own type, and a `let` of `true` or `false` is a
    // `boolean` without one.
    let inferred = matches!(value, Expr::Closure { .. })
        || (mutable && matches!(value, Expr::Lit(Lit::Bool(_))) && matches!(ty, Some(t) if *t == Ty::bool()));
    let inferred = inferred || (!mutable && ty.is_some_and(|t| infers_as(value, t)));
    let grown = crate::PUSHED.with(|p| p.borrow().contains(name));
    let shown = |t: &Ty| match t {
        Ty::Vec(item) if grown => format!("Array<{}>", emit_ty(item)),
        t => emit_ty(t),
    };
    let annotation = ty.filter(|_| !inferred).map(|t| format!(": {}", shown(t))).unwrap_or_default();
    // A value cast to the type already states it (`let n = 0 as Usize`).
    let stated = |value: &str| match ty {
        Some(t) if crate::tidy::cast_type(value) == Some(emit_ty(t).as_str()) => String::new(),
        _ => annotation.clone(),
    };
    match value {
        Expr::Var(n) if n.as_str() == name => {}
        Expr::Try { expr, on } => {
            let tmp = try_temp(Some(name), *on);
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
        // annotation already says. A place no `match` tests is not
        // narrowed, and the cast would be one lint refuses.
        v if is_place(v)
            && matches!(ty, Some(Ty::Named(n)) if !crate::is_struct(n.as_str()))
            && crate::TESTED.with(|t| t.borrow().iter().any(|t| within(v, t))) =>
        {
            let t = emit_ty(ty.unwrap());
            out.push_str(&format!("{pad}{keyword} {name} = {} as {t};\n", emit_expr(v, indent)));
        }
        v if v.needs_statements() && let_else(name, mutable, ty, v, indent, out) => {}
        // A temporary bound for the value alone (`$r = s.parse(); match $r`)
        // needs no block of its own: a made name meets no source name.
        // A name, literal, or variant literal is read in place, as
        // `emit_stmts` does (`opt ?? true`).
        Expr::Let { name: inner, mutable: false, ty: inner_ty, value: inner_value, then }
            if inner.as_str().starts_with('$') =>
        {
            // A `Result` that typing named `$result` (`map_err(f)?`) is
            // named after the local it is for: `emailResult`.
            let made =
                inner.as_str().trim_start_matches('$').trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
            if made == "result" && !inner_value.is_inlinable() {
                if let Some(renamed) = named_after(name, "result") {
                    let renamed = Name::new(renamed);
                    let then = subst(then, inner, &Expr::Var(renamed.clone()));
                    emit_let(renamed.as_str(), false, inner_ty.as_ref(), inner_value, indent, out);
                    emit_let(name, mutable, ty, &then, indent, out);
                    return;
                }
            }
            if inner_value.is_inlinable() {
                emit_let(name, mutable, ty, &subst(then, inner, inner_value), indent, out);
            } else {
                emit_let(inner.as_str(), false, inner_ty.as_ref(), inner_value, indent, out);
                emit_let(name, mutable, ty, then, indent, out);
            }
        }
        v if let Some((prelude, s)) = value_expr(v, indent) => {
            out.push_str(&prelude);
            let s = crate::tidy::strip_outer(&s);
            out.push_str(&format!("{pad}{keyword} {name}{} = {s};\n", stated(s)));
        }
        v if v.needs_statements() => {
            out.push_str(&format!("{pad}let {name}{annotation};\n"));
            // A Rust block: its bindings end with it. One whose printed
            // lines declare nothing at its level needs no braces (lint: a
            // lone block).
            let mut inner = String::new();
            let block = matches!(v, Expr::Let { .. } | Expr::Seq { .. }) && {
                emit_stmts(v, indent + 1, Sink::Assign(name), &mut inner);
                let own = "  ".repeat(indent + 1);
                inner.lines().any(|l| {
                    l.strip_prefix(own.as_str())
                        .is_some_and(|r| !r.starts_with(' ') && (r.starts_with("const ") || r.starts_with("let ")))
                })
            };
            if block {
                out.push_str(&format!("{pad}{{\n"));
                out.push_str(&inner);
                out.push_str(&format!("{pad}}}\n"));
            } else {
                emit_stmts(v, indent, Sink::Assign(name), out);
            }
        }
        v => {
            let s = emit_expr(v, indent);
            let s = crate::tidy::strip_outer(&s);
            out.push_str(&format!("{pad}{keyword} {name}{} = {s};\n", stated(s)));
        }
    }
}

/// Whether TS gives a `const` of `value` the type `ty` without being told:
/// a call of something that declares what it returns, a test, or a place
/// that is no union (a narrowed union is cast back, a `null` held would
/// stay). A literal, a variant, and a `?:` of them would infer their own.
fn infers_as(value: &Expr, ty: &Ty) -> bool {
    match peel(value) {
        // The annotation carries the wrapper's comment (`/* Box */ P`).
        _ if matches!(ty, Ty::Ignored { .. }) => false,
        Expr::Lit(_) => false,
        v if declares_return(v) => true,
        // `x?` is read where its test has narrowed it to the payload.
        Expr::Try { .. } => true,
        _ if *ty == Ty::bool() => true,
        v if is_place(v) => match ty {
            Ty::Prim(_) | Ty::Vec(_) => true,
            Ty::Named(n) => crate::is_struct(n.as_str()),
            _ => false,
        },
        _ => false,
    }
}

/// A call of something that declares what it returns: not a variant,
/// `Ok` / `Err`, or `Some` / `None`, whose literal infers its own type.
fn declares_return(value: &Expr) -> bool {
    use purecrate_ir::Callee as C;
    matches!(peel(value), Expr::Call { callee, .. }
        if !matches!(callee, C::Variant { .. } | C::ResultOk | C::ResultErr | C::OptionSome | C::OptionNone))
}

/// What `x?` is held in before its test: `<name>Result` or `<name>Option`
/// for `let name = x?`, else `result` or `option`.
fn try_temp(name: Option<&str>, on: Option<TryOn>) -> String {
    let what = if on == Some(TryOn::Option) { "option" } else { "result" };
    // A temporary's own name (`$value_2`) says nothing; its `_<depth>` is kept.
    name.and_then(|n| named_after(n, what)).unwrap_or_else(|| temp(what))
}

/// A temporary named after the local it is for (`$nResult` for `n`), or
/// `None` for a made local. A shadow `n$1` prints as `n2`: its temporary is
/// `n2Result`, so the two read as a pair.
fn named_after(name: &str, what: &str) -> Option<String> {
    if name.starts_with('$') {
        return None;
    }
    let n = match name.rsplit_once('$') {
        Some((b, k)) if k.bytes().all(|c| c.is_ascii_digit()) => {
            format!("{b}{}", k.parse::<usize>().map_or(0, |k| k + 1))
        }
        _ => name.to_string(),
    };
    // Numbered as every temporary is: two in one block (`let x = r.ok()`,
    // then `x.ok_or(e)` matched) would be one name, declared twice.
    Some(crate::temp(&format!("{n}{}", purecrate_ir::upper_first(what))))
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

/// Whether `body` grows the local `source` is (`push` or `insert`).
fn grows(body: &Expr, source: &Expr) -> bool {
    let Expr::Var(name) = source else { return false };
    let mut found = false;
    body.walk(|e| found |= e.grown() == Some(name));
    found
}
