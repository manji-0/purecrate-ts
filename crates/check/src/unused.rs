//! Bindings the Rust leaves unused, so the printed TS passes a consumer's
//! `noUnusedLocals` and `noUnusedParameters`. rustc only warns about them
//! (and says nothing with a leading `_`); TS rejects every unread `const`,
//! whatever its name, but not a parameter or `for..of` variable that starts
//! with `_`.
//!
//! A binding is unused when it is not read in its JS scope (a later binding
//! of the same name in another arm does not count). An unused `let` keeps
//! its value as a statement, so what Rust evaluates (and may panic on) is
//! still evaluated.

use std::collections::HashSet;

use purecrate_ir::{Arm, BinOp, Callee, ClosureParam, Crate, Expr, Fn, IntOp, Item, Name, Param, Pattern, UnOp};

pub fn drop_unused(krate: Crate) -> Crate {
    let items: HashSet<String> = krate.items.iter().map(|item| item.name().as_str().to_string()).collect();
    let dropped = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(f) => Item::Fn(drop_in_fn(f, &items)),
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), dropped)
}

/// Repeated until nothing changes: dropping `let x = $v1` leaves `$v1`,
/// bound by the arm pattern `check::tuple` made, unused in turn.
fn drop_in_fn(f: Fn, items: &HashSet<String>) -> Fn {
    let mut taken: HashSet<String> = items.clone();
    taken.extend(f.params.iter().map(|p| p.name.as_str().to_string()));
    let mut body = f.body;
    loop {
        let mut cx = Cx { taken: taken.clone() };
        let next = cx.expr(body.clone());
        if next == body {
            let params = f
                .params
                .into_iter()
                .map(|p| {
                    let used = used_in(&next, p.name.as_str());
                    Param { name: cx.param_name(p.name, used), ty: p.ty }
                })
                .collect();
            return Fn { params, body: next, ..f };
        }
        body = next;
    }
}

/// Whether `expr` is, or ends in, `let $r: Result<..> = match ..; $r`, as
/// `ok_or` builds it.
fn names_result(expr: &Expr) -> bool {
    match expr {
        Expr::Let { name, ty: Some(purecrate_ir::Ty::Result { .. }), value, then, .. }
            if matches!(value.as_ref(), Expr::Match { .. }) && matches!(then.as_ref(), Expr::Var(n) if n == name) =>
        {
            true
        }
        Expr::Let { then, .. } => names_result(then),
        _ => false,
    }
}

struct Cx {
    /// Every name in the function and the crate's item names, so a `_x`
    /// made here meets none of them.
    taken: HashSet<String>,
}

impl Cx {
    /// An unread parameter, renamed with the `_` that TS exempts from its
    /// unused checks.
    fn param_name(&mut self, n: Name, used: bool) -> Name {
        if used || n.as_str().starts_with('_') {
            return n;
        }
        let base = format!("_{}", n.as_str());
        let mut fresh = base.clone();
        let mut k = 1;
        while self.taken.contains(&fresh) {
            fresh = format!("{base}${k}");
            k += 1;
        }
        self.taken.insert(fresh.clone());
        Name::new(fresh)
    }

