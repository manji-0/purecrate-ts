//! `map` and `filter` over a `Vec`, a string's chars and bytes, and a
//! split agree with Rust, including which panic comes first.

use crate::support;

purecrate_canon::fixture!(mod adapters = "fixtures/adapters.rs");

const SOURCE: &str = adapters::SOURCE;

#[test]
fn adapters_match_rust() {
    let lists: Vec<Vec<u32>> = vec![
        vec![],
        vec![1, 2, 3],
        vec![0, 1, 2],
        vec![2, 0, 1],
        vec![3, 7, 0, 9],
        vec![u32::MAX / 2, u32::MAX / 2, 3],
        vec![u32::MAX, 1],
        vec![u32::MAX / 4, u32::MAX / 4, u32::MAX / 4, u32::MAX],
    ];
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for xs in &lists {
            cases.push(case!(adapters::doubled_sum(xs.clone())));
            cases.push(case!(adapters::doubled(xs.clone())));
            cases.push(case!(adapters::quotients(xs.clone(), 12u32)));
            cases.push(case!(adapters::kept(xs.clone())));
            cases.push(case!(adapters::copy(xs.clone())));
            cases.push(case!(adapters::first_big(xs.clone())));
            cases.push(case!(adapters::all_small(xs.clone())));
        }
        for s in ["", "a1b2", "é9z", "1234567890", "99999", "  two  words ", "x"] {
            cases.push(case!(adapters::digits(s)));
            cases.push(case!(adapters::letters(s)));
            cases.push(case!(adapters::byte_sum(s)));
            cases.push(case!(adapters::words(s)));
        }
        cases
    });
    support::assert_equivalent("adapters", SOURCE, &cases);
}
