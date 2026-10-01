//! A `match` on a tuple becomes nested `match`es on its elements, one element
//! at a time (a decision tree). rustc has checked the tuple arms are
//! exhaustive; after this rewrite every enum, `Option`, and `Result` element
//! is matched by a `match` that names every case, so it prints as a `switch`
//! ending in `assertNever` and TS checks exhaustiveness on its own.
//! Integer, `char`, and string elements stay `if` chains ending in `_`, as a
//! `match` on them does.
//!
//! Elements that are places are matched where they are; any other is bound
//! to a `let` first, left to right, as Rust evaluates the tuple. An arm that
//! several cases reach is copied into each; cases that reach the same code
//! and bind nothing share one arm (`A | B`).
//!
//! A `match` with guards comes here too, a single value as a tuple of one:
//! where an arm's pattern has matched, its guard is tested, and if it is
//! false the arms after it that can still match follow in the `else`. Each
//! element is tested once on a path, so TS never narrows a value that a
//! later `switch` would name again.

use purecrate_ir::{tuple_field, Arm, Expr, Name, Pattern, Ty, VariantBind, VariantFields};

use crate::defs::Defs;

/// One tuple arm still in play: the patterns of the elements not yet
/// matched, the names bound so far, and the body.
#[derive(Clone)]
struct Row {
    pats: Vec<Pattern>,
    binds: Vec<(Name, Ty, Expr)>,
    guard: Option<Expr>,
    body: Expr,
}

struct Lowering<'d, 'a, 'n> {
    defs: &'d Defs<'a>,
    fresh: &'n mut usize,
}

/// `scrutinee` has type `(tys..)`, normalized; `arms` are typed tuple arms,
/// `|` of tuples, or a last `_`.
pub fn lower(defs: &Defs, scrutinee: Expr, tys: Vec<Ty>, arms: Vec<Arm>, fresh: &mut usize) -> Expr {
    let width = tys.len();
    let mut rows = Vec::new();
    for arm in arms {
        let alts = match arm.pattern {
            Pattern::Tuple(ps) => vec![ps],
            Pattern::Or(alts) => alts
                .into_iter()
                .map(|a| match a {
                    Pattern::Tuple(ps) => ps,
                    other => unreachable!("exhaustive: `|` of tuples only, found {other:?}"),
                })
                .collect(),
            Pattern::Wildcard => vec![vec![Pattern::Wildcard; width]],
            other => unreachable!("exhaustive: tuple arms only, found {other:?}"),
        };
        for pats in alts {
            rows.push(Row { pats, binds: Vec::new(), guard: arm.guard.clone(), body: arm.body.clone() });
        }
    }
    let mut lw = Lowering { defs, fresh };
    // Places are read where they are; anything else once, in order.
    let (subjects, lets): (Vec<Expr>, Vec<(Name, Ty, Expr)>) = match scrutinee {
        Expr::Tuple(xs) if xs.iter().all(is_place) => (xs, Vec::new()),
        Expr::Tuple(xs) => {
            let mut subjects = Vec::new();
            let mut lets = Vec::new();
            for (x, t) in xs.into_iter().zip(&tys) {
                let n = lw.name("e");
                subjects.push(Expr::Var(n.clone()));
                lets.push((n, t.clone(), x));
            }
            (subjects, lets)
        }
        // A tuple value. A place is read where it sits (`t[0]`); anything
        // else is bound once, then read the same way. No second name copies
        // each element.
        other => {
            let (base, lets) = if is_place(&other) {
                (other, Vec::new())
            } else {
                let whole = lw.name("t");
                let lets = vec![(whole.clone(), Ty::Tuple(tys.clone()), other)];
                (Expr::Var(whole), lets)
            };
            let subjects = (0..tys.len())
                .map(|i| Expr::Field {
                    base: Box::new(base.clone()),
                    name: tuple_field(i),
                })
                .collect();
            (subjects, lets)
        }
    };
    let subjects: Vec<(Expr, Ty)> = subjects.into_iter().zip(tys).collect();
    let tree = lw.compile(&subjects, rows);
    wrap(lets, tree)
}

fn is_place(expr: &Expr) -> bool {
    match expr {
        Expr::Var(_) => true,
        Expr::Field { base, .. } => is_place(base),
        _ => false,
    }
}

fn refutable(p: &Pattern) -> bool {
    !matches!(p, Pattern::Wildcard | Pattern::Var(_))
}

