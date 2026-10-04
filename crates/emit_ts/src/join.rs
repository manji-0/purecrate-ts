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

use purecrate_ir::{Arm, BinOp, Expr, Lit, Name, Pattern, TryOn, VariantBind};

use crate::stmt::{breaks, is_place, touches};

mod flow;

/// `expr` with every such `match` joined, innermost first.
pub(crate) fn joined(expr: &Expr) -> Expr {
    let mut out = expr.clone();
    unstated(&mut out, true);
    same_sides(&mut out);
    // What the cleanup simplifies (`match o { Some(_) if c => true, _ =>
    // false }` to `c`) may let narrowing decide more: until it settles.
    for _ in 0..3 {
        let before = out.clone();
        flow::flow_fn(&mut out);
        join(&mut out);
        unread(&mut out);
        decided(&mut out, false);
        unread(&mut out);
        if out == before {
            break;
        }
    }
    out
}

/// `if true { a } else { b }` as `a` where `a` declares names, which the
/// fold in `join` leaves: safe unless a statement of the same block follows
/// (`followed`), whose names `a`'s could meet. A value or a tail is printed
/// in a block of its own or ends its block. TS refuses `if (true)` as
/// unreachable code on the other side.
fn decided(expr: &mut Expr, followed: bool) {
    if let Expr::If { cond, then, else_ } = expr {
        if let Expr::Lit(Lit::Bool(c)) = **cond {
            let side = if c { &**then } else { &**else_ };
            if !followed || !declares(side) {
                *expr = side.clone();
                return decided(expr, followed);
            }
        }
    }
    match expr {
        Expr::Seq { first, then } => {
            decided(first, true);
            decided(then, followed);
        }
        Expr::Let { value, then, .. } => {
            decided(value, false);
            decided(then, followed);
        }
        other => {
            for child in other.children_mut() {
                decided(child, false);
            }
        }
    }
}

/// What narrowing and joining folded may leave a name unread: an arm's
/// binding goes, and a `let` runs its value as a statement (or not at all,
/// where it only reads). TS refuses an unread `const`.
fn unread(expr: &mut Expr) {
    for child in expr.children_mut() {
        unread(child);
    }
    literal_compare(expr);
    match expr {
        Expr::Match { scrutinee, arms } => {
            for arm in arms.iter_mut() {
                let read = |n: &Name| mentions(&arm.body, n) || arm.guard.as_ref().is_some_and(|g| mentions(g, n));
                arm.pattern = unbind(std::mem::replace(&mut arm.pattern, Pattern::Wildcard), &read);
            }
            // Every arm the same, binding nothing (`Some(_) => false, None =>
            // false`): the value, where the scrutinee does nothing.
            let same =
                arms.iter().all(|a| a.guard.is_none() && a.pattern.bindings().is_empty() && a.body == arms[0].body);
            if same && !arms.is_empty() && effectless(scrutinee) {
                *expr = arms[0].body.clone();
            }
        }
        // `x && false` where `x` does nothing is `false` (TS: unreachable
        // code); `true && x` is `x`, and the same for `||`.
        Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } => {
            let unit = *op == BinOp::And; // `true && x` is `x`
            let lit = |e: &Expr| match e {
                Expr::Lit(Lit::Bool(b)) => Some(*b),
                _ => None,
            };
            *expr = match (lit(left), lit(right)) {
                (Some(l), _) if l == unit => (**right).clone(),
                (Some(l), _) => Expr::Lit(Lit::Bool(l)),
                (_, Some(r)) if r == unit => (**left).clone(),
                (_, Some(r)) if pure(left) => Expr::Lit(Lit::Bool(r)),
                _ => return,
            };
        }
        // A test the cleanup above or folding decided (`k < n &&
        // o.map(f).is_none()` once `o` is known `Some`): what runs before the
        // answer runs alone, and the side not taken goes. TS refuses the code
        // it knows is unreachable.
        Expr::If { cond, then, else_ } if constant(cond).is_some() => {
            let (before, taken) = constant(cond).expect("checked above");
            let side = if taken { &**then } else { &**else_ };
            let side = if declares(side) {
                // The side declares names, which its block keeps apart from
                // what follows (`decided`): `if (true) { .. }` alone.
                Expr::If {
                    cond: Box::new(Expr::Lit(Lit::Bool(true))),
                    then: Box::new(side.clone()),
                    else_: Box::new(Expr::Lit(Lit::Unit)),
                }
            } else {
                side.clone()
            };
            *expr = sequence(before, side);
        }
        Expr::While { cond, .. } if matches!(constant(cond), Some((_, false))) => {
            *expr = constant(cond).expect("checked above").0;
        }
        // What follows a statement that always leaves never runs.
        Expr::Seq { first, .. } if ends(first) => *expr = (**first).clone(),
        Expr::Let { value, .. } if ends(value) => *expr = (**value).clone(),
        // A `let mut` nothing reads: its writes run their values alone.
        Expr::Let { name, value, then, .. } if !mentions(then, name) => {
            let then = unassigned((**then).clone(), name);
            *expr = sequence(effects((**value).clone()), then);
        }
        // A statement's value nothing reads: only what it does is left.
        Expr::Seq { first, then } => {
            *expr = sequence(effects((**first).clone()), (**then).clone());
        }
        // A loop's body is run for what it does: its value is unread too.
        Expr::While { body, .. } | Expr::For { body, .. } | Expr::ForEach { body, .. } => {
            **body = effects(std::mem::replace(&mut **body, Expr::Lit(Lit::Unit)));
        }
        _ => {}
    }
}

