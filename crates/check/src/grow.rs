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

use std::collections::{HashMap, HashSet};

use purecrate_ir::{Callee, Crate, Expr, Item, Name, Over, Reason, TreeOp};

use crate::Diagnostic;

pub fn grow(mut krate: Crate) -> Result<Crate, Vec<Diagnostic>> {
    let mut out = Vec::new();
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