/// `let n: t = v; .. body`, in order.
fn wrap(lets: Vec<(Name, Ty, Expr)>, body: Expr) -> Expr {
    lets.into_iter().rev().fold(body, |then, (name, ty, value)| Expr::Let {
        name,
        mutable: false,
        ty: Some(ty),
        value: Box::new(value),
        then: Box::new(then),
    })
}

impl Lowering<'_, '_, '_> {
    /// A field's type as `compile` reads it: `Box` and friends peeled,
    /// aliases followed.
    fn norm(&self, ty: &Ty) -> Ty {
        let mut t = ty.peel().clone();
        for _ in 0..32 {
            match &t {
                Ty::Named(n) => match self.defs.aliases.get(n.as_str()) {
                    Some(a) => t = a.ty.peel().clone(),
                    None => break,
                },
                _ => break,
            }
        }
        t
    }

    /// Not a Rust identifier, so no source name is shadowed or captured.
    fn name(&mut self, what: &str) -> Name {
        *self.fresh += 1;
        Name::new(format!("${what}{}", self.fresh))
    }

    fn compile(&mut self, subjects: &[(Expr, Ty)], mut rows: Vec<Row>) -> Expr {
        if rows.is_empty() {
            // Only after an `if` on a literal whose `else` rustc knows no
            // value reaches (ranges covering the type).
            return Expr::Unreachable;
        }
        let Some(col) = rows[0].pats.iter().position(refutable) else {
            let mut row = rows.remove(0);
            return match row.guard.take() {
                None => leaf(subjects, row),
                // The bindings are read where the guard reads them, so the
                // `else` sees none of them.
                Some(guard) => {
                    let binds = bindings(subjects, &row);
                    let cond = substitute(guard, &binds);
                    Expr::If {
                        cond: Box::new(cond),
                        then: Box::new(leaf(subjects, row)),
                        else_: Box::new(self.compile(subjects, rows)),
                    }
                }
            };
        };
        let (subject, ty) = &subjects[col];
        match ty {
            Ty::Named(n) if self.defs.enums.contains_key(n.as_str()) => self.enum_column(subjects, col, n, rows),
            Ty::Option(inner) => {
                let some = self.name("v");
                let arms = vec![
                    (Pattern::OptionSome(Box::new(Pattern::Var(some.clone()))), Some((some, (**inner).clone()))),
                    (Pattern::OptionNone, None),
                ];
                self.two_way(subjects, col, subject, arms, rows)
            }
            Ty::Result { ok, err } => {
                let (o, e) = (self.name("v"), self.name("v"));
                let arms = vec![
                    (Pattern::ResultOk(Box::new(Pattern::Var(o.clone()))), Some((o, (**ok).clone()))),
                    (Pattern::ResultErr(Box::new(Pattern::Var(e.clone()))), Some((e, (**err).clone()))),
                ];
                self.two_way(subjects, col, subject, arms, rows)
            }
            _ => self.lit_column(subjects, col, rows),
        }
    }

    /// A `switch` over every variant, in declaration order. A variant whose
    /// case binds no field and compiles to the same code as an earlier one
    /// joins that arm with `|`.
    fn enum_column(&mut self, subjects: &[(Expr, Ty)], col: usize, en: &Name, rows: Vec<Row>) -> Expr {
        let variants = self.defs.enums[en.as_str()].variants.clone();
        let mut arms: Vec<Arm> = Vec::new();
        for v in &variants {
            // The fields some row binds, each to one fresh name.
            let (tys, named): (Vec<Ty>, Vec<Name>) = match &v.fields {
                VariantFields::Unit => (Vec::new(), Vec::new()),
                VariantFields::Tuple(ts) => (ts.clone(), Vec::new()),
                VariantFields::Struct(fs) => (fs.iter().map(|f| f.ty.clone()).collect(), fs.iter().map(|f| f.name.clone()).collect()),
            };
            let column_tys: Vec<Ty> = tys.iter().map(|t| self.norm(t)).collect();
            // Named after the field where it has one (`$verified`).
            let fresh: Vec<Name> = (0..tys.len()).map(|i| match named.get(i) {
                Some(f) => self.name(f.as_str()),
                None => self.name("f"),
            }).collect();
            let fields_of = |bind: VariantBind| -> Vec<(usize, Pattern)> {
                match bind {
                    VariantBind::Unit => Vec::new(),
                    VariantBind::Tuple(ps) => ps.into_iter().enumerate().collect(),
                    VariantBind::Struct(ps) => ps
                        .into_iter()
                        .filter_map(|(f, p)| named.iter().position(|n| *n == f).map(|i| (i, p)))
                        .collect(),
                }
            };
            // A field some row tests inside (`verified: false`) becomes a
            // column of its own, matched further in.
            let columns: Vec<usize> = (0..tys.len())
                .filter(|&i| {
                    rows.iter().any(|row| match &row.pats[col] {
                        Pattern::Variant { variant, bind, .. } if *variant == v.name => {
                            fields_of(bind.clone()).iter().any(|(j, p)| *j == i && p.refutable())
                        }
                        _ => false,
                    })
                })
                .collect();
            let mut used: Vec<bool> = (0..tys.len()).map(|i| columns.contains(&i)).collect();
            let kept: Vec<Row> = rows
                .iter()
                .filter_map(|row| {
                    let mut row = row.clone();
                    let p = std::mem::replace(&mut row.pats[col], Pattern::Wildcard);
                    let mut inner = vec![Pattern::Wildcard; columns.len()];
                    match p {
                        Pattern::Wildcard => {}
                        Pattern::Var(n) => row.binds.push((n, subjects[col].1.clone(), subjects[col].0.clone())),
                        Pattern::Variant { variant, bind, .. } if variant == v.name => {
                            for (i, p) in fields_of(bind) {
                                match columns.iter().position(|c| *c == i) {
                                    Some(k) => inner[k] = p,
                                    None => bind_names(&mut row, p, &tys[i], Expr::Var(fresh[i].clone()), &mut used[i]),
                                }
                            }
                        }
                        Pattern::Or(alts) if alts.iter().any(|a| matches!(a, Pattern::Variant { variant, .. } if *variant == v.name)) => {}
                        _ => return None,
                    }
                    row.pats.extend(inner);
                    Some(row)
                })
                .collect();
            let mut inner_subjects = subjects.to_vec();
            inner_subjects.extend(columns.iter().map(|&i| (Expr::Var(fresh[i].clone()), column_tys[i].clone())));
            let body = self.compile(&inner_subjects, kept);
            let bind = match &v.fields {
                VariantFields::Unit => VariantBind::Unit,
                VariantFields::Tuple(_) => VariantBind::Tuple(
                    fresh
                        .iter()
                        .zip(&used)
                        .map(|(n, u)| if *u { Pattern::Var(n.clone()) } else { Pattern::Wildcard })
                        .collect(),
                ),
                VariantFields::Struct(_) => VariantBind::Struct(
                    named
                        .iter()
                        .zip(&fresh)
                        .zip(&used)
                        .filter(|(_, u)| **u)
                        .map(|((f, n), _)| (f.clone(), Pattern::Var(n.clone())))
                        .collect(),
                ),
            };
            let pattern = Pattern::Variant { ty: en.clone(), variant: v.name.clone(), bind };
            let binds_nothing = !used.contains(&true);
            let shared = arms.iter_mut().find(|a| {
                binds_nothing && a.body == body && a.pattern.bindings().is_empty() && !matches!(a.pattern, Pattern::Tuple(_))
            });
            match shared {
                Some(arm) => {
                    let earlier = std::mem::replace(&mut arm.pattern, Pattern::Wildcard);
                    arm.pattern = match earlier {
                        Pattern::Or(mut alts) => {
                            alts.push(pattern);
                            Pattern::Or(alts)
                        }
                        single => Pattern::Or(vec![single, pattern]),
                    };
                }
                None => arms.push(Arm { guard: None, pattern, body }),
            }
        }
        Expr::Match { scrutinee: Box::new(subjects[col].0.clone()), arms }
    }

    /// `Option` or `Result`: both cases, each binding its payload to a fresh
    /// name that the rows' own bindings then read.
    fn two_way(
        &mut self,
        subjects: &[(Expr, Ty)],
        col: usize,
        subject: &Expr,
        cases: Vec<(Pattern, Option<(Name, Ty)>)>,
        rows: Vec<Row>,
    ) -> Expr {
        let arms = cases
            .into_iter()
            .map(|(case, payload)| {
                let payload_of = |p: &Pattern| -> Option<Pattern> {
                    match (p, &case) {
                        (Pattern::OptionSome(q), Pattern::OptionSome(_))
                        | (Pattern::ResultOk(q), Pattern::ResultOk(_))
                        | (Pattern::ResultErr(q), Pattern::ResultErr(_)) => Some((**q).clone()),
                        _ => None,
                    }
                };
                // A payload some row tests inside (`Some(Event::Pay { .. })`)
                // becomes a column of its own, matched further in.
                let column = payload.is_some() && rows.iter().any(|row| payload_of(&row.pats[col]).is_some_and(|q| q.refutable()));
                let mut uses = column;
                let kept: Vec<Row> = rows
                    .iter()
                    .filter_map(|row| {
                        let mut row = row.clone();
                        let p = std::mem::replace(&mut row.pats[col], Pattern::Wildcard);
                        let inner = match (&p, &case) {
                            (Pattern::Wildcard, _) => None,
                            (Pattern::Var(n), _) => {
                                row.binds.push((n.clone(), subjects[col].1.clone(), subject.clone()));
                                None
                            }
                            (Pattern::OptionNone, Pattern::OptionNone) => None,
                            _ => Some(payload_of(&p)?),
                        };
                        match (inner, &payload) {
                            (inner, Some(_)) if column => row.pats.push(inner.unwrap_or(Pattern::Wildcard)),
                            (Some(inner), Some((v, t))) => bind_names(&mut row, inner, t, Expr::Var(v.clone()), &mut uses),
                            _ => {}
                        }
                        Some(row)
                    })
                    .collect();
                let mut inner_subjects = subjects.to_vec();
                if let (true, Some((v, t))) = (column, &payload) {
                    inner_subjects.push((Expr::Var(v.clone()), self.norm(t)));
                }
                let body = self.compile(&inner_subjects, kept);
                let pattern = match case {
                    Pattern::OptionSome(_) if !uses => Pattern::OptionSome(Box::new(Pattern::Wildcard)),
                    Pattern::ResultOk(_) if !uses => Pattern::ResultOk(Box::new(Pattern::Wildcard)),
                    Pattern::ResultErr(_) if !uses => Pattern::ResultErr(Box::new(Pattern::Wildcard)),
                    other => other,
                };
                Arm { guard: None, pattern, body }
            })
            .collect();
        Expr::Match { scrutinee: Box::new(subject.clone()), arms }
    }

    /// Integers, `char`s, strings: `match s { p => yes, _ => no }` on the
    /// first row's pattern. A row with the same pattern is decided by the
    /// test; any other keeps its own and is tested again further in.
    fn lit_column(&mut self, subjects: &[(Expr, Ty)], col: usize, rows: Vec<Row>) -> Expr {
        let p = rows[0].pats[col].clone();
        // Where `p` matched: rows with `p` go on without testing it again;
        // a row that may still match (a range around `p`) tests again. Where
        // it did not, rows that `p` fully covers drop out. A `bool` is
        // narrowed by TS after one test, so its rows are settled here.
        let yes: Vec<Row> = rows
            .iter()
            .filter_map(|row| {
                let mut row = row.clone();
                match after(&row.pats[col], &p, true) {
                    Some(q) => row.pats[col] = q,
                    None => return None,
                }
                Some(row)
            })
            .collect();
        let no: Vec<Row> = rows
            .into_iter()
            .filter_map(|mut row| {
                row.pats[col] = after(&row.pats[col], &p, false)?;
                Some(row)
            })
            .collect();
        let (yes, no) = (self.compile(subjects, yes), self.compile(subjects, no));
        Expr::Match {
            scrutinee: Box::new(subjects[col].0.clone()),
            arms: vec![Arm { guard: None, pattern: p, body: yes }, Arm { guard: None, pattern: Pattern::Wildcard, body: no }],
        }
    }
}