    fn expr(&mut self, expr: Expr) -> Expr {
        // `match { let $t = v; e } { .. }` is `let $t = v; match e { .. }`:
        // `v` runs first either way, and a made name meets no arm's binding.
        // Taken where the binding is the `$result` of `ok_or`'s `match` (or
        // what leads to it), whose annotation is the only place TS learns the
        // error type; a binding the tail collapse below would print as
        // nothing keeps that.
        let expr = match expr {
            Expr::Match { scrutinee, arms } => match *scrutinee {
                Expr::Let { name, mutable: false, ty, value, then }
                    if name.as_str().starts_with('$')
                        && names_result(&Expr::Let {
                            name: name.clone(),
                            mutable: false,
                            ty: ty.clone(),
                            value: value.clone(),
                            then: then.clone(),
                        }) =>
                {
                    return self.expr(Expr::Let {
                        name,
                        mutable: false,
                        ty,
                        value,
                        then: Box::new(Expr::Match { scrutinee: then, arms }),
                    });
                }
                scrutinee => Expr::Match { scrutinee: Box::new(scrutinee), arms },
            },
            other => other,
        };
        let mut expr = match expr {
            // `collect::<T>()` and `sum::<T>()` are a typed `let $name = e; $name`
            // so the turbofish is the `want` during typing. The name is used once,
            // as that tail, so the binding prints as nothing. (As a scrutinee it
            // went before the `match` above, keeping its annotation.)
            Expr::Let { name, mutable: false, value, then, .. }
                if name.as_str().starts_with('$') && matches!(then.as_ref(), Expr::Var(n) if n == &name) =>
            {
                return self.expr(*value);
            }
            Expr::Let { name, value, then, .. } if !used_in(&then, name.as_str()) => {
                let value = self.expr(effects(*value));
                let then = self.expr(strip_assigns(*then, &name));
                return if is_pure(&value) { then } else { Expr::Seq { first: Box::new(value), then: Box::new(then) } };
            }
            Expr::Match { scrutinee, arms } => Expr::Match {
                scrutinee,
                arms: arms
                    .into_iter()
                    .map(|a| {
                        let used = |n: &str| a.guard.as_ref().is_some_and(|g| used_in(g, n)) || used_in(&a.body, n);
                        Arm { pattern: drop_pattern(a.pattern, used), guard: a.guard, body: a.body }
                    })
                    .collect(),
            },
            // A loop whose body has nothing left to do (its appends to an
            // unread `String` gone) is its source's evaluation: oxfmt writes
            // an empty body on two lines, which oxlint refuses.
            Expr::ForEach { source, body, over, .. } if empty(&body) => return self.expr(drained(*source, over)),
            Expr::For { start, end, body, .. } if empty(&body) => {
                let (start, end) = (effects(*start), effects(*end));
                return self.expr(Expr::Seq { first: Box::new(start), then: Box::new(end) });
            }
            Expr::ForEach { var, over, source, body } => {
                let used = used_in(&body, var.as_str());
                // `_` itself: TS exempts it, and oxlint's `no-underscore-dangle`
                // allows it where it refuses `_x` on a local. Each loop is
                // its own block, so nested ones may all take it.
                Expr::ForEach { var: if used { var } else { Name::new("_") }, over, source, body }
            }
            Expr::Closure { params, ret, body } => Expr::Closure {
                params: params
                    .into_iter()
                    .map(|p| {
                        let used = used_in(&body, p.name.as_str());
                        ClosureParam { name: self.param_name(p.name, used), ty: p.ty }
                    })
                    .collect(),
                ret,
                body,
            },
            other => other,
        };
        for child in expr.children_mut() {
            let owned = std::mem::replace(child, Expr::Unreachable);
            *child = self.expr(owned);
        }
        // Again once the body is walked: what emptied it (an unread `let`)
        // is only dropped there.
        match expr {
            // A statement runs for its effects alone (an `if` whose sides
            // are now empty is its test).
            Expr::Seq { first, then } => match effects(*first) {
                first if is_unit(&first) => *then,
                first => Expr::Seq { first: Box::new(first), then },
            },
            Expr::ForEach { source, body, over, .. } if empty(&body) => drained(*source, over),
            Expr::For { start, end, body, .. } if empty(&body) => {
                Expr::Seq { first: Box::new(effects(*start)), then: Box::new(effects(*end)) }
            }
            expr => expr,
        }
    }
}

fn drop_pattern(mut p: Pattern, used: impl std::ops::Fn(&str) -> bool + Copy) -> Pattern {
    fn go(p: &mut Pattern, used: &dyn std::ops::Fn(&str) -> bool) {
        match p {
            Pattern::Var(n) if !used(n.as_str()) => *p = Pattern::Wildcard,
            p => p.children_mut().into_iter().for_each(|p| go(p, used)),
        }
    }
    go(&mut p, &used);
    p
}

