//! Tuple patterns in `let`, closure parameters, and `for` variables: `mut`
//! and `_` elements, an annotation, left-to-right evaluation with overflow,
//! `&(a, b)` over `iter()`, and `break` / `continue` in the loop.

use crate::support;

purecrate_canon::fixture!(mod destructure = "fixtures/destructure.rs");

#[test]
fn generated_destructuring_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for x in [0, 1, 2, 7, u32::MAX - 1, u32::MAX] {
            cases.push(case!(destructure::swap(x)));
            cases.push(case!(destructure::bump(x)));
            cases.push(case!(destructure::first(x)));
            for k in [0, 1, 3, u32::MAX] {
                cases.push(case!(destructure::scaled(x, k)));
            }
        }
        for (a, b) in [(0, 0), (1, 2), (15, 15), (16, 15), (255, 0), (0, 255), (254, 0)] {
            cases.push(case!(destructure::both(a, b)));
        }
        for xs in
            [vec![], vec![(2, 3), (4, 5)], vec![(2, 3), (0, 5), (9, 9)], vec![(2, 0), (4, 5)], vec![(65_536, 65_536)]]
        {
            cases.push(case!(destructure::sum_pairs(&xs)));
        }
        for xs in [vec![], vec![(1, 2), (255, 255)], vec![(0, 9); 3]] {
            cases.push(case!(destructure::sum_refs(xs.clone())));
        }
        for o in [None, Some((1, 2)), Some((u32::MAX, 1))] {
            cases.push(case!(destructure::add(o)));
        }
        cases
    });
    support::assert_equivalent("destructure", destructure::SOURCE, &cases);
}