/// `expr` run for what it does alone, its value unread: what does nothing
/// goes (an `if` or `match` whose sides only read, as `unwrap_or(d)`'s
/// leaves where its value is not kept, with `d` then run on its own), and a
/// `let` nothing reads runs its value.
fn effects(expr: Expr) -> Expr {
    let unit = Expr::Lit(Lit::Unit);
    match expr {
        e if effectless(&e) => unit,
        // A comparison or `!` whose value is unread: its operands run, in
        // order (`k < Int.i32.div(100, n);` is only the division).
        Expr::Binary { op, left, right } if is_compare(op) => sequence(effects(*left), effects(*right)),
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => effects(*expr),
        // `a && b` runs `b` only where `a` holds.
        Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } => {
            let right = effects(*right);
            if right == unit {
                effects(*left)
            } else if op == BinOp::And {
                Expr::If { cond: left, then: Box::new(right), else_: Box::new(unit) }
            } else {
                Expr::If { cond: left, then: Box::new(unit), else_: Box::new(right) }
            }
        }
        Expr::Seq { first, then } => sequence(effects(*first), effects(*then)),
        Expr::Let { name, mutable, ty, value, then } => {
            let then = effects(*then);
            if mentions(&then, &name) {
                Expr::Let { name, mutable, ty, value, then: Box::new(then) }
            } else {
                sequence(effects(*value), unassigned(then, &name))
            }
        }
        Expr::If { cond, then, else_ } => {
            let (then, else_) = (effects(*then), effects(*else_));
            if then == unit && else_ == unit {
                effects(*cond)
            } else {
                Expr::If { cond, then: Box::new(then), else_: Box::new(else_) }
            }
        }
        Expr::Match { scrutinee, arms } => {
            let arms: Vec<Arm> = arms.into_iter().map(|a| Arm { body: effects(a.body), ..a }).collect();
            if arms.iter().all(|a| a.body == unit && a.guard.is_none()) {
                effects(*scrutinee)
            } else {
                Expr::Match { scrutinee, arms }
            }
        }
        other => other,
    }
}

