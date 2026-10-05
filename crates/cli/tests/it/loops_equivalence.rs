//! `for i in a..b` (design/02 §2): empty and negative ranges, `i64`,
//! early `return`, `?` in the body, nesting, a closure over the variable,
//! bounds read once, shadowing, and panics in the bounds and in the body;
//! and a `while` on a flag its body sets.

use crate::support;

purecrate_canon::fixture!(mod loops = "fixtures/loops.rs");

#[test]
fn for_ranges_match_rust() {
    support::equivalence("loops", loops::SOURCE, |cases| {
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
        grid!(cases, [loops::table, loops::bound_once, loops::shadow]; n in [0u32, 1, 5, 100, 3000]);
        grid!(cases, loops::captured; n in [0i32, 4, -2]);
        grid!(cases, [loops::steps_until_past, loops::steps_while_going]; n in [0u32, 1, 4, 1000]);
        for (a, b) in [(10i32, 2i32), (1, 0), (i32::MIN, -1), (10000, 1)] {
            cases.push(case!(loops::bounds_panic(a, b)));
        }
    });
}
