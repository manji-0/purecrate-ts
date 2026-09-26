//! `match` becomes `switch (x.kind)` with `assertNever` in `default`, so every
//! arm must name a variant of one enum and every variant must appear once.

use std::collections::HashMap;

use purecrate_ir::{Arm, Crate, Enum, Expr, Fields, Item, Pattern};

use crate::Diagnostic;

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let enums: HashMap<&str, &Enum> = krate
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) => Some((e.name.as_str(), e)),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        if let Item::Fn(f) = item {
            walk(&f.body, &mut |arms| match_arms(i, arms, &enums, &mut out));
        }
    }
    out
}

fn match_arms(i: usize, arms: &[Arm], enums: &HashMap<&str, &Enum>, out: &mut Vec<Diagnostic>) {
    let mut ty: Option<&str> = None;
    let mut seen: Vec<&str> = Vec::new();
    for arm in arms {
        let Pattern::Variant { ty: t, variant, .. } = &arm.pattern else {
            out.push(Diagnostic::at(i, "match arms must name an enum variant"));
            return;
        };
        match ty {
            None => ty = Some(t.as_str()),
            Some(first) if first != t.as_str() => {
                out.push(Diagnostic::at(
                    i,
                    format!("match mixes variants of `{first}` and `{}`", t.as_str()),
                ));
                return;
            }
            Some(_) => {}
        }
        if seen.contains(&variant.as_str()) {
            out.push(Diagnostic::at(
                i,
                format!("`{}::{}` is matched more than once", t.as_str(), variant.as_str()),
            ));
        }
        seen.push(variant.as_str());
    }
    let Some(ty) = ty else {
        out.push(Diagnostic::at(i, "match has no arms"));
        return;
    };
    // Unknown enums are reported by name resolution.
    let Some(e) = enums.get(ty) else { return };
    let missing: Vec<String> = e
        .variants
        .iter()
        .filter(|v| !seen.contains(&v.name.as_str()))
        .map(|v| format!("`{ty}::{}`", v.name.as_str()))
        .collect();
    if !missing.is_empty() {
        out.push(Diagnostic::at(
            i,
            format!("match on `{ty}` is missing {}", missing.join(", ")),
        ));
    }
}

fn walk(expr: &Expr, on_match: &mut impl FnMut(&[Arm])) {
    match expr {
        Expr::Match { scrutinee, arms } => {
            on_match(arms);
            walk(scrutinee, on_match);
            arms.iter().for_each(|a| walk(&a.body, on_match));
        }
        Expr::Let { value, then, .. } => {
            walk(value, on_match);
            walk(then, on_match);
        }
        Expr::If { cond, then, else_ } => {
            walk(cond, on_match);
            walk(then, on_match);
            walk(else_, on_match);
        }
        Expr::Call { args, .. } | Expr::Tuple(args) | Expr::Array(args) => {
            args.iter().for_each(|a| walk(a, on_match))
        }
        Expr::Construct { fields, .. } => match fields {
            Fields::Positional(xs) => xs.iter().for_each(|x| walk(x, on_match)),
            Fields::Named(xs) => xs.iter().for_each(|(_, x)| walk(x, on_match)),
            Fields::Unit => {}
        },
        Expr::Field { base, .. } | Expr::Unary { expr: base, .. } | Expr::Return(base) => {
            walk(base, on_match)
        }
        Expr::Binary { left, right, .. } => {
            walk(left, on_match);
            walk(right, on_match);
        }
        Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable => {}
    }
}
