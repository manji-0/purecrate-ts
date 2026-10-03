//! A guard that falls back, joined into its arm's test before printing.
//!
//! `check::accept` tests a guard inside its arm, so `Some(s) if g => a,
//! _ => b` reaches the printer as `Some(s) => if g { a } else { b }, _ => b`,
//! with `b` twice. Where the other arm is that `b` and binds nothing, the
//! two tests join: `if matches!(o, Some(_)) && g { a } else { b }`, the
//! payload read where the test has narrowed it. An inner two-way `match`
//! whose other arm is `b` joins the same way (`Some(n) => match n { 0 => a,
//! _ => b }, None => b`), so a chain of them is one `&&` and one `b`.
//!
//! The tree also copies an arm under each case of a value, where a `match`
//! in it on that value can take one arm only; it is that arm (`narrow`).

use purecrate_ir::{Arm, BinOp, Expr, Lit, Name, Pattern, VariantBind};

use crate::stmt::{is_place, touches};

/// `expr` with every such `match` joined, innermost first.
pub(crate) fn joined(expr: &Expr) -> Expr {
    let mut out = expr.clone();
    narrow(&mut out, &mut Vec::new());
    join(&mut out);
    out
}

/// A place and the variants an enclosing arm has narrowed it to.
type Known = Vec<(Expr, Vec<Name>)>;

/// A `match` on a place an enclosing arm has narrowed, as the arm every
/// one of those variants takes: the decision tree copies `(s, _) => if
/// matches!(s, Done) { a } else { b }` under each case of `s`, where only
/// one side is reachable. A `let` of the place that nothing reads then is
/// left out.
fn narrow(expr: &mut Expr, known: &mut Known) {
    if let Some(taken) = taken_arm(expr, known) {
        *expr = taken;
        return narrow(expr, known);
    }
    match expr {
        // An arm of `A | B` is narrowed for each variant; where the bodies
        // differ, it is one arm per variant (the printer shares the cases of
        // those that still do the same).
        Expr::Match { scrutinee, arms } if is_place(scrutinee) => {
            let mut split = Vec::with_capacity(arms.len());
            for mut arm in std::mem::take(arms) {
                let variants = variants_of(&arm.pattern);
                if variants.is_empty() || touches(&arm.body, root(scrutinee)) {
                    narrow(&mut arm.body, known);
                    split.push(arm);
                    continue;
                }
                let bodies: Vec<Expr> = variants
                    .into_iter()
                    .map(|v| {
                        let mut body = arm.body.clone();
                        known.push(((**scrutinee).clone(), vec![v]));
                        narrow(&mut body, known);
                        known.pop();
                        body
                    })
                    .collect();
                match &arm.pattern {
                    Pattern::Or(alts) if bodies.iter().any(|b| *b != bodies[0]) => {
                        split.extend(alts.iter().zip(bodies).map(|(alt, body)| Arm { pattern: alt.clone(), guard: arm.guard.clone(), body }));
                    }
                    _ => {
                        arm.body = bodies.into_iter().next().expect("a variant");
                        split.push(arm);
                    }
                }
            }
            *arms = split;
        }
        Expr::Let { name, mutable: false, value, then, .. } if is_place(value) && !touches(then, root(value)) => {
            let alias = known.iter().find(|(p, _)| p == &**value).map(|(_, vs)| vs.clone());
            if let Some(vs) = &alias {
                known.push((Expr::Var(name.clone()), vs.clone()));
            }
            narrow(then, known);
            if alias.is_some() {
                known.pop();
                if !mentions(then, name) {
                    *expr = (**then).clone();
                }
            }
        }
        // A closure may run where the narrowing no longer holds.
        Expr::Closure { body, .. } => narrow(body, &mut Vec::new()),
        _ => {
            for child in expr.children_mut() {
                narrow(child, known);
            }
        }
    }
}