/// Whether `name` is read in `expr`, stopping at a nested binder of that name.
fn used_in(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Var(n) => n.as_str() == name,
        Expr::Call { callee: Callee::Local(n), args } => n.as_str() == name || args.iter().any(|a| used_in(a, name)),
        // `s = s + piece` (`s.push(..)`) writes `s`; only the piece reads.
        // Otherwise what a write of `name` reads of `name` is read only
        // where the write is kept for its effects (`b = c && b` is not).
        Expr::Assign { name: n, value } if n.as_str() == name => match appended(value, n) {
            Some(piece) => used_in(piece, name),
            None => used_in(&effects((**value).clone()), name),
        },
        Expr::Assign { value, .. } => used_in(value, name),
        Expr::Let { name: n, value, then, .. } => used_in(value, name) || (n.as_str() != name && used_in(then, name)),
        Expr::Match { scrutinee, arms } => used_in(scrutinee, name) || arms.iter().any(|a| arm_uses(a, name)),
        Expr::For { var, start, end, body, .. } => {
            used_in(start, name) || used_in(end, name) || (var.as_str() != name && used_in(body, name))
        }
        Expr::ForEach { var, source, body, .. } => {
            used_in(source, name) || (var.as_str() != name && used_in(body, name))
        }
        Expr::Closure { params, body, .. } => !params.iter().any(|p| p.name.as_str() == name) && used_in(body, name),
        _ => expr.children().iter().any(|c| used_in(c, name)),
    }
}

fn arm_uses(a: &Arm, name: &str) -> bool {
    if a.pattern.bindings().iter().any(|b| b.as_str() == name) {
        return false;
    }
    a.guard.as_ref().is_some_and(|g| used_in(g, name)) || used_in(&a.body, name)
}

/// Writes to `name` after its unused `let` was dropped: keep a non-pure
/// value as a statement, drop the rest.
fn strip_assigns(expr: Expr, name: &Name) -> Expr {
    match expr {
        Expr::Assign { name: n, value } if n == *name => effects(appended(&value, &n).cloned().unwrap_or(*value)),
        other => {
            let mut other = other;
            for child in other.children_mut() {
                let owned = std::mem::replace(child, Expr::Unreachable);
                *child = strip_assigns(owned, name);
            }
            other
        }
    }
}

/// Dropping it changes nothing: no call, arithmetic, or index that could
/// panic or loop.
/// What evaluating `expr` for its effects alone needs: its value is
/// dropped, so a tail that only reads (an arm's `v`, a `match` on a place
/// whose arms only read) goes, and the names bound for it with it. What
/// may panic stays, in order. `()` when nothing is left.
/// What a loop with nothing left in its body does: its source's effects,
/// or, where a stage runs the crate's code on each item (`map`,
/// `filter_map`, `take_while`, ..), every item walked, as `count()` walks
/// them.
fn drained(source: Expr, over: purecrate_ir::Over) -> Expr {
    match source {
        Expr::Call {
            callee:
                Callee::IterMap { .. }
                | Callee::IterFilter { .. }
                | Callee::IterFilterMap { .. }
                | Callee::IterFlatten { .. }
                | Callee::IterCut { .. },
            ..
        } => Expr::Call { callee: Callee::Consume { method: purecrate_ir::Consume::Count, over }, args: vec![source] },
        source => effects(source),
    }
}

