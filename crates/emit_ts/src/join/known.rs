//! What a place is known to hold, and the tests and arms that decides:
//! cases by name (`$Some`, `$Ok`, a variant), a `bool`'s value, an alias's
//! place.

use super::*;

/// A place and the variants an enclosing arm has narrowed it to.
pub(super) type Known = Vec<(Expr, Vec<Name>)>;

/// The arm a `match` on `None`, `Some(e)`, `Ok(e)`, or `Err(e)` takes, with
/// `e` bound to the name its pattern gives it. `None` where an arm before
/// it may take the value, a guard decides, or the payload's pattern is
/// neither a name nor `_` over a value that cannot do anything.
pub(super) fn constructed_arm(expr: &Expr) -> Option<Expr> {
    use purecrate_ir::Callee;
    let Expr::Match { scrutinee, arms } = expr else { return None };
    let Expr::Call { callee, args } = &**scrutinee else { return None };
    let (case, payload) = match (callee, args.as_slice()) {
        (Callee::OptionNone, []) => (NONE, None),
        (Callee::OptionSome, [e]) => (SOME, Some(e)),
        (Callee::ResultOk, [e]) => (OK, Some(e)),
        (Callee::ResultErr, [e]) => (ERR, Some(e)),
        _ => return None,
    };
    let case = Name::new(case);
    for arm in arms {
        if admits(&arm.pattern, &case) {
            if arm.guard.is_some() {
                return None;
            }
            let inner = match &arm.pattern {
                Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => Some(&**p),
                _ => None,
            };
            return match (inner, payload) {
                (Some(Pattern::Var(n)), Some(e)) => Some(Expr::Let {
                    name: n.clone(),
                    mutable: false,
                    ty: None,
                    value: Box::new(e.clone()),
                    then: Box::new(arm.body.clone()),
                }),
                (Some(Pattern::Var(_)), None) => None,
                // A payload left unread must do nothing: a name, a literal, a field.
                (_, Some(e)) if !pure(e) => None,
                _ => Some(arm.body.clone()),
            };
        }
        if may_take(&arm.pattern, &case) {
            return None;
        }
    }
    None
}

/// Each `match name { .. }` in `expr` as the arm `value`, a constructor,
/// takes, where `constructed_arm` can tell.
pub(super) fn decide_on(expr: &mut Expr, name: &Name, value: &Expr) {
    use purecrate_ir::Callee;
    // `name?` of `Some(e)` / `Ok(e)` never leaves, and its payload is `e`
    // (`Result.ok(10).value` reads what TS cannot narrow).
    let payload = match value {
        Expr::Call { callee: Callee::OptionSome | Callee::ResultOk, args } => match args.as_slice() {
            [e] if matches!(e, Expr::Var(_) | Expr::Lit(_)) => Some(e),
            _ => None,
        },
        _ => None,
    };
    if let Some(e) = payload {
        if let Expr::Seq { first, then } = expr {
            if matches!(&**first, Expr::Try { expr: t, .. } if **t == Expr::Var(name.clone())) {
                *expr = std::mem::replace(&mut **then, Expr::Lit(Lit::Unit));
                return decide_on(expr, name, value);
            }
        }
        if matches!(expr, Expr::Field { base, name: f } if **base == Expr::Var(name.clone()) && f.as_str() == "value") {
            *expr = e.clone();
            return;
        }
    }
    if let Expr::Match { scrutinee, arms } = expr {
        if **scrutinee == Expr::Var(name.clone()) {
            let on_value = Expr::Match { scrutinee: Box::new(value.clone()), arms: arms.clone() };
            if let Some(taken) = constructed_arm(&on_value) {
                *expr = taken;
                return decide_on(expr, name, value);
            }
        }
    }
    // A binding of the same name hides it below; `let $p = p` is `p` too.
    if let Expr::Let { name: n, value: v, then, mutable: false, .. } = expr {
        if n == name {
            return decide_on(v, name, value);
        }
        if **v == Expr::Var(name.clone()) && !touches(then, n) {
            let n = n.clone();
            decide_on(then, &n, value);
        }
    }
    for child in expr.children_mut() {
        decide_on(child, name, value);
    }
}

