//! `??` beside `?:`, `||`, and `&&` is parenthesized (JS rejects it bare
//! beside `||` / `&&`, and `?:` would take `??` as its test); a comment
//! above an operand leaves its grouping as it was; a tuple read in part is
//! not taken for the whole.

use crate::support;

purecrate_canon::fixture!(mod precedence = "fixtures/precedence.rs");

#[test]
fn generated_precedence_matches_rust() {
    support::equivalence("precedence", precedence::SOURCE, |cases| {
        for c in [false, true] {
            for o in [None, Some(0i32), Some(5)] {
                cases.push(case!(precedence::coalesce_choice(o, c)));
                cases.push(case!(precedence::mapped_is_none(o)));
                cases.push(case!(precedence::not_mapped(o)));
                for p in [None, Some(3i32)] {
                    cases.push(case!(precedence::same_presence(o, p)));
                }
                let r = match o {
                    Some(v) => Ok(v),
                    None => Err(1i32),
                };
                cases.push(case!(precedence::ok_is_some(r)));
            }
            for a in [false, true] {
                for b in [false, true] {
                    for o in [None, Some(false), Some(true)] {
                        cases.push(case!(precedence::coalesce_or(o, a, b)));
                        cases.push(case!(precedence::or_coalesce(c, o, a)));
                    }
                    cases.push(case!(precedence::and_commented(c, a, b)));
                }
            }
        }
        grid!(cases, precedence::first_two; x in [None, Some((1i32, 2i32, 3i32))]);
        grid!(cases, precedence::len_or_empty; o in [None, Some(vec![1u8, 2])]);
    });
}