fn effects(expr: Expr) -> Expr {
    let unit = || Expr::Lit(purecrate_ir::Lit::Unit);
    match expr {
        e if inert(&e) => unit(),
        Expr::Match { scrutinee, arms } => {
            let arms: Vec<Arm> = arms.into_iter().map(|a| Arm { body: effects(a.body), ..a }).collect();
            if arms.iter().all(|a| is_unit(&a.body) && a.guard.as_ref().is_none_or(is_pure)) {
                effects(*scrutinee)
            } else {
                Expr::Match { scrutinee, arms }
            }
        }
        Expr::If { cond, then, else_ } => {
            let (then, else_) = (effects(*then), effects(*else_));
            if is_unit(&then) && is_unit(&else_) {
                effects(*cond)
            } else {
                Expr::If { cond, then: Box::new(then), else_: Box::new(else_) }
            }
        }
        Expr::Let { name, mutable, ty, value, then } => {
            let then = effects(*then);
            if used_in(&then, name.as_str()) {
                Expr::Let { name, mutable, ty, value, then: Box::new(then) }
            } else {
                seq(effects(*value), strip_assigns(then, &name))
            }
        }
        // A `loop` whose value is dropped: each `break v` is `v`'s effects.
        Expr::While { cond, mut body } if *cond == Expr::Lit(purecrate_ir::Lit::Bool(true)) => {
            body.map_breaks(&mut |v| seq(effects(v), Expr::Break(None)));
            Expr::While { cond, body }
        }
        Expr::Seq { first, then } => {
            let then = effects(*then);
            if is_unit(&then) {
                *first
            } else {
                Expr::Seq { first, then: Box::new(then) }
            }
        }
        // A test or a sum that cannot panic leaves its operands' effects,
        // in order: `x.checked_sub(y).is_some()` as a statement is nothing,
        // and TS refuses a comparison standing as one. What is still a
        // `Binary` here cannot panic: integer and `f32` arithmetic is a
        // `Callee::Int` / `Fround` call by now, so this is `f64`'s, or a
        // comparison.
        Expr::Binary { op, left, right } if !matches!(op, BinOp::And | BinOp::Or) => {
            seq(effects(*left), effects(*right))
        }
        Expr::Unary { op: UnOp::Neg, expr } => effects(*expr),
        // A tuple or an array built only to be dropped: its elements, in order.
        Expr::Tuple(items) | Expr::Array(items) => items.into_iter().map(effects).fold(unit(), seq),
        Expr::Field { base, .. } => effects(*base),
        Expr::Call { callee: Callee::AsFloat(_) | Callee::Fround, args } => {
            args.into_iter().map(effects).fold(unit(), seq)
        }
        // The right side runs only sometimes: with nothing to run, the left
        // side is all; otherwise an `if`, which TS takes as a statement.
        Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } => {
            let right = effects(*right);
            if is_unit(&right) {
                effects(*left)
            } else {
                let cond = match (op, *left) {
                    (BinOp::And, left) => left,
                    (_, Expr::Unary { op: UnOp::Not, expr }) => *expr,
                    (_, left) => Expr::Unary { op: UnOp::Not, expr: Box::new(left) },
                };
                Expr::If { cond: Box::new(cond), then: Box::new(right), else_: Box::new(unit()) }
            }
        }
        Expr::Unary { op: UnOp::Not, expr } => effects(*expr),
        Expr::Call { callee, args } if cannot_panic(&callee) => args.into_iter().map(effects).fold(unit(), seq),
        Expr::At { at, expr } => {
            let inner = effects(*expr);
            if is_unit(&inner) {
                inner
            } else {
                Expr::At { at, expr: Box::new(inner) }
            }
        }
        other => other,
    }
}

/// `first; then`, leaving out either side that is `()`.
fn seq(first: Expr, then: Expr) -> Expr {
    match (is_unit(&first), is_unit(&then)) {
        (true, _) => then,
        (_, true) => first,
        _ => Expr::Seq { first: Box::new(first), then: Box::new(then) },
    }
}

/// A call that panics on no arguments: `Some`, `Ok`, `Err`, the tests on
/// `Option`, and the integer methods that report or wrap their overflow.
fn cannot_panic(callee: &Callee) -> bool {
    use purecrate_ir::IntMethod as M;
    match callee {
        Callee::OptionSome | Callee::ResultOk | Callee::ResultErr | Callee::OptionIsSome | Callee::OptionIsNone => true,
        // Conversions to a float, which saturate or round.
        Callee::IntToFloat { .. } | Callee::FloatToFloat { .. } | Callee::FloatConst { .. } | Callee::Float { .. } => {
            true
        }
        // Widening, except into `usize` from what may pass 2^53.
        Callee::IntFrom { from, to } => {
            *to != purecrate_ir::IntTy::Usize || from.is_some_and(|f| !f.is_big() && f != purecrate_ir::IntTy::Usize)
        }
        Callee::Int { op, .. } => match op {
            IntOp::And | IntOp::Or | IntOp::Xor | IntOp::Not => true,
            IntOp::Method(m) => matches!(
                m,
                M::Min
                    | M::Max
                    | M::CheckedAdd
                    | M::CheckedSub
                    | M::CheckedMul
                    | M::CheckedDiv
                    | M::CheckedRem
                    | M::CheckedNeg
                    | M::CheckedPow
                    | M::SaturatingAdd
                    | M::SaturatingSub
                    | M::SaturatingMul
                    | M::SaturatingPow
                    | M::WrappingAdd
                    | M::WrappingSub
                    | M::WrappingMul
                    | M::WrappingNeg
                    | M::WrappingPow
            ),
            _ => false,
        },
        _ => false,
    }
}

