//! The one array the output writes: a local `let mut v: Vec<T>` that
//! `v.push(x)`, `v.insert(i, x)`, `v.remove(i)`, or `v[i] = x` writes
//! (design/01 §7.14). Run after `rename`, so a name is one
//! binding in its function.
//!
//! Such a local is bound to an array of its own: a value that is not a new
//! array (`vec![..]`, `Vec::new()`, `clone`, `collect`) is copied when bound
//! or assigned, since in TS the caller may still hold the array a
//! parameter, a field, or a returned value is. No closure reads it: a
//! closure would see the array as it is when called, not when made.
//!
//! An array `[T; N]` is `Copy` (design/01 §7.27): `let c = a;` leaves `a`
//! to be written again, where a `Vec` is moved. So where a written array
//! local is read whole into a value that outlives the read (bound, passed,
//! put in a value, returned), and a write may follow, the read is a copy.

use std::collections::{HashMap, HashSet};

use purecrate_ir::{Callee, Crate, Expr, Item, Name, Over, Reason, TreeOp, Ty};

use crate::Diagnostic;

pub fn grow(mut krate: Crate) -> Result<Crate, Vec<Diagnostic>> {
    let mut out = Vec::new();
    let aliases: HashMap<Name, Ty> = krate
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Alias(a) => Some((a.name.clone(), a.ty.clone())),
            _ => None,
        })
        .collect();
    for (i, item) in krate.items.iter_mut().enumerate() {
        let Item::Fn(f) = item else { continue };
        let mut kinds: HashMap<Name, Kind> = HashMap::new();
        f.body.walk(|e| {
            if let Some(n) = e.grown() {
                kinds.insert(n.clone(), kind(e));
            }
        });
        if kinds.is_empty() {
            continue;
        }
        let pushed: HashSet<Name> = kinds.keys().cloned().collect();
        if let Some(n) = captured(&f.body, &pushed, false) {
            out.push(Diagnostic::at(
                i,
                Reason::Closure,
                format!(
                    "a closure reads `{}`, which the function writes in place; read it outside the closure, or write a copy",
                    n.as_str()
                ),
            ));
            continue;
        }
        own(&mut f.body, &kinds);
        let arrays: HashSet<Name> = kinds.keys().filter(|n| is_array_local(&f.body, n, &aliases)).cloned().collect();
        for n in &arrays {
            copy_reads(&mut f.body, n);
        }
    }
    if out.is_empty() {
        Ok(krate)
    } else {
        Err(out)
    }
}

/// A grown local a closure reads.
fn captured(expr: &Expr, pushed: &HashSet<Name>, inside: bool) -> Option<Name> {
    match expr {
        Expr::Var(n) if inside && pushed.contains(n) => Some(n.clone()),
        Expr::Closure { body, .. } => captured(body, pushed, true),
        e => e.children().into_iter().find_map(|c| captured(c, pushed, inside)),
    }
}

/// What a grown local holds: an array, or (design/01 §7.19) a map or a set.
#[derive(Clone, Copy)]
enum Kind {
    Array,
    Map,
    Set,
}

fn kind(write: &Expr) -> Kind {
    match write {
        Expr::Call {
            callee: Callee::Tree(TreeOp::Insert | TreeOp::RemoveKey | TreeOp::Put | TreeOp::Take), ..
        } => Kind::Map,
        Expr::Call { callee: Callee::Tree(_), .. } => Kind::Set,
        _ => Kind::Array,
    }
}

/// A new array, map, or set: nothing else holds it.
fn fresh(value: &Expr) -> bool {
    matches!(
        value,
        Expr::Array(_)
            | Expr::Call { callee: Callee::ArrayRepeat { .. }, .. }
            | Expr::Call { callee: Callee::Collect { .. }, .. }
            | Expr::Call {
                callee: Callee::Tree(
                    TreeOp::NewMap
                        | TreeOp::NewSet
                        | TreeOp::CloneMap
                        | TreeOp::CloneSet
                        | TreeOp::FromPairs
                        | TreeOp::FromKeys
                ),
                ..
            }
    )
}

fn copy(value: &mut Expr, kind: Kind) {
    if !fresh(value) {
        let v = std::mem::replace(value, Expr::Array(Vec::new()));
        let callee = match kind {
            Kind::Array => Callee::Collect { result: false, over: Over::Items },
            Kind::Map => Callee::Tree(TreeOp::CloneMap),
            Kind::Set => Callee::Tree(TreeOp::CloneSet),
        };
        *value = Expr::Call { callee, args: vec![v] };
    }
}

/// Each binding and assignment of a grown local takes a value of its own.
fn own(expr: &mut Expr, kinds: &HashMap<Name, Kind>) {
    match expr {
        Expr::Let { name, value, .. } | Expr::Assign { name, value } if kinds.contains_key(name) => {
            copy(value, kinds[name])
        }
        _ => {}
    }
    expr.children_mut().into_iter().for_each(|c| own(c, kinds));
}

