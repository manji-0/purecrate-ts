//! Replaces a trailing `_` arm with the cases the other arms leave, so the
//! printer lists every case (`case "A": case "B":`) and TS still checks the
//! `switch` is exhaustive. `exhaustive` has already required that `_` is
//! last and follows at least one arm. A `_` that takes no case is dropped.

use purecrate_ir::{Crate, Enum, Expr, Item, Pattern, VariantBind, VariantFields};

pub fn expand(mut krate: Crate) -> Crate {
    let enums: Vec<Enum> = krate
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Enum(e) => Some(e.clone()),
            _ => None,
        })
        .collect();
    for item in &mut krate.items {
        if let Item::Fn(f) = item {
            walk(&mut f.body, &enums);
        }
    }
    krate
}

fn walk(expr: &mut Expr, enums: &[Enum]) {
    if let Expr::Match { arms, .. } = expr {
        // `true => a, false => b`: `exhaustive` has checked both are named,
        // so the last arm takes what is left. One arm naming both
        // (`false | true`) becomes `true` and `_` with the same body.
        if arms.iter().all(|a| a.pattern.is_bool_case()) {
            if arms.len() == 1 {
                let body = arms[0].body.clone();
                arms[0].pattern = Pattern::Lit(purecrate_ir::Lit::Bool(true));
                arms.push(purecrate_ir::Arm { guard: None, pattern: Pattern::Wildcard, body });
            } else if let Some(last) = arms.last_mut() {
                last.pattern = Pattern::Wildcard;
            }
            expr.children_mut().into_iter().for_each(|c| walk(c, enums));
            return;
        }
        if let Some((last, named)) = arms.split_last_mut() {
            if last.pattern == Pattern::Wildcard {
                last.pattern = rest(named.iter().map(|a| &a.pattern), enums);
            }
        }
        if arms.last().is_some_and(|a| a.pattern == Pattern::Or(Vec::new())) {
            arms.pop();
        }
    }
    expr.children_mut().into_iter().for_each(|c| walk(c, enums));
}

fn rest<'a>(named: impl Iterator<Item = &'a Pattern>, enums: &[Enum]) -> Pattern {
    let named: Vec<&Pattern> = named
        .flat_map(|p| match p {
            Pattern::Or(alts) => alts.iter().collect(),
            other => vec![other],
        })
        .collect();
    let wild = || Box::new(Pattern::Wildcard);
    if named.len() == 2 && !named[0].is_lit_case() && !matches!(named[0], Pattern::Variant { .. }) {
        // `Some`/`None` or `Ok`/`Err` both named: nothing is left.
        return Pattern::Or(Vec::new());
    }
    match named.first() {
        Some(Pattern::OptionSome(_)) => Pattern::OptionNone,
        Some(Pattern::OptionNone) => Pattern::OptionSome(wild()),
        Some(Pattern::ResultOk(_)) => Pattern::ResultErr(wild()),
        Some(Pattern::ResultErr(_)) => Pattern::ResultOk(wild()),
        Some(Pattern::Variant { ty, .. }) => {
            let e = enums.iter().find(|e| e.name == *ty).expect("exhaustive: the arms name a known enum");
            let taken =
                |v: &str| named.iter().any(|p| matches!(p, Pattern::Variant { variant, .. } if variant.as_str() == v));
            Pattern::Or(
                e.variants
                    .iter()
                    .filter(|v| !taken(v.name.as_str()))
                    .map(|v| Pattern::Variant {
                        ty: ty.clone(),
                        variant: v.name.clone(),
                        bind: match &v.fields {
                            VariantFields::Unit => VariantBind::Unit,
                            VariantFields::Tuple(tys) => {
                                VariantBind::Tuple(tys.iter().map(|_| Pattern::Wildcard).collect())
                            }
                            VariantFields::Struct(_) => VariantBind::Struct(Vec::new()),
                        },
                    })
                    .collect(),
            )
        }
        // Integers and strings: `_` stays the final `else`.
        Some(p) if p.is_lit_case() => Pattern::Wildcard,
        _ => unreachable!("exhaustive: `_` follows an arm naming a case"),
    }
}