/// `expr` as `None`, `Some(e)`, `Ok(e)`, or `Err(e)`, seeing through
/// `let t = place;` around it, `t` read as the place.
pub(super) fn as_constructor(expr: &Expr) -> Option<Expr> {
    match expr {
        e if constructed_case(e).is_some() => Some(e.clone()),
        Expr::Let { name, mutable: false, value, then, .. } if is_place(value) && !touches(then, root(value)) => {
            let mut inner = as_constructor(then)?;
            replace_var(&mut inner, name, value);
            Some(inner)
        }
        _ => None,
    }
}

/// The case `None`, `Some(e)`, `Ok(e)`, or `Err(e)` builds.
pub(super) fn constructed_case(expr: &Expr) -> Option<Name> {
    use purecrate_ir::Callee;
    let Expr::Call { callee, .. } = expr else { return None };
    let case = match callee {
        Callee::OptionNone => NONE,
        Callee::OptionSome => SOME,
        Callee::ResultOk => OK,
        Callee::ResultErr => ERR,
        _ => return None,
    };
    Some(Name::new(case))
}

/// Whether `pattern` may match a value of `case`, payload aside.
fn may_take(pattern: &Pattern, case: &Name) -> bool {
    match pattern {
        Pattern::ResultOk(_) => case.as_str() == OK,
        Pattern::ResultErr(_) => case.as_str() == ERR,
        Pattern::OptionSome(_) => case.as_str() == SOME,
        Pattern::OptionNone => case.as_str() == NONE,
        Pattern::Or(alts) => alts.iter().any(|p| may_take(p, case)),
        _ => true,
    }
}

/// The place a name `let t = place` binds holds, followed to its end.
pub(super) fn aliased(expr: &Expr) -> Option<Expr> {
    let Expr::Var(n) = expr else { return None };
    let target = ALIASES.with(|a| a.borrow().iter().rev().find(|(m, _)| m == n).map(|(_, p)| p.clone()))?;
    // `let v = v` (a pattern's binding read again under its own name) is
    // no alias: following it would never end.
    if target == *expr {
        return None;
    }
    Some(aliased(&target).unwrap_or(target))
}

/// The `return` of `place?` where `place` is known to hold `None` or an
/// `Err`, directly or through the `let` it aliases.
pub(super) fn failed_try(place: &Expr, on: TryOn, known: &Known) -> Option<Expr> {
    let fails = Name::new(if on == TryOn::Option { NONE } else { ERR });
    let holds = |p: &Expr| {
        known.iter().rev().find(|(k, _)| k == p).is_some_and(|(_, vs)| vs.as_slice() == std::slice::from_ref(&fails))
    };
    if !(holds(place) || aliased(place).is_some_and(|t| holds(&t))) {
        return None;
    }
    Some(Expr::Return(Box::new(if on == TryOn::Option {
        Expr::Call { callee: purecrate_ir::Callee::OptionNone, args: Vec::new() }
    } else {
        place.clone()
    })))
}

/// `cond` with each `bool` place a test has decided read as its value,
/// through `!`, `&&`, and `||`.
pub(super) fn decide(cond: &mut Expr, known: &Known) {
    match cond {
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => decide(expr, known),
        Expr::Binary { op: BinOp::And | BinOp::Or, left, right } => {
            decide(left, known);
            decide(right, known);
        }
        c if is_place(c) => {
            if let Some(b) = known_bool(c, known) {
                *c = Expr::Lit(Lit::Bool(b));
            }
        }
        _ => {}
    }
}

