//! Narrowing as TS's control-flow analysis does it, so the printer folds
//! exactly the tests TS has already decided (TS refuses them: "no overlap",
//! a property of `never`).
//!
//! A [`State`] holds, for each place (a name or a field chain), the cases
//! it may still hold: `Option`'s and `Result`'s, an enum's variants, or a
//! `bool`'s value. It flows forward through the body. A test refines it on
//! each side; an arm of a `match` holds the cases its pattern admits less
//! those an earlier unguarded arm took; where control flow joins (after an
//! `if` or a `match`, at a loop's head, past a loop) the states are joined
//! case by case; a write forgets the place; `return`, `break`, and
//! `continue` leave the state to the function or the loop. A loop's head is
//! found by running its body until the state there stops changing. A
//! closure keeps what is known of names nothing ever writes, as TS keeps a
//! parameter's narrowing in an arrow made after it.
//!
//! The decisions themselves (`taken_arm`, `failed_try`, `decide`, ..) are
//! the parent module's: only how the state is found is here.

use std::collections::HashSet;

use super::*;

/// What is known at a point: each place once, with its cases sorted.
#[derive(Clone, Default, PartialEq, Debug)]
pub(super) struct State(pub(super) Known);

impl State {
    fn get(&self, p: &Expr) -> Option<Vec<Name>> {
        let own = |p: &Expr| self.0.iter().find(|(k, _)| k == p).map(|(_, v)| v.clone());
        own(p).or_else(|| aliased(p).and_then(|t| own(&t)))
    }

    fn set(&mut self, p: Expr, mut cases: Vec<Name>) {
        cases.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        cases.dedup();
        self.0.retain(|(k, _)| *k != p);
        self.0.push((p, cases));
    }

    /// Sets `p` and, where `p` is a made name standing for a place, that
    /// place: the printer reads one as the other.
    fn set_place(&mut self, p: &Expr, cases: Vec<Name>) {
        if let Some(t) = aliased(p) {
            self.set(t, cases.clone());
        }
        self.set(p.clone(), cases);
    }

    /// Forgets every place under `name` (a write, or a new binding of it).
    fn forget(&mut self, name: &Name) {
        self.0.retain(|(k, _)| root(k) != name);
    }

    fn join(&self, other: &State) -> State {
        let mut out = State::default();
        for (p, a) in &self.0 {
            if let Some((_, b)) = other.0.iter().find(|(k, _)| k == p) {
                let mut cases = a.clone();
                cases.extend(b.iter().cloned());
                out.set(p.clone(), cases);
            }
        }
        out
    }
}

/// The states that reach one point joined; `None` where none does.
fn join_all(states: impl IntoIterator<Item = State>) -> Option<State> {
    states.into_iter().reduce(|a, b| a.join(&b))
}

#[derive(Default)]
struct Jumps {
    breaks: Vec<State>,
    continues: Vec<State>,
}

struct Cx {
    loops: Vec<Jumps>,
    /// Names something in the function writes (`let mut`, assignment):
    /// a closure drops what is known of them.
    written: HashSet<String>,
}

/// How many times a loop's body runs to find the state at its head; the
/// lattice is finite, and this bounds the work of nested loops.
const ROUNDS: usize = 6;

/// `body` with every test TS has decided folded, run until nothing changes.
pub(super) fn flow_fn(body: &mut Expr) {
    let mut written = HashSet::new();
    collect_written(body, &mut written);
    for _ in 0..4 {
        let before = body.clone();
        let mut cx = Cx { loops: Vec::new(), written: written.clone() };
        flow(body, State::default(), &mut cx);
        if *body == before {
            break;
        }
    }
}

fn collect_written(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Assign { name, .. } | Expr::Let { name, mutable: true, .. } => {
            out.insert(name.as_str().to_string());
        }
        _ => {}
    }
    for c in expr.children() {
        collect_written(c, out);
    }
}

