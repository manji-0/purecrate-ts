//! `??` beside `?:`, `||`, and `&&` is parenthesized (JS rejects it bare
//! beside `||` / `&&`, and `?:` would take `??` as its test); a comment
//! above an operand leaves its grouping as it was; a tuple read in part is
//! not taken for the whole.

use crate::support;

purecrate_canon::fixture!(mod precedence = "fixtures/precedence.rs");

const SOURCE: &str = precedence::SOURCE;

#[test]
fn generated_precedence_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for c in [false, true] {
            for o in [None, Some(0i32), Some(5)] {
                cases.push(case!(precedence::coalesce_choice(o, c)));
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
        for x in [None, Some((1i32, 2i32, 3i32))] {
            cases.push(case!(precedence::first_two(x)));
        }
        cases
    });
    support::assert_equivalent("precedence", SOURCE, &cases);
}