/// Every element left is `_` or a binding: bind, then run the body.
fn leaf(subjects: &[(Expr, Ty)], row: Row) -> Expr {
    let mut binds = row.binds;
    for (p, (subject, ty)) in row.pats.into_iter().zip(subjects) {
        if let Pattern::Var(n) = p {
            binds.push((n, ty.clone(), subject.clone()));
        }
    }
    wrap(binds, row.body)
}

/// What is left of `q` once a literal test `p` has (`hit`) or has not
/// matched: `_` when it is sure to match, `None` when it cannot, else `q`
/// to be tested again.
fn after(q: &Pattern, p: &Pattern, hit: bool) -> Option<Pattern> {
    if !refutable(q) {
        return Some(q.clone());
    }
    if q == p {
        return if hit { Some(Pattern::Wildcard) } else { None };
    }
    if let (Some(qs), Some(ps)) = (bools(q), bools(p)) {
        // The values of `q` still possible: `p`'s own if it matched, the
        // other one if not.
        let left: Vec<bool> = [true, false].into_iter().filter(|b| ps.contains(b) == hit).collect();
        return if left.iter().all(|b| qs.contains(b)) {
            Some(Pattern::Wildcard)
        } else if left.iter().any(|b| qs.contains(b)) {
            Some(q.clone())
        } else {
            None
        };
    }
    // Two single literals of one type are the same value only if equal.
    if hit && matches!((q, p), (Pattern::Lit(_), Pattern::Lit(_))) {
        return None;
    }
    Some(q.clone())
}