/// The cases each side of `cond` leaves the places it tests in; `None` for
/// a side no value takes (`true` has no `else`).
fn refine(cond: &Expr, st: &State) -> (Option<State>, Option<State>) {
    if let Expr::Lit(Lit::Bool(b)) = cond {
        return if *b { (Some(st.clone()), None) } else { (None, Some(st.clone())) };
    }
    let side = |holds: bool| {
        let mut s = st.clone();
        for (p, case) in tests(cond, holds) {
            s.set_place(&p, vec![case]);
        }
        s
    };
    (Some(side(true)), Some(side(false)))
}

/// The places `cond` decides where it is `holds`: a `bool` place, an
/// `Option` place's `is_some()` / `is_none()`, through `!`, `&&` (holding),
/// and `||` (failing).
fn tests(cond: &Expr, holds: bool) -> Vec<(Expr, Name)> {
    use purecrate_ir::Callee;
    match cond {
        c if is_place(c) => vec![(c.clone(), Name::new(if holds { TRUE } else { FALSE }))],
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => tests(expr, !holds),
        Expr::Binary { op: BinOp::And, left, right } if holds => {
            let mut out = tests(left, true);
            out.extend(tests(right, true));
            out
        }
        Expr::Binary { op: BinOp::Or, left, right } if !holds => {
            let mut out = tests(left, false);
            out.extend(tests(right, false));
            out
        }
        Expr::Call { callee: callee @ (Callee::OptionIsSome | Callee::OptionIsNone), args } => match args.as_slice() {
            [p] if is_place(p) => {
                let some = holds == (*callee == Callee::OptionIsSome);
                vec![(p.clone(), Name::new(if some { SOME } else { NONE }))]
            }
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// The cases a place known to hold `known` (if anything is known) may still
/// hold once `taken` are excluded; `None` where that is not known.
fn remaining(known: Option<&Vec<Name>>, taken: &[Name]) -> Option<Vec<Name>> {
    let all: Vec<Name> = match known {
        Some(k) => k.clone(),
        None => {
            let pair = |a: &str, b: &str| taken.iter().any(|t| t.as_str() == a || t.as_str() == b);
            if pair(SOME, NONE) {
                vec![Name::new(SOME), Name::new(NONE)]
            } else if pair(OK, ERR) {
                vec![Name::new(OK), Name::new(ERR)]
            } else if pair(TRUE, FALSE) {
                vec![Name::new(TRUE), Name::new(FALSE)]
            } else {
                return None;
            }
        }
    };
    let left: Vec<Name> = all.into_iter().filter(|c| !taken.contains(c)).collect();
    (!left.is_empty()).then_some(left)
}

/// The constructor a value is, as TS narrows a binding to it: `null` is
/// `None`, `Some(v)` prints as `v`, which is never `null`. (`Result.ok(v)`
/// has the whole union's type, so it decides nothing.)
fn built_option(value: &Expr) -> Option<Name> {
    match constructed_case(value)?.as_str() {
        c @ (SOME | NONE) => Some(Name::new(c)),
        _ => None,
    }
}

/// Runs `expr` from `st`, folding what is decided there; the state after
/// it, or `None` where it always leaves.
fn flow(expr: &mut Expr, st: State, cx: &mut Cx) -> Option<State> {
    // A test reads a `bool` place an enclosing test has decided as that
    // value, as TS does: its control flow then matches TS's.
    if let Expr::If { cond, .. } | Expr::While { cond, .. } = expr {
        decide(cond, &st.0);
    }
    // A test whose value narrowing decides though something in it must
    // still run (`100 / a > 0 && c` with `c` known `false`): that part runs
    // as a statement, then the side taken. TS would refuse the comparison.
    if let Expr::If { cond, then, else_ } = expr {
        if !matches!(**cond, Expr::Lit(_)) {
            if let Some((before, b)) = known_test(cond, &st) {
                let side =
                    Expr::If { cond: Box::new(Expr::Lit(Lit::Bool(b))), then: then.clone(), else_: else_.clone() };
                *expr = sequence(before, side);
                return flow(expr, st, cx);
            }
        }
    }
    // Decided with nothing to run first, it is the side taken, so what reads
    // it (`match { let t = if c { .. } else { r }; t }`) sees that side.
    if let Expr::If { cond, then, else_ } = expr {
        if let Some((Expr::Lit(Lit::Unit), b)) = constant(cond) {
            let side = if b { &**then } else { &**else_ };
            if !declares(side) {
                *expr = side.clone();
                same_sides(expr);
                return flow(expr, st, cx);
            }
        }
    }
    if let Some(taken) = taken_arm(expr, &st.0) {
        *expr = taken;
        return flow(expr, st, cx);
    }
    // `let x = r?` where `r` is known `Err` always leaves; known `Ok`, it
    // is the payload. So is `Ok(e)?` (what a fold may leave), and `Err(e)?`
    // always leaves.
    if let Expr::Let { value, .. } = expr {
        if let Expr::Try { expr: place, on: Some(on) } = &mut **value {
            if let Some(exit) = failed_try(place, *on, &st.0).filter(|_| is_place(place)) {
                *expr = exit;
                return flow(expr, st, cx);
            }
            if let Some(payload) = known_payload(place, *on, &st) {
                **value = payload;
                return flow(expr, st, cx);
            }
        }
    }
    // `r?` where `r` is known `Ok` never leaves; known `Err`, always does.
    if let Expr::Seq { first, then } = expr {
        if let Expr::Try { expr: place, on: Some(on) } = &**first {
            if known_payload(place, *on, &st).is_some() {
                *expr = (**then).clone();
                return flow(expr, st, cx);
            }
            if let Some(exit) = failed_try(place, *on, &st.0).filter(|_| is_place(place)) {
                *expr = exit;
                return flow(expr, st, cx);
            }
        }
    }
    match expr {
        Expr::Match { scrutinee, .. } if is_place(scrutinee) => flow_match(expr, st, cx),
        Expr::Match { scrutinee, arms } => {
            let s = flow(scrutinee, st, cx)?;
            // A scrutinee narrowing has made a place (`{ let t = if false {
            // .. } else { r }; t }` is `r`) is decided as the place.
            same_sides(scrutinee);
            if is_place(scrutinee) {
                return flow_match(expr, s, cx);
            }
            let outs: Vec<State> = arms
                .iter_mut()
                .filter_map(|arm| {
                    let mut a = s.clone();
                    for n in arm.pattern.bindings() {
                        a.forget(n);
                    }
                    if let Some(g) = &mut arm.guard {
                        a = flow(g, a, cx)?;
                    }
                    flow(&mut arm.body, a, cx)
                })
                .collect();
            if let Some(taken) = constructed_arm(expr) {
                *expr = taken;
            }
            join_all(outs)
        }
        Expr::Seq { first, then } => {
            let mut s = flow(first, st.clone(), cx)?;
            // `p?;` as a statement tests `p` itself (a `let` of `p?` tests a
            // copy, `xOption`, which TS narrows instead).
            if let Expr::Try { expr: inner, on: Some(on) } = &mut **first {
                // Narrowing made what `?` takes a place (`{ let t = if c { r }
                // else { .. }; t }?`): decided as the place, from the start.
                same_sides(inner);
                if known_payload(inner, *on, &st).is_some() || failed_try(inner, *on, &st.0).is_some() {
                    return flow(expr, st, cx);
                }
            }
            if let Expr::Try { expr: place, on: Some(on) } = &**first {
                if is_place(place) {
                    s.set_place(place, vec![tried_case(*on)]);
                }
            }
            flow(then, s, cx)
        }
        Expr::Let { name, mutable: false, value, then, .. } if is_place(value) && !touches(then, root(value)) => {
            let name = name.clone();
            let mut s = st;
            let known = s.get(value);
            s.forget(&name);
            if let Some(cases) = known {
                s.set(Expr::Var(name.clone()), cases);
            }
            // A made name for a place is printed as the place: one is the
            // other, so a test on either decides both.
            let made = name.as_str().starts_with('$');
            if made {
                ALIASES.with(|a| a.borrow_mut().push((name.clone(), (**value).clone())));
            }
            let after = flow(then, s, cx);
            if made {
                ALIASES.with(|a| a.borrow_mut().pop());
            }
            // Reading a place does nothing: a binding narrowing left unread
            // (`unwrap_or(a)`'s `$optOr = a` past `o?`) goes.
            if !mentions(then, &name) {
                *expr = (**then).clone();
            }
            after.map(|mut a| {
                a.forget(&name);
                a
            })
        }
        Expr::Let { mutable, value, then, .. } => {
            let mutable = *mutable;
            let mut s = flow(value, st.clone(), cx)?;
            // Narrowing made the value a place: a `let` of a place, as above.
            same_sides(value);
            if !mutable && is_place(value) && !touches(then, root(value)) {
                return flow(expr, st, cx);
            }
            // ..or made `p?` of a place whose case is known.
            if let Expr::Try { expr: place, on: Some(on) } = &**value {
                if known_payload(place, *on, &st).is_some() || failed_try(place, *on, &st.0).is_some() {
                    return flow(expr, st, cx);
                }
            }
            let Expr::Let { name, value, then, .. } = expr else { unreachable!("matched") };
            let name = name.clone();
            if let Some(e) = constructed_arm(value) {
                **value = e;
            }
            s.forget(&name);
            if let Some(case) = built_option(value) {
                s.set(Expr::Var(name.clone()), vec![case]);
            }
            // TS does not narrow `o` from `Result.ok(a)`, whose type is the
            // whole union: each `match o` is decided on the value itself,
            // its payload read again where the `match` was (only a name
            // nothing reassigns, or a literal).
            if !mutable {
                let built = as_constructor(value);
                let steady = built.as_ref().is_some_and(|b| {
                    b.children().iter().all(|c| match c {
                        Expr::Lit(_) => true,
                        c if is_place(c) => !touches(then, root(c)),
                        _ => false,
                    })
                });
                if let (Some(built), true, false) = (built, steady, touches(then, &name)) {
                    decide_on(then, &name, &built);
                }
            }
            let after = flow(then, s, cx);
            // A binding narrowing left unread (`unwrap_or(d)`'s eager `d`
            // once its `None` arm is gone) still runs, as a statement; TS
            // refuses the unread `const`.
            if !mutable && !mentions(then, &name) {
                *expr = if effectless(value) {
                    (**then).clone()
                } else {
                    Expr::Seq { first: value.clone(), then: then.clone() }
                };
            }
            after.map(|mut a| {
                a.forget(&name);
                a
            })
        }
        Expr::If { cond, then, else_ } => {
            let s = flow(cond, st, cx)?;
            let (yes, no) = refine(cond, &s);
            let a = yes.and_then(|y| flow(then, y, cx));
            let b = no.and_then(|n| flow(else_, n, cx));
            join_all(a.into_iter().chain(b))
        }
        Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } => {
            let and = *op == BinOp::And;
            let s = flow(left, st, cx)?;
            let (yes, no) = refine(left, &s);
            let (go, stop) = if and { (yes, no) } else { (no, yes) };
            let after_right = go.and_then(|g| flow(right, g, cx));
            join_all(after_right.into_iter().chain(stop))
        }
        Expr::While { cond, body } => {
            let head = loop_head(st, cx, |cx, head| {
                let (mut c, mut b) = ((**cond).clone(), (**body).clone());
                let sc = flow(&mut c, head, cx)?;
                let (yes, _) = refine(&c, &sc);
                flow(&mut b, yes?, cx)
            });
            cx.loops.push(Jumps::default());
            let sc = flow(cond, head, cx);
            if let Some(yes) = sc.as_ref().and_then(|sc| refine(cond, sc).0) {
                flow(body, yes, cx);
            }
            let jumps = cx.loops.pop().expect("pushed");
            let exit = sc.and_then(|sc| refine(cond, &sc).1);
            join_all(jumps.breaks.into_iter().chain(exit))
        }
        Expr::For { var, start, end, body, .. } => {
            let s = flow(start, st, cx)?;
            let mut s = flow(end, s, cx)?;
            s.forget(var);
            let var = var.clone();
            let head = loop_head(s, cx, |cx, head| flow(&mut (**body).clone(), head, cx));
            cx.loops.push(Jumps::default());
            flow(body, head.clone(), cx);
            let jumps = cx.loops.pop().expect("pushed");
            join_all(jumps.breaks.into_iter().chain(Some(head))).map(|mut a| {
                a.forget(&var);
                a
            })
        }
        Expr::ForEach { var, source, body, .. } => {
            let mut s = flow(source, st, cx)?;
            s.forget(var);
            let var = var.clone();
            let head = loop_head(s, cx, |cx, head| flow(&mut (**body).clone(), head, cx));
            cx.loops.push(Jumps::default());
            flow(body, head.clone(), cx);
            let jumps = cx.loops.pop().expect("pushed");
            join_all(jumps.breaks.into_iter().chain(Some(head))).map(|mut a| {
                a.forget(&var);
                a
            })
        }
        Expr::Assign { name, value } => {
            let mut s = flow(value, st, cx)?;
            s.forget(name);
            if let Some(case) = built_option(value) {
                s.set(Expr::Var(name.clone()), vec![case]);
            }
            Some(s)
        }
        Expr::Return(value) => {
            flow(value, st, cx);
            None
        }
        Expr::Break => {
            if let Some(j) = cx.loops.last_mut() {
                j.breaks.push(st);
            }
            None
        }
        Expr::Continue => {
            if let Some(j) = cx.loops.last_mut() {
                j.continues.push(st);
            }
            None
        }
        // A closure runs later: it keeps what is known of names nothing
        // writes (TS keeps a parameter's narrowing in an arrow), and its
        // own parameters are new names.
        Expr::Closure { params, body, .. } => {
            let mut inside = st.clone();
            inside.0.retain(|(p, _)| !cx.written.contains(root(p).as_str()));
            for p in params.iter() {
                inside.forget(&p.name);
            }
            let outer = std::mem::take(&mut cx.loops);
            flow(body, inside, cx);
            cx.loops = outer;
            Some(st)
        }
        _ => {
            let mut s = st;
            for child in expr.children_mut() {
                s = flow(child, s, cx)?;
            }
            // A scrutinee narrowing made a constructor (`r.ok()` in `r`'s
            // `Err` arm is `None`) decides its `match` too.
            if let Some(taken) = constructed_arm(expr) {
                *expr = taken;
            }
            // `a == b` where narrowing has decided both `bool`s.
            if let Expr::Binary { op: op @ (BinOp::Eq | BinOp::Ne), left, right } = expr {
                if let (Some(l), Some(r)) = (known_bool(left, &s.0), known_bool(right, &s.0)) {
                    *expr = Expr::Lit(Lit::Bool((l == r) == (*op == BinOp::Eq)));
                }
            }
            literal_compare(expr);
            // `is_some()` of a place whose case is known.
            if let Expr::Call {
                callee: callee @ (purecrate_ir::Callee::OptionIsSome | purecrate_ir::Callee::OptionIsNone),
                args,
            } = expr
            {
                if let [p] = args.as_slice() {
                    if let Some(c) = st_case(&s, p) {
                        let some = c.as_str() == SOME;
                        *expr = Expr::Lit(Lit::Bool(some == (*callee == purecrate_ir::Callee::OptionIsSome)));
                    }
                }
            }
            // `is_some()` of what narrowing made `None` or `Some(v)`.
            if let Expr::Call {
                callee: callee @ (purecrate_ir::Callee::OptionIsSome | purecrate_ir::Callee::OptionIsNone),
                args,
            } = expr
            {
                if let [arg] = args.as_slice() {
                    let pure = arg.children().iter().all(|c| matches!(c, Expr::Var(_) | Expr::Lit(_)));
                    let some = match constructed_case(arg).as_ref().map(Name::as_str) {
                        Some(SOME) => Some(true),
                        Some(NONE) => Some(false),
                        _ => None,
                    };
                    if let (Some(some), true) = (some, pure) {
                        *expr = Expr::Lit(Lit::Bool(some == (*callee == purecrate_ir::Callee::OptionIsSome)));
                    }
                }
            }
            Some(s)
        }
    }
}

/// The value of a test narrowing has decided, with what in it must still
/// run first (`()` if nothing): `&&` and `||` run their left side and stop
/// where it decides; a side that decides alone is reached only if it does
/// nothing, so the other runs on its own. Comparisons of two such values
/// run both.
fn known_test(cond: &Expr, st: &State) -> Option<(Expr, bool)> {
    let unit = Expr::Lit(Lit::Unit);
    if let Some(b) = known_bool(cond, &st.0) {
        return Some((if effectless(cond) { unit } else { return None }, b));
    }
    match cond {
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => known_test(expr, st).map(|(e, b)| (e, !b)),
        Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } => {
            let decides = *op == BinOp::Or;
            match known_test(left, st) {
                Some((e, l)) if l == decides => Some((e, l)),
                Some((e, _)) => {
                    let (r, b) = known_test(right, st)?;
                    Some((sequence(e, r), b))
                }
                None => match known_test(right, st) {
                    Some((r, b)) if b == decides && r == unit => Some((effects(left.as_ref().clone()), b)),
                    _ => None,
                },
            }
        }
        Expr::Binary { op: op @ (BinOp::Eq | BinOp::Ne), left, right } => {
            let (l, a) = known_test(left, st)?;
            let (r, b) = known_test(right, st)?;
            Some((sequence(l, r), (a == b) == (*op == BinOp::Eq)))
        }
        _ => None,
    }
}

/// The one case a place is known to hold.
fn st_case(st: &State, p: &Expr) -> Option<Name> {
    if !is_place(p) {
        return None;
    }
    match st.get(p)?.as_slice() {
        [c] => Some(c.clone()),
        _ => None,
    }
}

/// What `place?` gives where it cannot leave: the place itself known to
/// hold `Some` / `Ok` (its payload read as TS has narrowed it), or `Some(e)`
/// / `Ok(e)` built in place (`e`).
fn known_payload(place: &Expr, on: TryOn, st: &State) -> Option<Expr> {
    use purecrate_ir::Callee;
    if let Expr::Call { callee: Callee::OptionSome | Callee::ResultOk, args } = place {
        let ok = matches!(place, Expr::Call { callee: Callee::OptionSome, .. }) == (on == TryOn::Option);
        return match args.as_slice() {
            [e] if ok => Some(e.clone()),
            _ => None,
        };
    }
    if !is_place(place) || st.get(place).is_none_or(|c| c != [tried_case(on)]) {
        return None;
    }
    Some(match on {
        TryOn::Option => place.clone(),
        TryOn::Result => Expr::Field { base: Box::new(place.clone()), name: Name::new("value") },
    })
}

/// The state at a loop's head: what enters, joined with what the end of
/// the body and each `continue` bring back, until it stops changing.
/// `round` runs one pass on a copy of the body from a head state.
fn loop_head(entry: State, cx: &mut Cx, mut round: impl FnMut(&mut Cx, State) -> Option<State>) -> State {
    let mut head = entry.clone();
    for _ in 0..ROUNDS {
        cx.loops.push(Jumps::default());
        let end = round(cx, head.clone());
        let jumps = cx.loops.pop().expect("pushed");
        let next = join_all(std::iter::once(entry.clone()).chain(end).chain(jumps.continues))
            .expect("the entry reaches the head");
        if next == head {
            break;
        }
        head = next;
    }
    head
}

/// A `match` on a place: each arm runs with the place narrowed to the cases
/// its pattern admits and no earlier unguarded arm took. An arm of `A | B`
/// runs once per variant; where those bodies come out different it is one
/// arm per variant (the printer shares the cases of those that still do the
/// same).
fn flow_match(expr: &mut Expr, st: State, cx: &mut Cx) -> Option<State> {
    let Expr::Match { scrutinee, arms } = expr else { unreachable!("a match") };
    let known = st.get(scrutinee);
    let mut taken: Vec<Name> = Vec::new();
    let mut outs: Vec<State> = Vec::new();
    let mut split = Vec::with_capacity(arms.len());
    for mut arm in std::mem::take(arms) {
        let named = variants_of(&arm.pattern);
        let admitted: Vec<Name> = named.iter().filter(|v| admits(&arm.pattern, v)).cloned().collect();
        let unguarded = arm.guard.is_none();
        let variants: Vec<Name> = named.iter().filter(|v| !taken.contains(v)).cloned().collect();
        let writes = touches(&arm.body, root(scrutinee));
        let mut base = st.clone();
        for n in arm.pattern.bindings() {
            base.forget(n);
        }
        let run = |cx: &mut Cx, body: &mut Expr, guard: &mut Option<Expr>, mut s: State, outs: &mut Vec<State>| {
            if let Some(g) = guard {
                match flow(g, s.clone(), cx) {
                    Some(after) => s = refine(g, &after).0.unwrap_or(after),
                    None => return,
                }
            }
            if let Some(after) = flow(body, s, cx) {
                outs.push(after);
            }
        };
        if named.is_empty() || writes {
            // `_` or a binding: what no earlier arm took, where that is known.
            if !writes {
                if let Some(rest) = remaining(known.as_ref(), &taken) {
                    base.set_place(scrutinee, rest);
                }
            }
            run(cx, &mut arm.body, &mut arm.guard, base, &mut outs);
            split.push(arm);
        } else if !variants.is_empty() {
            let mut bodies = Vec::with_capacity(variants.len());
            for v in &variants {
                let mut body = arm.body.clone();
                let mut guard = arm.guard.clone();
                let mut s = base.clone();
                s.set_place(scrutinee, vec![v.clone()]);
                run(cx, &mut body, &mut guard, s, &mut outs);
                bodies.push((v.clone(), body));
            }
            match &arm.pattern {
                Pattern::Or(alts) if bodies.iter().any(|(_, b)| *b != bodies[0].1) => {
                    for alt in alts {
                        let body = variants_of(alt)
                            .first()
                            .and_then(|v| bodies.iter().find(|(w, _)| w == v))
                            .map_or_else(|| arm.body.clone(), |(_, b)| b.clone());
                        split.push(Arm { pattern: alt.clone(), guard: arm.guard.clone(), body });
                    }
                }
                _ => {
                    arm.body = bodies.into_iter().next().expect("a variant").1;
                    split.push(arm.clone());
                }
            }
        } else {
            // Every case it names an earlier arm took: never reached.
            split.push(arm.clone());
        }
        if unguarded {
            for v in admitted {
                if !taken.contains(&v) {
                    taken.push(v);
                }
            }
        }
    }
    // Narrowing may have folded every read of a name an arm binds; TS
    // refuses the unread `const`.
    for arm in &mut split {
        let read = |n: &Name| mentions(&arm.body, n) || arm.guard.as_ref().is_some_and(|g| mentions(g, n));
        arm.pattern = unbind(std::mem::replace(&mut arm.pattern, Pattern::Wildcard), &read);
    }
    *arms = split;
    join_all(outs)
}
