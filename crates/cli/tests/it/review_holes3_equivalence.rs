//! What the statements generator found once it drew blocks and shadowing,
//! against Rust.

use crate::support;

purecrate_canon::fixture!(mod review_holes3 = "fixtures/review_holes3.rs");

#[test]
fn review_holes3_match_rust() {
    support::equivalence("review_holes3", review_holes3::SOURCE, |cases| {
        for (a, c) in [(5, true), (5, false), (i32::MAX, false)] {
            cases.push(case!(review_holes3::block_in_branch(a, c)));
        }
        for (a, n) in [(5, 0), (5, 2)] {
            cases.push(case!(review_holes3::block_in_loop(a, n)));
        }
        for (a, b) in [(2, 3), (2, -3)] {
            cases.push(case!(review_holes3::block_in_value(a, b)));
        }
        for o in [None, Some(10)] {
            cases.push(case!(review_holes3::blocks_side_by_side(5, o)));
        }
        cases.push(case!(review_holes3::closure_in_value(4)));
        let results: [Result<i32, i32>; 3] = [Ok(1), Err(3), Err(i32::MAX)];
        for r in results {
            cases.push(case!(review_holes3::closure_in_arm(r)));
            cases.push(case!(review_holes3::decided_in_closure(r)));
            cases.push(case!(review_holes3::empty_arm(2, r)));
            for c in [true, false] {
                cases.push(case!(review_holes3::decided_try(c, r)));
                cases.push(case!(review_holes3::decides_failing(r, c)));
            }
            for b in [3, i32::MAX] {
                cases.push(case!(review_holes3::twice_tried(r, b)));
            }
        }
        for a in [3, i32::MAX] {
            cases.push(case!(review_holes3::failing_try(a)));
        }
    });
}