/// A test whose answer is known: what still runs before it (`()` if
/// nothing), and the answer. `a && false` runs `a`; `false && b` runs
/// nothing, as `b` is never evaluated.
fn constant(cond: &Expr) -> Option<(Expr, bool)> {
    let unit = Expr::Lit(Lit::Unit);
    match cond {
        Expr::Lit(Lit::Bool(b)) => Some((unit, *b)),
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => constant(expr).map(|(e, b)| (e, !b)),
        Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } => {
            // `false` for `&&` and `true` for `||` decide the test alone.
            let decides = *op == BinOp::Or;
            match constant(left) {
                Some((e, b)) if b == decides => Some((e, b)),
                Some((e, _)) => {
                    let (r, b) = constant(right)?;
                    (r == unit).then_some((e, b))
                }
                None => match constant(right) {
                    Some((r, b)) if b == decides && r == unit => Some((effects((**left).clone()), b)),
                    _ => None,
                },
            }
        }
        _ => None,
    }
}

/// Whether running `expr` always leaves the block: a `return`, `break`, or
/// `continue`, last or on every side.
fn ends(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) | Expr::Break | Expr::Continue => true,
        Expr::Seq { first, then } => ends(first) || ends(then),
        Expr::Let { value, then, .. } => ends(value) || ends(then),
        Expr::Assign { value, .. } => ends(value),
        Expr::If { cond, then, else_ } => match constant(cond) {
            Some((_, b)) => ends(if b { then } else { else_ }),
            None => ends(then) && ends(else_),
        },
        Expr::Match { arms, .. } => !arms.is_empty() && arms.iter().all(|a| ends(&a.body)),
        Expr::At { expr, .. } => ends(expr),
        // `while true` nothing breaks out of (what a `?` in a loop's test
        // leaves when the test is decided): only a `return` leaves it.
        Expr::While { cond, body } => matches!(**cond, Expr::Lit(Lit::Bool(true))) && !breaks(body),
        _ => false,
    }
}

/// `==`, `!=`, `<`, `<=`, `>`, `>=`: no effect of their own.
fn is_compare(op: BinOp) -> bool {
    matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
}

/// `first; then`, without a `first` that is `()`.
fn sequence(first: Expr, then: Expr) -> Expr {
    if first == Expr::Lit(Lit::Unit) {
        then
    } else {
        Expr::Seq { first: Box::new(first), then: Box::new(then) }
    }
}

/// `expr` with each write of `name`, which nothing reads, as its value run
/// alone.
fn unassigned(expr: Expr, name: &Name) -> Expr {
    let mut expr = expr;
    fn walk(e: &mut Expr, name: &Name) {
        if let Expr::Assign { name: n, value } = e {
            if n == name {
                *e = effects(std::mem::replace(&mut **value, Expr::Lit(Lit::Unit)));
                return walk(e, name);
            }
        }
        e.children_mut().into_iter().for_each(|c| walk(c, name));
    }
    walk(&mut expr, name);
    expr
}

/// `expr` without the comments that are not above a statement: one above
/// an operand, an argument, or a `let`'s value has no line of its own, and
/// the printer reads such a value's precedence from its node, which a
/// comment would hide (`c && { // .. \n a || b }` is `c && (a || b)`).
/// `block` is whether `expr` stands where a statement may.
fn unstated(expr: &mut Expr, block: bool) {
    if !block {
        while let Expr::Seq { first, then } = expr {
            if !matches!(**first, Expr::Comment(_)) {
                break;
            }
            *expr = std::mem::replace(&mut **then, Expr::Unreachable);
        }
    }
    match expr {
        Expr::Seq { first, then } => {
            unstated(first, true);
            unstated(then, true);
        }
        Expr::Let { value, then, .. } => {
            unstated(value, false);
            unstated(then, true);
        }
        Expr::If { cond, then, else_ } => {
            unstated(cond, false);
            unstated(then, true);
            unstated(else_, true);
        }
        Expr::Match { scrutinee, arms } => {
            unstated(scrutinee, false);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    unstated(guard, false);
                }
                unstated(&mut arm.body, true);
            }
        }
        Expr::For { start, end, body, .. } => {
            unstated(start, false);
            unstated(end, false);
            unstated(body, true);
        }
        Expr::ForEach { source, body, .. } => {
            unstated(source, false);
            unstated(body, true);
        }
        Expr::While { cond, body } => {
            unstated(cond, false);
            unstated(body, true);
        }
        Expr::Closure { body, .. } => unstated(body, true),
        Expr::Ignored { expr: inner, .. } => unstated(inner, block),
        other => other.children_mut().into_iter().for_each(|c| unstated(c, false)),
    }
}