/// The value of a `bool` literal, of a place a test has decided, or of
/// `!` of one.
pub(super) fn known_bool(expr: &Expr, known: &Known) -> Option<bool> {
    match expr {
        Expr::Lit(Lit::Bool(b)) => Some(*b),
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => known_bool(expr, known).map(|b| !b),
        // `o.is_some()` of a place whose case is known.
        Expr::Call {
            callee: callee @ (purecrate_ir::Callee::OptionIsSome | purecrate_ir::Callee::OptionIsNone),
            args,
        } => {
            let [p] = args.as_slice() else { return None };
            if !is_place(p) {
                return None;
            }
            let (_, vs) = known.iter().rev().find(|(k, _)| k == p)?;
            let some = match vs.as_slice() {
                [v] if v.as_str() == SOME => true,
                [v] if v.as_str() == NONE => false,
                _ => return None,
            };
            Some(some == (*callee == purecrate_ir::Callee::OptionIsSome))
        }
        // A `match` of `bool`s (`matches!(o, Some(_) if c)`, printed `o !==
        // null && c`): the value every arm that may run gives, an arm whose
        // guard is decided `false` never running.
        // Only where nothing in it can panic: folded, it no longer runs.
        Expr::Match { arms, .. } if effectless(expr) => {
            let mut value = None;
            for arm in arms {
                if arm.guard.as_ref().is_some_and(|g| known_bool(g, known) == Some(false)) {
                    continue;
                }
                let v = known_bool(&arm.body, known)?;
                if value.is_some_and(|w| w != v) {
                    return None;
                }
                value = Some(v);
                if arm.guard.is_none() && matches!(arm.pattern, Pattern::Wildcard | Pattern::Var(_)) {
                    break;
                }
            }
            value
        }
        // `if c { a } else { b }` of `bool`s: the side `c` takes, or the
        // value both give.
        Expr::If { cond, then, else_ } if effectless(expr) => match known_bool(cond, known) {
            Some(c) => known_bool(if c { then } else { else_ }, known),
            None => {
                let (a, b) = (known_bool(then, known)?, known_bool(else_, known)?);
                (a == b).then_some(a)
            }
        },
        // `t == ","` where `t` is known to be some literal.
        Expr::Binary { op: op @ (BinOp::Eq | BinOp::Ne), left, right } => {
            let (p, l) = match (&**left, &**right) {
                (p, Expr::Lit(l)) | (Expr::Lit(l), p) if is_place(p) => (p, l),
                _ => return None,
            };
            let case = flow::literal_case(l)?;
            let (_, vs) = known.iter().rev().find(|(k, _)| k == p)?;
            let [v] = vs.as_slice() else { return None };
            if !v.as_str().starts_with("$=") {
                return None;
            }
            Some((*v == case) == (*op == BinOp::Eq))
        }
        // TS types `x && c` as `false` once `c` is, and `true` once both are.
        Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } if effectless(expr) => {
            let decides = *op == BinOp::Or;
            match (known_bool(left, known), known_bool(right, known)) {
                (Some(l), _) if l == decides => Some(l),
                (_, Some(r)) if r == decides => Some(r),
                (Some(l), Some(r)) => Some(l && r || (l || r) && decides),
                _ => None,
            }
        }
        p if is_place(p) => {
            let (_, vs) = known.iter().rev().find(|(k, _)| k == p)?;
            match vs.as_slice() {
                [v] if v.as_str() == TRUE => Some(true),
                [v] if v.as_str() == FALSE => Some(false),
                _ => None,
            }
        }
        _ => None,
    }
}

/// What a place holds past a `?` on it.
pub(super) fn tried_case(on: TryOn) -> Name {
    Name::new(if on == TryOn::Option { SOME } else { OK })
}

