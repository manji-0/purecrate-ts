//! A guard that falls back, joined into its arm's test before printing.
//!
//! `check::accept` tests a guard inside its arm, so `Some(s) if g => a,
//! _ => b` reaches the printer as `Some(s) => if g { a } else { b }, _ => b`,
//! with `b` twice. Where the other arm is that `b` and binds nothing, the
//! two tests join: `if matches!(o, Some(_)) && g { a } else { b }`, the
//! payload read where the test has narrowed it. An inner two-way `match`
//! whose other arm is `b` joins the same way (`Some(n) => match n { 0 => a,
//! _ => b }, None => b`), so a chain of them is one `&&` and one `b`.

use purecrate_ir::{Arm, BinOp, Expr, Lit, Name, Pattern, VariantBind};

use crate::stmt::{is_place, touches};

/// `expr` with every such `match` joined, innermost first.
pub(crate) fn joined(expr: &Expr) -> Expr {
    let mut out = expr.clone();
    join(&mut out);
    out
}

fn join(expr: &mut Expr) {
    for child in expr.children_mut() {
        join(child);
    }
    if let Some(joined) = join_match(expr) {
        *expr = joined;
    }
    // `if true { a } else { b }` is `a`: the test reads nothing. Not where
    // the side declares names, which its block kept apart from the rest.
    if let Expr::If { cond, then, else_ } = expr {
        if let Expr::Lit(Lit::Bool(c)) = **cond {
            let side = if c { &**then } else { &**else_ };
            if !declares(side) {
                *expr = side.clone();
            }
        }
    }
}

/// Whether printing `expr` as statements declares a name in its block.
fn declares(expr: &Expr) -> bool {
    match expr {
        Expr::Let { .. } | Expr::Try { .. } => true,
        Expr::Seq { first, then } => declares(first) || declares(then),
        Expr::Match { scrutinee, .. } => !is_place(scrutinee),
        _ => false,
    }
}

fn join_match(expr: &Expr) -> Option<Expr> {
    let Expr::Match { scrutinee, arms } = expr else { return None };
    let [a, b] = arms.as_slice() else { return None };
    if !b.pattern.bindings().is_empty() || a.guard.is_some() || b.guard.is_some() {
        return None;
    }
    let body = read_in(a, scrutinee)?;
    let (cond, then) = inner_choice(&body, &b.body)?;
    let tested = test(scrutinee, &a.pattern)?;
    // A guard of `true` adds nothing to the test.
    let cond = match cond {
        Expr::Lit(Lit::Bool(true)) => tested,
        cond => Expr::Binary { op: BinOp::And, left: Box::new(tested), right: Box::new(cond) },
    };
    let then = read_lets(then);
    let or = |l: Expr, r: Expr| Expr::Binary { op: BinOp::Or, left: Box::new(l), right: Box::new(r) };
    let and = |l: Expr, r: Expr| Expr::Binary { op: BinOp::And, left: Box::new(l), right: Box::new(r) };
    // `c ? true : false` is `c`, as `fold` prints a `match` of `bool`s.
    Some(match (&then, &b.body) {
        (Expr::Lit(Lit::Bool(true)), Expr::Lit(Lit::Bool(false))) => cond,
        (Expr::Lit(Lit::Bool(true)), e) if !e.needs_statements() => or(cond, e.clone()),
        (t, Expr::Lit(Lit::Bool(false))) if !t.needs_statements() => and(cond, then),
        _ => Expr::If { cond: Box::new(cond), then: Box::new(then), else_: Box::new(b.body.clone()) },
    })
}

/// `let x = place; e` as `e` reading the place, where `e` does not reassign
/// the variable the place starts from: the decision tree binds a tuple's
/// elements so, and a value that is one expression stays one.
fn read_lets(expr: Expr) -> Expr {
    match expr {
        Expr::Let { name, mutable: false, value, then, .. } if is_place(&value) && !touches(&then, root(&value)) => {
            read_lets(crate::expr::subst(&then, &name, &value))
        }
        other => other,
    }
}

fn root(place: &Expr) -> &Name {
    match place {
        Expr::Field { base, .. } => root(base),
        Expr::Var(n) => n,
        _ => unreachable!("a place starts from a variable"),
    }
}