/// `if c { p } else { p }` as the place `p` where `c` does nothing, and
/// `{ let t = p; t }` as `p`: TS types either as `p` narrowed, and a place is
/// what `narrow` follows through the `let` that holds it.
fn same_sides(expr: &mut Expr) {
    for child in expr.children_mut() {
        same_sides(child);
    }
    match expr {
        Expr::If { cond, then, else_ } if then == else_ && is_place(then) && effectless(cond) => {
            *expr = (**then).clone()
        }
        Expr::Let { name, mutable: false, value, then, .. } if is_place(value) && **then == Expr::Var(name.clone()) => {
            *expr = (**value).clone()
        }
        _ => {}
    }
}

/// In an arm of a `match` on a place that binds every field of its variant
/// to a name, the variant built again from those names in order
/// (`Lines::Cons(head, rest) => { let lines = Lines::Cons(head, rest); .. }`,
/// what moving a value out of a `match` costs in Rust) is the place: a value
/// is never changed. A name nothing reads then is `_`.
fn rebuilt(expr: &mut Expr) {
    let Expr::Match { scrutinee, arms } = expr else { return };
    if !is_place(scrutinee) {
        return;
    }
    let mut root = &**scrutinee;
    while let Expr::Field { base, .. } = root {
        root = base;
    }
    let Expr::Var(root) = root.clone() else { return };
    // Not under a guard, which may read a name the arm then drops.
    for arm in arms.iter_mut().filter(|a| a.guard.is_none()) {
        let Pattern::Variant { ty, variant, bind } = &mut arm.pattern else { continue };
        let names: Vec<(Option<Name>, Name)> = match bind {
            VariantBind::Tuple(ps) => ps
                .iter()
                .map(|p| match p {
                    Pattern::Var(n) => Some((None, n.clone())),
                    _ => None,
                })
                .collect::<Option<_>>(),
            VariantBind::Struct(ps) => ps
                .iter()
                .map(|(f, p)| match p {
                    Pattern::Var(n) => Some((Some(f.clone()), n.clone())),
                    _ => None,
                })
                .collect::<Option<_>>(),
            VariantBind::Unit => None,
        }
        .unwrap_or_default();
        if names.is_empty() || touches(&arm.body, &root) || names.iter().any(|(_, n)| touches(&arm.body, n)) {
            continue;
        }
        let is_copy = |e: &Expr| match e {
            Expr::Construct { ty: t, variant: Some(v), fields, base: None } if t == ty && v == variant => {
                match fields {
                    purecrate_ir::Fields::Positional(xs) => {
                        xs.len() == names.len()
                            && xs.iter().zip(&names).all(|(x, (_, n))| matches!(x, Expr::Var(v) if v == n))
                    }
                    purecrate_ir::Fields::Named(xs) => {
                        xs.len() == names.len()
                            && names.iter().all(|(f, n)| {
                                xs.iter().any(|(g, x)| Some(g) == f.as_ref() && matches!(x, Expr::Var(v) if v == n))
                            })
                    }
                    purecrate_ir::Fields::Unit => false,
                }
            }
            _ => false,
        };
        fn replace(e: &mut Expr, is_copy: &dyn Fn(&Expr) -> bool, with: &Expr) -> bool {
            if is_copy(e) {
                *e = with.clone();
                return true;
            }
            let mut any = false;
            for c in e.children_mut() {
                any |= replace(c, is_copy, with);
            }
            any
        }
        if !replace(&mut arm.body, &is_copy, scrutinee) {
            continue;
        }
        let unread = |n: &Name| !mentions(&arm.body, n);
        match bind {
            VariantBind::Tuple(ps) => ps.iter_mut().for_each(|p| {
                if matches!(p, Pattern::Var(n) if unread(n)) {
                    *p = Pattern::Wildcard;
                }
            }),
            VariantBind::Struct(ps) => ps.iter_mut().for_each(|(_, p)| {
                if matches!(p, Pattern::Var(n) if unread(n)) {
                    *p = Pattern::Wildcard;
                }
            }),
            VariantBind::Unit => {}
        }
    }
}