/// The body of the one arm a `match` on a narrowed place takes for every
/// variant it may hold. Where that arm binds names, they are read from the
/// place (`r.value`): TS has narrowed it too, and would refuse the test it
/// knows the answer to (`"Err"` and `"Ok"` have no overlap).
pub(super) fn taken_arm(expr: &Expr, known: &Known) -> Option<Expr> {
    let Expr::Match { scrutinee, arms } = expr else { return None };
    let (_, variants) = known.iter().rev().find(|(p, _)| p == &**scrutinee)?;
    // A place known `Some` is read as its payload once narrowed (`o` for
    // `o.unwrap_or(d)`): a `match` testing the payload (`0..=9 => ..`) is
    // not decided by what is known of the option.
    if !arms.iter().all(|a| tests_case(&a.pattern, &variants[0])) {
        return None;
    }
    let pick = |v: &Name| arms.iter().position(|a| admits(&a.pattern, v));
    let first = pick(&variants[0])?;
    let arm = &arms[first];
    if arm.guard.is_some() || !variants.iter().all(|v| pick(v) == Some(first)) {
        return None;
    }
    if arm.pattern.bindings().is_empty() {
        Some(arm.body.clone())
    } else {
        read_in(arm, scrutinee)
    }
}

/// What a `bool` place holds where a test has decided it.
pub(super) const TRUE: &str = "$true";

pub(super) const FALSE: &str = "$false";

/// The cases of `Result` and `Option`, as `Known` names them: no variant of
/// a crate's enum starts with `$`.
pub(super) const OK: &str = "$Ok";

pub(super) const ERR: &str = "$Err";

pub(super) const SOME: &str = "$Some";

pub(super) const NONE: &str = "$None";

/// The variants a pattern on an enum, a `Result`, or an `Option` admits;
/// none for any other pattern.
pub(super) fn variants_of(pattern: &Pattern) -> Vec<Name> {
    match pattern {
        Pattern::Variant { variant, .. } => vec![variant.clone()],
        Pattern::ResultOk(_) => vec![Name::new(OK)],
        Pattern::ResultErr(_) => vec![Name::new(ERR)],
        Pattern::OptionSome(_) => vec![Name::new(SOME)],
        Pattern::OptionNone => vec![Name::new(NONE)],
        Pattern::Or(alts) => {
            let vs: Vec<Name> = alts.iter().flat_map(variants_of).collect();
            if vs.len() == alts.len() {
                vs
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    }
}

pub(super) fn admits(pattern: &Pattern, variant: &Name) -> bool {
    match pattern {
        Pattern::Wildcard | Pattern::Var(_) => true,
        Pattern::Variant { variant: v, bind, .. } => v == variant && bind_admits_all(bind),
        Pattern::ResultOk(p) => variant.as_str() == OK && !p.refutable(),
        Pattern::ResultErr(p) => variant.as_str() == ERR && !p.refutable(),
        Pattern::OptionSome(p) => variant.as_str() == SOME && !p.refutable(),
        Pattern::OptionNone => variant.as_str() == NONE,
        Pattern::Lit(Lit::Bool(b)) => variant.as_str() == if *b { TRUE } else { FALSE },
        Pattern::Or(alts) => alts.iter().any(|p| admits(p, variant)),
        _ => false,
    }
}

/// Whether `pattern` tests the kind of value `case` names (an `Option`'s,
/// a `Result`'s, an enum's, or a `bool`'s), or nothing.
fn tests_case(pattern: &Pattern, case: &Name) -> bool {
    match pattern {
        Pattern::Wildcard | Pattern::Var(_) => true,
        Pattern::OptionSome(_) | Pattern::OptionNone => [SOME, NONE].contains(&case.as_str()),
        Pattern::ResultOk(_) | Pattern::ResultErr(_) => [OK, ERR].contains(&case.as_str()),
        Pattern::Lit(Lit::Bool(_)) => [TRUE, FALSE].contains(&case.as_str()),
        Pattern::Variant { .. } => !case.as_str().starts_with('$'),
        Pattern::Or(alts) => alts.iter().all(|p| tests_case(p, case)),
        _ => false,
    }
}

/// Whether a variant's payload patterns take any payload.
fn bind_admits_all(bind: &VariantBind) -> bool {
    let any = |p: &Pattern| matches!(p, Pattern::Wildcard | Pattern::Var(_));
    match bind {
        VariantBind::Unit => true,
        VariantBind::Tuple(ps) => ps.iter().all(any),
        VariantBind::Struct(ps) => ps.iter().all(|(_, p)| any(p)),
    }
}