fn bools(p: &Pattern) -> Option<Vec<bool>> {
    match p {
        Pattern::Lit(purecrate_ir::Lit::Bool(b)) => Some(vec![*b]),
        Pattern::Or(alts) => alts.iter().map(bools).try_fold(Vec::new(), |mut acc, b| {
            acc.extend(b?);
            Some(acc)
        }),
        _ => None,
    }
}

/// `pattern` is `_`, a name, or a tuple of those. A name reads `place`;
/// a tuple's names read `place[i]`. `used` is set when any name is bound,
/// so the place itself is kept.
fn bind_names(row: &mut Row, pattern: Pattern, ty: &Ty, place: Expr, used: &mut bool) {
    match pattern {
        Pattern::Var(n) => {
            *used = true;
            row.binds.push((n, ty.clone(), place));
        }
        Pattern::Tuple(ps) => {
            let elems = match ty {
                Ty::Tuple(xs) => xs.clone(),
                _ => Vec::new(),
            };
            for (i, p) in ps.into_iter().enumerate() {
                if let Pattern::Var(n) = p {
                    *used = true;
                    row.binds.push((
                        n,
                        elems.get(i).cloned().unwrap_or(Ty::Never),
                        Expr::Field { base: Box::new(place.clone()), name: tuple_field(i) },
                    ));
                }
            }
        }
        _ => {}
    }
}