/// A read, or a value built of reads: `Some(v)`, `Ok(v)`, `None`.
fn inert(expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee: Callee::OptionSome | Callee::OptionNone | Callee::ResultOk | Callee::ResultErr, args } => {
            args.iter().all(inert)
        }
        e => is_pure(e),
    }
}

fn is_unit(expr: &Expr) -> bool {
    matches!(expr, Expr::Lit(purecrate_ir::Lit::Unit))
}

fn is_pure(expr: &Expr) -> bool {
    match expr {
        Expr::Var(_) | Expr::Lit(_) | Expr::Closure { .. } => true,
        Expr::Field { base, .. } => is_pure(base),
        // Conversions that cannot panic (into `usize` they may, past 2^53).
        Expr::Call { callee, args } if args.iter().all(is_pure) => match callee {
            Callee::IntToFloat { .. }
            | Callee::FloatToFloat { .. }
            | Callee::FloatConst { .. }
            | Callee::Float { .. }
            | Callee::AsFloat(_)
            | Callee::Fround
            | Callee::StringNew
            | Callee::VecGet
            | Callee::VecLast
            | Callee::VecContains
            | Callee::VecRev
            | Callee::VecZip
            // Made only where the length holds them.
            | Callee::SliceElem { .. }
            | Callee::SliceRest { .. }
            | Callee::Tree(
                purecrate_ir::TreeOp::NewMap
                | purecrate_ir::TreeOp::NewSet
                | purecrate_ir::TreeOp::Get
                | purecrate_ir::TreeOp::Has
                | purecrate_ir::TreeOp::Len
                | purecrate_ir::TreeOp::IsEmpty
                | purecrate_ir::TreeOp::Entries
                | purecrate_ir::TreeOp::Keys
                | purecrate_ir::TreeOp::Values
                | purecrate_ir::TreeOp::Lookup
                | purecrate_ir::TreeOp::EntriesBy
                | purecrate_ir::TreeOp::KeysBy
                | purecrate_ir::TreeOp::ValuesBy,
            )
            // No `str` method on the allow-list panics; slicing does, and is
            // `Callee::Slice`, not here.
            | Callee::Str(_)
            | Callee::StrSplit
            | Callee::StrSplitStr
            // Padding never panics.
            | Callee::Format(_)
            | Callee::StringFrom => true,
            Callee::IntCast { to, .. } | Callee::FloatToInt { to, .. } => *to != purecrate_ir::IntTy::Usize,
            Callee::IntFrom { to, .. } => *to != purecrate_ir::IntTy::Usize,
            // A copy of an array (`v.clone()`, a `Vec` bound to a grown local).
            Callee::Collect { result: false, over: purecrate_ir::Over::Items } => args.len() == 1,
            _ => false,
        },
        _ => false,
    }
}

/// The piece `s = s + piece` appends, as `check` writes `s.push(..)`.
fn appended<'e>(value: &'e Expr, name: &Name) -> Option<&'e Expr> {
    match value {
        Expr::Call { callee: Callee::StrConcat, args } if matches!(&args[0], Expr::Var(n) if n == name) => {
            Some(&args[1])
        }
        _ => None,
    }
}

/// A loop body that does nothing: `()`, a comment, or a run of them.
fn empty(body: &Expr) -> bool {
    match body {
        Expr::Lit(purecrate_ir::Lit::Unit) | Expr::Comment(_) => true,
        Expr::Seq { first, then } => empty(first) && empty(then),
        _ => false,
    }
}