/// The test and the value of `body` where it is `if c { t } else { fallback }`
/// or a two-way `match` whose other arm is `fallback`.
fn inner_choice(body: &Expr, fallback: &Expr) -> Option<(Expr, Expr)> {
    match body {
        Expr::If { cond, then, else_ } if **else_ == *fallback => Some(((**cond).clone(), (**then).clone())),
        Expr::Match { scrutinee, arms } => {
            let [x, y] = arms.as_slice() else { return None };
            if y.body != *fallback || !y.pattern.bindings().is_empty() || x.guard.is_some() || y.guard.is_some() {
                return None;
            }
            Some((test(scrutinee, &x.pattern)?, read_in(x, scrutinee)?))
        }
        _ => None,
    }
}

/// `matches!(scrutinee, pattern)`, its bindings left out, where the printer
/// writes it as one test (`o !== null`, `s.kind === "A"`, `n === 0`).
fn test(scrutinee: &Expr, pattern: &Pattern) -> Option<Expr> {
    if !is_place(scrutinee) {
        return None;
    }
    let tested = match pattern {
        Pattern::OptionSome(_) => Pattern::OptionSome(Box::new(Pattern::Wildcard)),
        Pattern::ResultOk(_) => Pattern::ResultOk(Box::new(Pattern::Wildcard)),
        Pattern::ResultErr(_) => Pattern::ResultErr(Box::new(Pattern::Wildcard)),
        Pattern::OptionNone => Pattern::OptionNone,
        Pattern::Variant { ty, variant, bind } => Pattern::Variant {
            ty: ty.clone(),
            variant: variant.clone(),
            bind: match bind {
                VariantBind::Unit => VariantBind::Unit,
                VariantBind::Tuple(ps) => VariantBind::Tuple(vec![Pattern::Wildcard; ps.len()]),
                VariantBind::Struct(ps) => VariantBind::Struct(ps.iter().map(|(f, _)| (f.clone(), Pattern::Wildcard)).collect()),
            },
        },
        p if p.is_lit_case() => p.clone(),
        _ => return None,
    };
    Some(Expr::Match {
        scrutinee: Box::new(scrutinee.clone()),
        arms: vec![
            Arm { pattern: tested, guard: None, body: Expr::Lit(Lit::Bool(true)) },
            Arm { pattern: Pattern::Wildcard, guard: None, body: Expr::Lit(Lit::Bool(false)) },
        ],
    })
}

/// The arm's body with each name its pattern binds read from `scrutinee`,
/// where the body does not reassign the variable the place starts from (TS
/// keeps the narrowing only then).
fn read_in(arm: &Arm, scrutinee: &Expr) -> Option<Expr> {
    let mut root = scrutinee;
    while let Expr::Field { base, .. } = root {
        root = base;
    }
    let Expr::Var(var) = root else { return None };
    if touches(&arm.body, var) {
        return None;
    }
    let field = |base: &Expr, name: &str| Expr::Field { base: Box::new(base.clone()), name: Name::new(name) };
    let bind = crate::expr::bind_in;
    let mut body = arm.body.clone();
    match &arm.pattern {
        Pattern::OptionSome(p) => bind(p, scrutinee.clone(), &mut body)?,
        Pattern::ResultOk(p) => bind(p, field(scrutinee, "value"), &mut body)?,
        Pattern::ResultErr(p) => bind(p, field(scrutinee, "error"), &mut body)?,
        Pattern::Variant { bind: VariantBind::Unit, .. } | Pattern::OptionNone => {}
        Pattern::Variant { bind: VariantBind::Tuple(ps), .. } => {
            for (k, p) in ps.iter().enumerate() {
                let place = if ps.len() == 1 {
                    field(scrutinee, "value")
                } else {
                    field(&field(scrutinee, "content"), &format!("[{k}]"))
                };
                bind(p, place, &mut body)?;
            }
        }
        Pattern::Variant { bind: VariantBind::Struct(ps), .. } => {
            for (f, p) in ps {
                bind(p, field(scrutinee, f.as_str()), &mut body)?;
            }
        }
        p if p.is_lit_case() => {}
        _ => return None,
    }
    Some(body)
}