/// The body of the one arm a `match` on a narrowed place takes for every
/// variant it may hold, where that arm binds nothing.
fn taken_arm(expr: &Expr, known: &Known) -> Option<Expr> {
    let Expr::Match { scrutinee, arms } = expr else { return None };
    let (_, variants) = known.iter().rev().find(|(p, _)| p == &**scrutinee)?;
    let pick = |v: &Name| arms.iter().position(|a| admits(&a.pattern, v));
    let first = pick(&variants[0])?;
    let arm = &arms[first];
    (arm.guard.is_none() && arm.pattern.bindings().is_empty() && variants.iter().all(|v| pick(v) == Some(first)))
        .then(|| arm.body.clone())
}

/// The variants a pattern on an enum admits; none for any other pattern.
fn variants_of(pattern: &Pattern) -> Vec<Name> {
    match pattern {
        Pattern::Variant { variant, .. } => vec![variant.clone()],
        Pattern::Or(alts) => {
            let vs: Vec<Name> = alts.iter().flat_map(variants_of).collect();
            if vs.len() == alts.len() { vs } else { Vec::new() }
        }
        _ => Vec::new(),
    }
}

fn admits(pattern: &Pattern, variant: &Name) -> bool {
    match pattern {
        Pattern::Wildcard | Pattern::Var(_) => true,
        Pattern::Variant { variant: v, bind, .. } => v == variant && bind_admits_all(bind),
        Pattern::Or(alts) => alts.iter().any(|p| admits(p, variant)),
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

fn mentions(expr: &Expr, name: &Name) -> bool {
    match expr {
        Expr::Var(n) | Expr::Call { callee: purecrate_ir::Callee::Local(n), .. } if n == name => true,
        other => other.children().into_iter().any(|c| mentions(c, name)),
    }
}

fn join(expr: &mut Expr) {
    for child in expr.children_mut() {
        join(child);
    }
    if let Some(joined) = join_match(expr) {
        *expr = joined;
    }
    // A statement with no effect is left out: a value that reads and builds
    // only (`Box::new(P { a, b: a });`), `x = x`, and an `if` whose sides do
    // nothing (what a write never read leaves).
    match expr {
        Expr::Seq { first, then } if pure(first) => *expr = (**then).clone(),
        Expr::Assign { name, value } if reads_only(value, name) => *expr = Expr::Lit(Lit::Unit),
        Expr::If { cond, then, else_ } if pure(cond) && **then == Expr::Lit(Lit::Unit) && **else_ == Expr::Lit(Lit::Unit) => {
            *expr = Expr::Lit(Lit::Unit)
        }
        _ => {}
    }
    // A `let mut` that is no longer written (its only write was `x = x`) is
    // a `const`.
    if let Expr::Let { name, mutable, then, .. } = expr {
        if *mutable && !assigns(then, name) {
            *mutable = false;
        }
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

/// Whether evaluating `expr` can neither panic nor change anything: it reads
/// names and fields, compares, and builds values. Integer arithmetic is a
/// call by now (it may overflow), as is everything else that may panic.
fn pure(expr: &Expr) -> bool {
    let here = matches!(
        expr,
        Expr::Lit(_)
            | Expr::Var(_)
            | Expr::Field { .. }
            | Expr::Construct { .. }
            | Expr::Tuple(_)
            | Expr::Array(_)
            | Expr::Ignored { .. }
            | Expr::Unary { .. }
            | Expr::Binary { .. }
    );
    here && expr.children().into_iter().all(pure)
}

/// `name` itself, or a copy of it (`String::from(&t)`, printed as `t`).
fn reads_only(value: &Expr, name: &Name) -> bool {
    match value {
        Expr::Var(v) => v == name,
        Expr::Call { callee: purecrate_ir::Callee::StringFrom, args } => matches!(args.as_slice(), [a] if reads_only(a, name)),
        _ => false,
    }
}

fn assigns(expr: &Expr, name: &Name) -> bool {
    matches!(expr, Expr::Assign { name: n, .. } if n == name) || expr.children().into_iter().any(|c| assigns(c, name))
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