/// An arm of a `match` on a place that builds the unit variant it matched
/// (`Ordering::Less => Ordering::Less`) is the place itself: a value is
/// never changed, so the two are equal. Arms that read it then share
/// their cases.
fn same_variant(expr: &mut Expr) {
    let Expr::Match { scrutinee, arms } = expr else { return };
    if !is_place(scrutinee) {
        return;
    }
    for arm in arms.iter_mut().filter(|a| a.guard.is_none()) {
        let Pattern::Variant { ty, variant, bind: VariantBind::Unit } = &arm.pattern else { continue };
        if matches!(&arm.body, Expr::Construct { ty: t, variant: Some(v), fields: purecrate_ir::Fields::Unit, base: None }
            if t == ty && v == variant)
        {
            arm.body = (**scrutinee).clone();
        }
    }
    // Arms of variants that bind nothing and do the same are one `A | B`
    // arm, where the first of them stood: the variants are apart, so the
    // order of the arms does not choose.
    let plain = |p: &Pattern| match p {
        Pattern::Variant { bind: VariantBind::Unit, .. } => true,
        Pattern::Or(alts) => alts.iter().all(|a| matches!(a, Pattern::Variant { bind: VariantBind::Unit, .. })),
        _ => false,
    };
    if arms.iter().any(|a| a.guard.is_some() || !plain(&a.pattern)) {
        return;
    }
    let mut merged: Vec<Arm> = Vec::with_capacity(arms.len());
    for arm in arms.drain(..) {
        match merged.iter_mut().find(|m| m.body == arm.body) {
            Some(m) => {
                let mut alts = match std::mem::replace(&mut m.pattern, Pattern::Wildcard) {
                    Pattern::Or(alts) => alts,
                    one => vec![one],
                };
                match arm.pattern {
                    Pattern::Or(more) => alts.extend(more),
                    one => alts.push(one),
                }
                m.pattern = Pattern::Or(alts);
            }
            None => merged.push(arm),
        }
    }
    *arms = merged;
}

/// A place and the variants an enclosing arm has narrowed it to.
type Known = Vec<(Expr, Vec<Name>)>;

