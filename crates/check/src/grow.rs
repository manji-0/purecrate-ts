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

use std::collections::HashSet;

use purecrate_ir::{Callee, Crate, Expr, Item, Name, Over, Reason};

use crate::Diagnostic;

pub fn grow(mut krate: Crate) -> Result<Crate, Vec<Diagnostic>> {
    let mut out = Vec::new();
    for (i, item) in krate.items.iter_mut().enumerate() {
        let Item::Fn(f) = item else { continue };
        let mut pushed = HashSet::new();
        f.body.walk(|e| pushed.extend(e.grown().cloned()));
        if pushed.is_empty() {
            continue;
        }
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
        own(&mut f.body, &pushed);
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

/// A new array: nothing else holds it.
fn fresh(value: &Expr) -> bool {
    matches!(value, Expr::Array(_) | Expr::Call { callee: Callee::Collect { .. }, .. })
}

fn copy(value: &mut Expr) {
    if !fresh(value) {
        let v = std::mem::replace(value, Expr::Array(Vec::new()));
        *value = Expr::Call { callee: Callee::Collect { result: false, over: Over::Items }, args: vec![v] };
    }
}

/// Each binding and assignment of a grown local takes an array of its own.
fn own(expr: &mut Expr, pushed: &HashSet<Name>) {
    match expr {
        Expr::Let { name, value, .. } | Expr::Assign { name, value } if pushed.contains(name) => copy(value),
        _ => {}
    }
    expr.children_mut().into_iter().for_each(|c| own(c, pushed));
}
