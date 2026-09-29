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

use purecrate_ir::{tuple_field, Arm, Expr, Name, Pattern, Ty, VariantBind, VariantFields};

use crate::defs::Defs;

/// One tuple arm still in play: the patterns of the elements not yet
/// matched, the names bound so far, and the body.
#[derive(Clone)]
struct Row {
    pats: Vec<Pattern>,
    binds: Vec<(Name, Ty, Expr)>,
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
            rows.push(Row { pats, binds: Vec::new(), body: arm.body.clone() });
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
        // A tuple value: bind it, then read each element into its own name.
        other => {
            let whole = lw.name("t");
            let mut subjects = Vec::new();
            let mut lets = vec![(whole.clone(), Ty::Tuple(tys.clone()), other)];
            for (i, t) in tys.iter().enumerate() {
                let n = lw.name("e");
                subjects.push(Expr::Var(n.clone()));
                let elem = Expr::Field { base: Box::new(Expr::Var(whole.clone())), name: tuple_field(i) };
                lets.push((n, t.clone(), elem));
            }
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
            let row = rows.swap_remove(0);
            return leaf(subjects, row);
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
            let fresh: Vec<Name> = tys.iter().map(|_| self.name("f")).collect();
            let mut used = vec![false; tys.len()];
            let kept: Vec<Row> = rows
                .iter()
                .filter_map(|row| {
                    let mut row = row.clone();
                    let p = std::mem::replace(&mut row.pats[col], Pattern::Wildcard);
                    match p {
                        Pattern::Wildcard => {}
                        Pattern::Var(n) => row.binds.push((n, subjects[col].1.clone(), subjects[col].0.clone())),
                        Pattern::Variant { variant, bind, .. } if variant == v.name => {
                            let fields: Vec<(usize, Pattern)> = match bind {
                                VariantBind::Unit => Vec::new(),
                                VariantBind::Tuple(ps) => ps.into_iter().enumerate().collect(),
                                VariantBind::Struct(ps) => ps
                                    .into_iter()
                                    .filter_map(|(f, p)| named.iter().position(|n| *n == f).map(|i| (i, p)))
                                    .collect(),
                            };
                            for (i, p) in fields {
                                if let Pattern::Var(n) = p {
                                    used[i] = true;
                                    row.binds.push((n, tys[i].clone(), Expr::Var(fresh[i].clone())));
                                }
                            }
                        }
                        Pattern::Or(alts) if alts.iter().any(|a| matches!(a, Pattern::Variant { variant, .. } if *variant == v.name)) => {}
                        _ => return None,
                    }
                    Some(row)
                })
                .collect();
            let body = self.compile(subjects, kept);
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
                None => arms.push(Arm { pattern, body }),
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
                let mut uses = false;
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
                            (Pattern::OptionSome(q), Pattern::OptionSome(_))
                            | (Pattern::ResultOk(q), Pattern::ResultOk(_))
                            | (Pattern::ResultErr(q), Pattern::ResultErr(_)) => Some(&**q),
                            (Pattern::OptionNone, Pattern::OptionNone) => None,
                            _ => return None,
                        };
                        if let (Some(Pattern::Var(n)), Some((v, t))) = (inner, &payload) {
                            uses = true;
                            row.binds.push((n.clone(), t.clone(), Expr::Var(v.clone())));
                        }
                        Some(row)
                    })
                    .collect();
                let body = self.compile(subjects, kept);
                let pattern = match case {
                    Pattern::OptionSome(_) if !uses => Pattern::OptionSome(Box::new(Pattern::Wildcard)),
                    Pattern::ResultOk(_) if !uses => Pattern::ResultOk(Box::new(Pattern::Wildcard)),
                    Pattern::ResultErr(_) if !uses => Pattern::ResultErr(Box::new(Pattern::Wildcard)),
                    other => other,
                };
                Arm { pattern, body }
            })
            .collect();
        Expr::Match { scrutinee: Box::new(subject.clone()), arms }
    }

    /// Integers, `char`s, strings: `match s { p => yes, _ => no }` on the
    /// first row's pattern. A row with the same pattern is decided by the
    /// test; any other keeps its own and is tested again further in.
    fn lit_column(&mut self, subjects: &[(Expr, Ty)], col: usize, rows: Vec<Row>) -> Expr {
        let p = rows[0].pats[col].clone();
        let yes: Vec<Row> = rows
            .iter()
            .map(|row| {
                let mut row = row.clone();
                if row.pats[col] == p {
                    row.pats[col] = Pattern::Wildcard;
                }
                row
            })
            .collect();
        let no: Vec<Row> = rows.into_iter().filter(|row| row.pats[col] != p).collect();
        let (yes, no) = (self.compile(subjects, yes), self.compile(subjects, no));
        Expr::Match {
            scrutinee: Box::new(subjects[col].0.clone()),
            arms: vec![Arm { pattern: p, body: yes }, Arm { pattern: Pattern::Wildcard, body: no }],
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
