//! `for i in a..b` (design/02 §2): empty and negative ranges, `i64`,
//! early `return`, `?` in the body, nesting, a closure over the variable,
//! bounds read once, shadowing, and panics in the bounds and in the body.

use crate::support;

purecrate_canon::fixture!(mod loops = "fixtures/loops.rs");

const SOURCE: &str = loops::SOURCE;

#[test]
fn for_ranges_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for (a, b) in [(0i32, 0i32), (0, 5), (5, 0), (-3, 3), (-5, -2), (0, 70000)] {
            cases.push(case!(loops::sum(a, b)));
            cases.push(case!(loops::all_checked(a, b)));
        }
        for (a, b) in [(0i64, 0i64), (-4, 4), (0, 3000000), (9223372036854775805, 9223372036854775807)] {
            cases.push(case!(loops::sum_big(a, b)));
        }
        for xs in [vec![], vec![1u8, 2], vec![3u8, 0, 0]] {
            cases.push(case!(loops::first_zero(xs.clone())));
        }
        for n in [0u32, 1, 5, 100, 3000] {
            cases.push(case!(loops::table(n)));
            cases.push(case!(loops::bound_once(n)));
            cases.push(case!(loops::shadow(n)));
        }
        for n in [0i32, 4, -2] {
            cases.push(case!(loops::captured(n)));
        }
        for (a, b) in [(10i32, 2i32), (1, 0), (i32::MIN, -1), (10000, 1)] {
            cases.push(case!(loops::bounds_panic(a, b)));
        }
        cases
    });
    support::assert_equivalent("loops", SOURCE, &cases);
}
