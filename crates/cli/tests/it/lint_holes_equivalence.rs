//! Shapes whose output oxlint refused, against Rust.

use crate::support;

purecrate_canon::fixture!(mod lint_holes = "fixtures/lint_holes.rs");

#[test]
fn lint_holes_match_rust() {
    support::equivalence("lint_holes", lint_holes::SOURCE, |cases| {
        for (a, b) in [(5, 0), (5, 3), (i32::MAX, 1), (i32::MIN, -1), (-7, 9)] {
            for c in [true, false] {
                cases.push(case!(lint_holes::unread_tests(a, b, c)));
                cases.push(case!(lint_holes::reads_itself(a, b, c)));
                cases.push(case!(lint_holes::sides_run(a, b, c)));
                cases.push(case!(lint_holes::turned_round(a, c)));
                cases.push(case!(lint_holes::else_guard(a, c)));
                for o in [None, Some(4)] {
                    for r in [Ok(2), Err(3)] {
                        cases.push(case!(lint_holes::keeps_itself(a, o, r, c)));
                    }
                }
            }
            cases.push(case!(lint_holes::emptied_loop(a, b)));
            cases.push(case!(lint_holes::param_unread(a, b)));
            for r in [Ok(2), Err(3)] {
                cases.push(case!(lint_holes::decided_return(r, a)));
                cases.push(case!(lint_holes::unread_try(a, b, r)));
            }
        }
    });
}
