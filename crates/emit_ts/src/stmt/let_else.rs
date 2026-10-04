//! `let x = match v { P(x) => x, _ => return .. }` as a guard and a
//! `const`.

use super::*;

/// `let x = match o { Some(v) => e, None => return .. }`, or the `if` that
/// returns on one side: the exit first, then `x` from the side that stays,
/// its payload read in place. Prints and returns `true` when `value` is
/// one; a side that stays and still needs statements goes round again.
pub(super) fn let_else(
    name: &str,
    mutable: bool,
    ty: Option<&Ty>,
    value: &Expr,
    indent: usize,
    out: &mut String,
) -> bool {
    let pad = "  ".repeat(indent);
    let (test, prelude, exit, keep) = match peel_identity(value) {
        Expr::If { cond, then, else_ } => {
            let c = emit_tx(cond, indent);
            match (ends_in_jump(then), ends_in_jump(else_)) {
                (true, false) => (c.print(), String::new(), (**then).clone(), (**else_).clone()),
                (false, true) => (c.not_all().print(), String::new(), (**else_).clone(), (**then).clone()),
                _ => return false,
            }
        }
        // `let e = match Email::parse(raw) { Ok(e) => e, Err(e) => return .. }`:
        // the call is bound first, as Rust evaluates it first, and the
        // binding goes round as a place.
        Expr::Match { scrutinee, arms } if !is_place(scrutinee) => {
            let Some((_, keep)) = exit_arms(arms) else { return false };
            // `let t = match f() { Some(t) => t, None => return .. }`: the
            // `Option` is held in `t` itself, which the exit narrows.
            let unwraps = matches!((&keep.pattern, &keep.body),
                (Pattern::OptionSome(p), Expr::Var(read)) if matches!(&**p, Pattern::Var(n) if n == read));
            let tmp = if unwraps && !mutable && !name.starts_with('$') { name.to_string() } else { match_temp(arms) };
            bind_scrutinee(&tmp, scrutinee, arms, indent, out);
            let on_tmp = Expr::Match { scrutinee: Box::new(Expr::Var(Name::new(tmp))), arms: arms.clone() };
            return let_else(name, mutable, ty, &on_tmp, indent, out);
        }
        Expr::Match { scrutinee, arms } => {
            let Some((exit, keep)) = exit_arms(arms) else { return false };
            // The payload is read from the place where it is used: not where
            // an arm assigns or rebinds the place first.
            let mut root = &**scrutinee;
            while let Expr::Field { base, .. } = root {
                root = base;
            }
            if matches!(root, Expr::Var(var) if touches(&keep.body, var) || touches(&exit.body, var)) {
                return false;
            }
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

/// Whether `a` is `b` renumbered (`client$1` for `client`): the same Rust
/// name, so reading `b` in its place loses no name the author chose.
pub(super) fn same_source(a: &Name, b: &Name) -> bool {
    let base = |n: &Name| n.as_str().split('$').next().unwrap_or("").to_string();
    !a.as_str().starts_with('$') && base(a) == base(b)
}

/// The variable of `if v.is_none() { return .. }`.
pub(super) fn null_guarded(expr: &Expr) -> Option<&Name> {
    match expr {
        Expr::If { cond, then, else_ } if ends_in_jump(then) && **else_ == Expr::Lit(Lit::Unit) => match &**cond {
            Expr::Call { callee: purecrate_ir::Callee::OptionIsNone, args } => match args.as_slice() {
                [Expr::Var(v)] => Some(v),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// `match v { Some(x) => x, None => return .. }` on a variable `v`: the
/// variable, the exit's test, and the exit.
pub(super) fn unwrapped_var(value: &Expr) -> Option<(&Name, String, &Expr)> {
    let Expr::Match { scrutinee, arms } = peel_identity(value) else { return None };
    let Expr::Var(var) = &**scrutinee else { return None };
    let (exit, keep) = exit_arms(arms)?;
    match (&keep.pattern, &keep.body) {
        (Pattern::OptionSome(p), Expr::Var(read)) if matches!(&**p, Pattern::Var(n) if n == read) => {
            Some((var, two_way_test(&exit.pattern, var.as_str())?, &exit.body))
        }
        _ => None,
    }
}

/// The pattern inside `Some(..)`, `Ok(..)`, or `Err(..)`.
pub(super) fn payload(pattern: &Pattern) -> Option<&Pattern> {
    match pattern {
        Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => Some(p),
        _ => None,
    }
}

fn field_of(base: &Expr, name: &str) -> Expr {
    Expr::Field { base: Box::new(base.clone()), name: Name::new(name) }
}
