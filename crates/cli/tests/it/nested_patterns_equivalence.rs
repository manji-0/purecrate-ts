//! Patterns inside patterns, lowered into the decision tree of
//! `check::tuple`: literals, ranges, variants, and `Some`/`Ok`/`Err` in a
//! variant's fields and in payloads, beside bindings, before guards and a
//! last `_`, in a tuple `match`, and in `matches!`; a side of `|` that tests
//! inside its case; a tuple with a literal inside a payload or a tuple.

use crate::support;

purecrate_canon::fixture!(mod nested_patterns = "fixtures/nested_patterns.rs");

use nested_patterns::{Login, Method};

const SOURCE: &str = nested_patterns::SOURCE;

fn logins() -> Vec<Login> {
    let mut out = vec![Login::Started];
    for verified in [true, false] {
        for attempts in [0u8, 1, 4, 5, 255] {
            out.push(Login::PasswordChecked { verified, attempts });
        }
    }
    for m in [Method::Card, Method::Bank] {
        for amount in [None, Some(0u32), Some(1), Some(9), Some(10), Some(101), Some(1000), Some(u32::MAX)] {
            out.push(Login::Paid(m, amount));
        }
    }
    out
}

#[test]
fn generated_nested_patterns_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for l in logins() {
            cases.push(case!(nested_patterns::next(l.clone())));
            cases.push(case!(nested_patterns::card_amount(l.clone())));
            cases.push(case!(nested_patterns::guarded(&l)));
            cases.push(case!(nested_patterns::is_verified(&l)));
            cases.push(case!(nested_patterns::is_big_card(&l)));
            cases.push(case!(nested_patterns::either(l.clone())));
            cases.push(case!(nested_patterns::either_guarded(&l)));
            for a in [None, Some(Method::Card), Some(Method::Bank)] {
                cases.push(case!(nested_patterns::pair(a, l.clone())));
            }
        }
        for r in [Ok(None), Ok(Some(Method::Card)), Ok(Some(Method::Bank)), Err(0u8), Err(1), Err(255)] {
            cases.push(case!(nested_patterns::deep(r)));
        }
        for x in [None, Some(0i64), Some(1), Some(2), Some(3), Some(-1), Some(-5), Some(-6), Some(i64::MAX / 2)] {
            cases.push(case!(nested_patterns::small(x)));
        }
        for s in [None, Some((0u32, 0u32)), Some((0, 5)), Some((3, 0)), Some((3, 4)), Some((u32::MAX, 0))] {
            cases.push(case!(nested_patterns::split(s)));
        }
        for a in [0u32, 1, 9] {
            for b in [true, false] {
                for n in [0u32, 2, 3] {
                    cases.push(case!(nested_patterns::quad((a, (b, n)))));
                }
            }
        }
        for card in [true, false] {
            for amount in [0u32, 1, 100, u32::MAX] {
                cases.push(case!(nested_patterns::free_card(card, amount)));
                cases.push(case!(nested_patterns::fee(card, amount)));
            }
        }
        for c in [None, Some('a'), Some('z'), Some('A'), Some('é')] {
            for s in [None, Some("yes"), Some("no"), Some("")] {
                cases.push(case!(nested_patterns::text(c, s)));
            }
        }
        cases
    });
    support::assert_equivalent("nested_patterns", SOURCE, &cases);
}
