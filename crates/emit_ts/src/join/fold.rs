//! Folding what deciding leaves: a decided test, unread bindings, effects
//! kept as statements, equal sides, a rebuilt value.

use super::*;

/// `if true { a } else { b }` as `a` where `a` declares names, which the
/// fold in `join` leaves: safe unless a statement of the same block follows
/// (`followed`), whose names `a`'s could meet. A value or a tail is printed
/// in a block of its own or ends its block. TS refuses `if (true)` as
/// unreachable code on the other side.
pub(super) fn decided(expr: &mut Expr, followed: bool) {
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
pub(super) fn unread(expr: &mut Expr) {
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
pub(super) fn effects(expr: Expr) -> Expr {
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
pub(super) fn constant(cond: &Expr) -> Option<(Expr, bool)> {
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

/// `==`, `!=`, `<`, `<=`, `>`, `>=`: no effect of their own.
fn is_compare(op: BinOp) -> bool {
    matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
}

/// `first; then`, without a `first` that is `()`.
pub(super) fn sequence(first: Expr, then: Expr) -> Expr {
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
pub(super) fn unstated(expr: &mut Expr, block: bool) {
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
pub(super) fn same_sides(expr: &mut Expr) {
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
pub(super) fn rebuilt(expr: &mut Expr) {
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
pub(super) fn same_variant(expr: &mut Expr) {
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

/// `pattern` with each name `read` says is unread made `_`.
pub(super) fn unbind(mut pattern: Pattern, read: &impl Fn(&Name) -> bool) -> Pattern {
    fn go(p: &mut Pattern, read: &dyn Fn(&Name) -> bool) {
        match p {
            Pattern::Var(n) if !read(n) => *p = Pattern::Wildcard,
            p => p.children_mut().into_iter().for_each(|p| go(p, read)),
        }
    }
    go(&mut pattern, read);
    pattern
}

/// `1 == 65535` as `false`: TS refuses comparing two literals it knows
/// apart.
pub(super) fn literal_compare(expr: &mut Expr) {
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

pub(super) fn replace_var(expr: &mut Expr, name: &Name, with: &Expr) {
    if *expr == Expr::Var(name.clone()) {
        *expr = with.clone();
        return;
    }
    for child in expr.children_mut() {
        replace_var(child, name, with);
    }
}

/// `let x = place; e` as `e` reading the place, where `e` does not reassign
/// the variable the place starts from: the decision tree binds a tuple's
/// elements so, and a value that is one expression stays one.
pub(super) fn read_lets(expr: Expr) -> Expr {
    match expr {
        Expr::Let { name, mutable: false, value, then, .. } if is_place(&value) && !touches(&then, root(&value)) => {
            read_lets(crate::expr::subst(&then, &name, &value))
        }
        other => other,
    }
}
