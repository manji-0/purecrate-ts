//! `match` becomes `switch (x.kind)` with `assertNever` in `default`, or a
//! two-way `if` for `Option`/`Result`. Every arm must name a case of one type
//! and every case must appear once.

use std::collections::HashMap;

use purecrate_ir::{Arm, Crate, Enum, Expr, Item, Pattern, Pos, Reason};

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
            walk(&f.body, None, &mut |arms, at| {
                let before = out.len();
                match_arms(i, arms, &enums, &mut out);
                if let Some(at) = at {
                    crate::locate(&mut out[before..], at);
                }
            });
        }
    }
    out
}

/// Which type an arm's pattern belongs to, and the case it names.
fn case_of(pattern: &Pattern) -> Option<(&str, &str)> {
    match pattern {
        Pattern::Variant { ty, variant, .. } => Some((ty.as_str(), variant.as_str())),
        Pattern::OptionSome(_) => Some(("Option", "Some")),
        Pattern::OptionNone => Some(("Option", "None")),
        Pattern::ResultOk(_) => Some(("Result", "Ok")),
        Pattern::ResultErr(_) => Some(("Result", "Err")),
        Pattern::Wildcard | Pattern::Var(_) | Pattern::Lit(_) => None,
    }
}

fn label(ty: &str, case: &str) -> String {
    match ty {
        "Option" | "Result" => format!("`{case}`"),
        _ => format!("`{ty}::{case}`"),
    }
}

fn match_arms(i: usize, arms: &[Arm], enums: &HashMap<&str, &Enum>, out: &mut Vec<Diagnostic>) {
    let mut ty: Option<&str> = None;
    let mut seen: Vec<&str> = Vec::new();
    for arm in arms {
        let Some((t, case)) = case_of(&arm.pattern) else {
            out.push(Diagnostic::at(i, Reason::ArmPattern, "match arms must name an enum variant, `Some`/`None` or `Ok`/`Err`"));
            return;
        };
        match ty {
            None => ty = Some(t),
            Some(first) if first != t => {
                out.push(Diagnostic::at(i, Reason::NonExhaustive, format!("match mixes cases of `{first}` and `{t}`")));
                return;
            }
            Some(_) => {}
        }
        if seen.contains(&case) {
            out.push(Diagnostic::at(i, Reason::NonExhaustive, format!("{} is matched more than once", label(t, case))));
        }
        seen.push(case);
    }
    let Some(ty) = ty else {
        out.push(Diagnostic::at(i, Reason::NonExhaustive, "match has no arms"));
        return;
    };
    let all: Vec<&str> = match ty {
        "Option" => vec!["Some", "None"],
        "Result" => vec!["Ok", "Err"],
        // Unknown enums are reported by name resolution.
        _ => match enums.get(ty) {
            Some(e) => e.variants.iter().map(|v| v.name.as_str()).collect(),
            None => return,
        },
    };
    let missing: Vec<String> = all
        .into_iter()
        .filter(|c| !seen.contains(c))
        .map(|c| label(ty, c))
        .collect();
    if !missing.is_empty() {
        out.push(Diagnostic::at(
            i, Reason::NonExhaustive,
            format!("match on `{ty}` is missing {}", missing.join(", ")),
        ));
    }
}

/// `at` is the innermost `Expr::At` around `expr`.
fn walk(expr: &Expr, at: Option<Pos>, on_match: &mut impl FnMut(&[Arm], Option<Pos>)) {
    let at = match expr {
        Expr::At { at, .. } => Some(*at),
        _ => at,
    };
    if let Expr::Match { arms, .. } = expr {
        on_match(arms, at);
    }
    expr.children().into_iter().for_each(|c| walk(c, at, on_match));
}