/// The arm a `match` on `None`, `Some(e)`, `Ok(e)`, or `Err(e)` takes, with
/// `e` bound to the name its pattern gives it. `None` where an arm before
/// it may take the value, a guard decides, or the payload's pattern is
/// neither a name nor `_` over a value that cannot do anything.
fn constructed_arm(expr: &Expr) -> Option<Expr> {
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
fn decide_on(expr: &mut Expr, name: &Name, value: &Expr) {
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

/// `pattern` with each name `read` says is unread made `_`.
fn unbind(pattern: Pattern, read: &impl Fn(&Name) -> bool) -> Pattern {
    let inner = |p: Pattern| Box::new(unbind(p, read));
    match pattern {
        Pattern::Var(n) if !read(&n) => Pattern::Wildcard,
        Pattern::OptionSome(p) => Pattern::OptionSome(inner(*p)),
        Pattern::ResultOk(p) => Pattern::ResultOk(inner(*p)),
        Pattern::ResultErr(p) => Pattern::ResultErr(inner(*p)),
        Pattern::Variant { ty, variant, bind: VariantBind::Tuple(ps) } => Pattern::Variant {
            ty,
            variant,
            bind: VariantBind::Tuple(ps.into_iter().map(|p| unbind(p, read)).collect()),
        },
        Pattern::Variant { ty, variant, bind: VariantBind::Struct(ps) } => Pattern::Variant {
            ty,
            variant,
            bind: VariantBind::Struct(ps.into_iter().map(|(f, p)| (f, unbind(p, read))).collect()),
        },
        other => other,
    }
}

/// `1 == 65535` as `false`: TS refuses comparing two literals it knows
/// apart.
fn literal_compare(expr: &mut Expr) {
    if let Expr::Binary { op: op @ (BinOp::Eq | BinOp::Ne), left, right } = expr {
        let int = |e: &Expr| match e {
            Expr::Lit(Lit::Int { value, .. }) => Some(*value),
            Expr::Unary { op: purecrate_ir::UnOp::Neg, expr } => match &**expr {
                Expr::Lit(Lit::Int { value, .. }) => Some(-*value),
                _ => None,
            },
            _ => None,
        };
        // `{ let v = 7; v }` is printed as `7`.
        let (left, right) = (crate::stmt::peel_identity(left), crate::stmt::peel_identity(right));
        let same = match (left, right) {
            (l, r) if int(l).is_some() && int(r).is_some() => Some(int(l) == int(r)),
            (Expr::Lit(Lit::Str(a)), Expr::Lit(Lit::Str(b))) => Some(a == b),
            (Expr::Lit(Lit::Char(a)), Expr::Lit(Lit::Char(b))) => Some(a == b),
            (Expr::Lit(Lit::Bool(a)), Expr::Lit(Lit::Bool(b))) => Some(a == b),
            _ => None,
        };
        if let Some(same) = same {
            *expr = Expr::Lit(Lit::Bool(same == (*op == BinOp::Eq)));
        }
    }
}

/// `expr` as `None`, `Some(e)`, `Ok(e)`, or `Err(e)`, seeing through
/// `let t = place;` around it, `t` read as the place.
fn as_constructor(expr: &Expr) -> Option<Expr> {
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

fn replace_var(expr: &mut Expr, name: &Name, with: &Expr) {
    if *expr == Expr::Var(name.clone()) {
        *expr = with.clone();
        return;
    }
    for child in expr.children_mut() {
        replace_var(child, name, with);
    }
}

/// The case `None`, `Some(e)`, `Ok(e)`, or `Err(e)` builds.
fn constructed_case(expr: &Expr) -> Option<Name> {
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

thread_local! {
    /// The `let t = place` around the expression `narrow` is in, innermost
    /// last, each with a place nothing below it writes.
    static ALIASES: std::cell::RefCell<Vec<(Name, Expr)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The place a name `let t = place` binds holds, followed to its end.
fn aliased(expr: &Expr) -> Option<Expr> {
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
fn failed_try(place: &Expr, on: TryOn, known: &Known) -> Option<Expr> {
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
fn decide(cond: &mut Expr, known: &Known) {
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
fn known_bool(expr: &Expr, known: &Known) -> Option<bool> {
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
fn tried_case(on: TryOn) -> Name {
    Name::new(if on == TryOn::Option { SOME } else { OK })
}

/// The body of the one arm a `match` on a narrowed place takes for every
/// variant it may hold. Where that arm binds names, they are read from the
/// place (`r.value`): TS has narrowed it too, and would refuse the test it
/// knows the answer to (`"Err"` and `"Ok"` have no overlap).
fn taken_arm(expr: &Expr, known: &Known) -> Option<Expr> {
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
const TRUE: &str = "$true";
const FALSE: &str = "$false";
/// The cases of `Result` and `Option`, as `Known` names them: no variant of
/// a crate's enum starts with `$`.
const OK: &str = "$Ok";
const ERR: &str = "$Err";
const SOME: &str = "$Some";
const NONE: &str = "$None";

/// The variants a pattern on an enum, a `Result`, or an `Option` admits;
/// none for any other pattern.
fn variants_of(pattern: &Pattern) -> Vec<Name> {
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

fn admits(pattern: &Pattern, variant: &Name) -> bool {
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

pub(crate) fn mentions(expr: &Expr, name: &Name) -> bool {
    expr.any(|e| matches!(e, Expr::Var(n) | Expr::Call { callee: purecrate_ir::Callee::Local(n), .. } if n == name))
}

fn join(expr: &mut Expr) {
    for child in expr.children_mut() {
        join(child);
    }
    rebuilt(expr);
    same_variant(expr);
    if let Some(joined) = join_match(expr) {
        *expr = joined;
    }
    // A statement with no effect is left out: a value that reads and builds
    // only (`Box::new(P { a, b: a });`), `x = x`, and an `if` whose sides do
    // nothing (what a write never read leaves).
    match expr {
        Expr::Seq { first, then } if pure(first) => *expr = (**then).clone(),
        Expr::Assign { name, value } if reads_only(value, name) => *expr = Expr::Lit(Lit::Unit),
        Expr::If { cond, then, else_ }
            if pure(cond) && **then == Expr::Lit(Lit::Unit) && **else_ == Expr::Lit(Lit::Unit) =>
        {
            *expr = Expr::Lit(Lit::Unit)
        }
        _ => {}
    }
    // A `let mut` that is no longer written (its only write was `x = x`) is
    // a `const`.
    if let Expr::Let { name, mutable, then, .. } = expr {
        if *mutable && !then.assigns(name) {
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

/// Whether evaluating `expr` can neither panic nor change anything, as
/// `pure`, through `Option` and `Result` built, tested, and matched.
fn effectless(expr: &Expr) -> bool {
    use purecrate_ir::Callee;
    match expr {
        Expr::Call {
            callee:
                Callee::OptionSome
                | Callee::OptionNone
                | Callee::ResultOk
                | Callee::ResultErr
                | Callee::OptionIsSome
                | Callee::OptionIsNone,
            args,
        } => args.iter().all(effectless),
        // Reads that cannot panic: a string's `len`, `as_str`, tests, and
        // pieces; a `Vec`'s `len` and `is_empty`; `String::from`. (Not
        // `char` methods: `to_digit` panics past radix 36.)
        Expr::Call { callee: Callee::Str(_) | Callee::VecLen | Callee::VecIsEmpty | Callee::StringFrom, args } => {
            args.iter().all(effectless)
        }
        Expr::Let { value, then, .. } => effectless(value) && effectless(then),
        Expr::Match { scrutinee, arms } => {
            effectless(scrutinee) && arms.iter().all(|a| a.guard.as_ref().is_none_or(effectless) && effectless(&a.body))
        }
        Expr::If { cond, then, else_ } => effectless(cond) && effectless(then) && effectless(else_),
        // Comparisons, `!`, `&&`, `||`: integer arithmetic is a call by now.
        Expr::Binary { left, right, .. } => effectless(left) && effectless(right),
        Expr::Unary { expr, .. } => effectless(expr),
        e => pure(e),
    }
}

/// `name` itself, or a copy of it (`String::from(&t)`, printed as `t`).
fn reads_only(value: &Expr, name: &Name) -> bool {
    match value {
        Expr::Var(v) => v == name,
        Expr::Call { callee: purecrate_ir::Callee::StringFrom, args } => {
            matches!(args.as_slice(), [a] if reads_only(a, name))
        }
        _ => false,
    }
}

/// Whether printing `expr` as statements declares a name in its block.
pub(crate) fn declares(expr: &Expr) -> bool {
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
                VariantBind::Struct(ps) => {
                    VariantBind::Struct(ps.iter().map(|(f, _)| (f.clone(), Pattern::Wildcard)).collect())
                }
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