/// Every name a row binds, with the place it reads.
fn bindings(subjects: &[(Expr, Ty)], row: &Row) -> Vec<(Name, Ty, Expr)> {
    let mut binds = row.binds.clone();
    for (p, (subject, ty)) in row.pats.iter().zip(subjects) {
        if let Pattern::Var(n) = p {
            binds.push((n.clone(), ty.clone(), subject.clone()));
        }
    }
    binds
}

/// `expr` with each bound name read from its place. The places are names
/// and fields, so reading one twice is the same as binding it once.
fn substitute(expr: Expr, binds: &[(Name, Ty, Expr)]) -> Expr {
    fn go(expr: &mut Expr, binds: &[(Name, Ty, Expr)]) {
        match expr {
            Expr::Var(n) => {
                if let Some((_, _, place)) = binds.iter().find(|(b, _, _)| b == n) {
                    *expr = place.clone();
                }
            }
            // A binder of the same name hides it from here on.
            Expr::Let { name, value, then, .. } => {
                go(value, binds);
                let inner: Vec<_> = binds.iter().filter(|(b, _, _)| b != name).cloned().collect();
                go(then, &inner);
            }
            Expr::Closure { params, body, .. } => {
                let inner: Vec<_> = binds.iter().filter(|(b, _, _)| !params.iter().any(|p| p.name == *b)).cloned().collect();
                go(body, &inner);
            }
            Expr::Match { scrutinee, arms } => {
                go(scrutinee, binds);
                for arm in arms {
                    let bound = arm.pattern.bindings();
                    let inner: Vec<_> = binds.iter().filter(|(b, _, _)| !bound.contains(&b)).cloned().collect();
                    if let Some(g) = &mut arm.guard {
                        go(g, &inner);
                    }
                    go(&mut arm.body, &inner);
                }
            }
            Expr::For { var, start, end, body, .. } => {
                go(start, binds);
                go(end, binds);
                let inner: Vec<_> = binds.iter().filter(|(b, _, _)| b != var).cloned().collect();
                go(body, &inner);
            }
            Expr::ForEach { var, source, body, .. } => {
                go(source, binds);
                let inner: Vec<_> = binds.iter().filter(|(b, _, _)| b != var).cloned().collect();
                go(body, &inner);
            }
            other => other.children_mut().into_iter().for_each(|c| go(c, binds)),
        }
    }
    let mut expr = expr;
    go(&mut expr, binds);
    expr
}
