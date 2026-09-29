//! `match` becomes `switch (x.kind)` with `assertNever` in `default`, or a
//! two-way `if` for `Option`/`Result`. Every arm must name cases of one type
//! (one, or several with `A | B`) and every case must appear once, unless a
//! last `_` arm takes the cases no other arm names. As in rustc, a `_` that
//! takes nothing is allowed (`matches!` makes one); it is dropped.

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

/// Which type an arm's pattern belongs to, and the cases it names.
fn cases_of(pattern: &Pattern) -> Option<Vec<(&str, &str)>> {
    match pattern {
        Pattern::Or(alts) => alts.iter().map(case_of).collect(),
        other => case_of(other).map(|c| vec![c]),
    }
}

fn case_of(pattern: &Pattern) -> Option<(&str, &str)> {
    match pattern {
        Pattern::Variant { ty, variant, .. } => Some((ty.as_str(), variant.as_str())),
        Pattern::OptionSome(_) => Some(("Option", "Some")),
        Pattern::OptionNone => Some(("Option", "None")),
        Pattern::ResultOk(_) => Some(("Result", "Ok")),
        Pattern::ResultErr(_) => Some(("Result", "Err")),
        Pattern::Wildcard
        | Pattern::Var(_)
        | Pattern::Lit(_)
        | Pattern::Or(_)
        | Pattern::Range { .. }
        | Pattern::Tuple(_) => None,
    }
}

fn label(ty: &str, case: &str) -> String {
    match ty {
        "Option" | "Result" => format!("`{case}`"),
        _ => format!("`{ty}::{case}`"),
    }
}

fn match_arms(i: usize, arms: &[Arm], enums: &HashMap<&str, &Enum>, out: &mut Vec<Diagnostic>) {
    if arms.iter().any(|a| a.pattern.is_tuple_case()) {
        return tuple_arms(i, arms, out);
    }
    if arms.iter().any(|a| a.pattern.is_int_case()) {
        return lit_arms(i, arms, "integer", Pattern::is_int_case, out);
    }
    if arms.iter().any(|a| a.pattern.is_char_case()) {
        return lit_arms(i, arms, "char", Pattern::is_char_case, out);
    }
    if arms.iter().any(|a| a.pattern.is_str_case()) {
        return lit_arms(i, arms, "string", Pattern::is_str_case, out);
    }
    let mut ty: Option<&str> = None;
    let mut seen: Vec<&str> = Vec::new();
    let mut rest = false;
    for (n, arm) in arms.iter().enumerate() {
        if arm.pattern == Pattern::Wildcard {
            if n + 1 != arms.len() {
                out.push(Diagnostic::at(i, Reason::ArmPattern, "`_` must be the last arm"));
                return;
            }
            rest = true;
            continue;
        }
        let Some(cases) = cases_of(&arm.pattern) else {
            out.push(Diagnostic::at(i, Reason::ArmPattern, "match arms must name an enum variant, `Some`/`None`, `Ok`/`Err`, or be `_`"));
            return;
        };
        for (t, case) in cases {
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
    }
    let Some(ty) = ty else {
        let message = if rest {
            "a match whose only arm is `_` names no type; bind the value with `let` instead"
        } else {
            "match has no arms"
        };
        out.push(Diagnostic::at(i, Reason::NonExhaustive, message));
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
    if !rest && !missing.is_empty() {
        out.push(Diagnostic::at(
            i, Reason::NonExhaustive,
            format!("match on `{ty}` is missing {}", missing.join(", ")),
        ));
    }
}

/// A `match` on a tuple is tried arm by arm, as an `if` chain. rustc has
/// checked it is exhaustive; here every arm must be a tuple of one width, or
/// a last `_`.
fn tuple_arms(i: usize, arms: &[Arm], out: &mut Vec<Diagnostic>) {
    let width = |p: &Pattern| match p {
        Pattern::Tuple(ps) => Some(ps.len()),
        Pattern::Or(alts) => alts.first().and_then(|a| match a {
            Pattern::Tuple(ps) => Some(ps.len()),
            _ => None,
        }),
        _ => None,
    };
    let first = arms.iter().find_map(|a| width(&a.pattern));
    for (n, arm) in arms.iter().enumerate() {
        match &arm.pattern {
            Pattern::Wildcard if n + 1 == arms.len() => {}
            Pattern::Wildcard => {
                out.push(Diagnostic::at(i, Reason::ArmPattern, "`_` must be the last arm"));
                return;
            }
            p if p.is_tuple_case() => {
                let widths = match p {
                    Pattern::Or(alts) => alts.iter().map(width).collect(),
                    other => vec![width(other)],
                };
                if widths.iter().any(|w| *w != first) {
                    out.push(Diagnostic::at(i, Reason::ArmPattern, "tuple arms of one `match` must have one width"));
                    return;
                }
            }
            _ => {
                out.push(Diagnostic::at(i, Reason::ArmPattern, "a match on a tuple takes tuple patterns, or a last `_`"));
                return;
            }
        }
    }
}

/// Integers, `char`s, and strings cannot be listed out, so the arms are tried in order
/// and a last `_` takes the rest, as an `if` chain in TS.
fn lit_arms(i: usize, arms: &[Arm], kind: &str, is_case: fn(&Pattern) -> bool, out: &mut Vec<Diagnostic>) {
    let (last, named) = arms.split_last().expect("a literal case was found");
    if let Some(other) = named.iter().find(|a| !is_case(&a.pattern)) {
        let found = if other.pattern == Pattern::Wildcard {
            "`_` must be the last arm".to_string()
        } else {
            format!("match mixes {kind} arms with other arms")
        };
        out.push(Diagnostic::at(i, Reason::ArmPattern, found));
    } else if last.pattern != Pattern::Wildcard {
        out.push(Diagnostic::at(i, Reason::NonExhaustive, format!("a match on {kind}s must end in a `_` arm in v0")));
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