/// Whether the local `n` is bound to an array `[T; N]`, as its `let`
/// says (aliases followed).
fn is_array_local(body: &Expr, n: &Name, aliases: &HashMap<Name, Ty>) -> bool {
    let mut found = false;
    body.walk(|e| {
        if let Expr::Let { name, ty: Some(t), .. } = e {
            if name == n {
                let mut t = t.peel();
                for _ in 0..32 {
                    match t {
                        Ty::Named(a) if aliases.contains_key(a) => t = aliases[a].peel(),
                        _ => break,
                    }
                }
                found |= matches!(t, Ty::Array { .. });
            }
        }
    });
    found
}

/// Each read of the array local `n` whole into a value that outlives it,
/// where a write of `n` may come after (later in the order the body runs,
/// or anywhere in a loop around the read), as a copy (`[...a]`).
fn copy_reads(body: &mut Expr, n: &Name) {
    #[derive(Default)]
    struct Scan {
        /// Every node is numbered in the order it finishes running.
        clock: usize,
        writes: Vec<usize>,
        /// Each kept read, and the loops around it, by index into `spans`.
        reads: Vec<(usize, Vec<usize>)>,
        /// Each loop's first and last number.
        spans: Vec<(usize, usize)>,
        open: Vec<usize>,
        /// `for x in a` by value, and its loop: Rust walks a copy, so a
        /// write in the body must not show in a later `x`.
        sources: Vec<(usize, usize)>,
    }
    fn scan(e: &Expr, n: &Name, keeps: bool, s: &mut Scan) {
        let looping = matches!(e, Expr::While { .. } | Expr::For { .. } | Expr::ForEach { .. });
        if looping {
            s.spans.push((s.clock, usize::MAX));
            s.open.push(s.spans.len() - 1);
        }
        let kept = kept_children(e);
        for (i, c) in e.children().into_iter().enumerate() {
            if i == 0 && matches!(e, Expr::ForEach { .. }) && matches!(c, Expr::Var(v) if v == n) {
                s.clock += 1;
                s.sources.push((s.clock, *s.open.last().expect("the loop is open")));
                continue;
            }
            scan(c, n, kept.contains(&i), s);
        }
        s.clock += 1;
        if e.grown() == Some(n) {
            s.writes.push(s.clock);
        }
        if keeps && matches!(e, Expr::Var(v) if v == n) {
            s.reads.push((s.clock, s.open.clone()));
        }
        if looping {
            let id = s.open.pop().expect("pushed above");
            s.spans[id].1 = s.clock;
        }
    }
    let mut s = Scan::default();
    scan(body, n, false, &mut s);
    let copied: HashSet<usize> = s
        .reads
        .iter()
        .filter(|(at, loops)| {
            s.writes.iter().any(|w| w > at || loops.iter().any(|&l| (s.spans[l].0..=s.spans[l].1).contains(w)))
        })
        .map(|(at, _)| *at)
        .chain(
            s.sources
                .iter()
                .filter(|(_, l)| s.writes.iter().any(|w| (s.spans[*l].0..=s.spans[*l].1).contains(w)))
                .map(|(at, _)| *at),
        )
        .collect();
    if copied.is_empty() {
        return;
    }
    fn rewrite(e: &mut Expr, n: &Name, copied: &HashSet<usize>, clock: &mut usize) {
        for c in e.children_mut() {
            rewrite(c, n, copied, clock);
        }
        *clock += 1;
        if copied.contains(clock) && matches!(e, Expr::Var(v) if v == n) {
            let v = std::mem::replace(e, Expr::Array(Vec::new()));
            *e = Expr::Call { callee: Callee::Collect { result: false, over: Over::Items }, args: vec![v] };
        }
    }
    rewrite(body, n, &copied, &mut 0);
}

/// Which children of `e` are values `e` keeps or gives on (bound, put in a
/// value, passed to a function, returned), rather than read and done with.
fn kept_children(e: &Expr) -> Vec<usize> {
    match e {
        Expr::Let { .. } => vec![0, 1],
        Expr::Assign { .. } | Expr::Return(_) | Expr::Break(Some(_)) => vec![0],
        Expr::Seq { .. } => vec![1],
        Expr::If { .. } => vec![1, 2],
        Expr::Match { arms, .. } => {
            let mut out = Vec::new();
            let mut i = 1;
            for a in arms {
                i += usize::from(a.guard.is_some());
                out.push(i);
                i += 1;
            }
            out
        }
        Expr::Tuple(xs) | Expr::Array(xs) => (0..xs.len()).collect(),
        Expr::Construct { .. } | Expr::Ignored { .. } => (0..e.children().len()).collect(),
        Expr::Call {
            callee:
                Callee::Fn(_)
                | Callee::Local(_)
                | Callee::Method { .. }
                | Callee::Dict { .. }
                | Callee::StructNew(_)
                | Callee::OptionSome
                | Callee::ResultOk
                | Callee::ResultErr
                | Callee::ArrayRepeat { .. },
            args,
        } => (0..args.len()).collect(),
        // The written local is the first; what is put in, the rest.
        Expr::Call { callee: Callee::VecPush | Callee::VecInsert | Callee::VecSet, args } => (1..args.len()).collect(),
        _ => Vec::new(),
    }
}
