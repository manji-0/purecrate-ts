//! `Vec` indexing and `len`: in-range reads and a walk by index agree with
//! Rust, and an out-of-range index panics on both sides with the same message.

use crate::support;

purecrate_canon::fixture!(mod vecs = "fixtures/vec.rs");

#[test]
fn generated_vec_reads_match_rust() {
    let samples = [vec![], vec![0], vec![1, 2], vec![-3, 4, 5, 6]];
    let cases = support::cases(|cases| {
        for xs in &samples {
            cases.push(case!(vecs::len(xs.clone())));
            cases.push(case!(vecs::sum_from(xs.as_slice(), 0usize)));
            if xs.len() > 1 {
                cases.push(case!(vecs::at(xs.clone(), 0usize)));
                cases.push(case!(vecs::at(xs.clone(), 1usize)));
                cases.push(case!(vecs::second(xs.clone())));
            } else if xs.len() == 1 {
                cases.push(case!(vecs::at(xs.clone(), 0usize)));
            }
        }
        cases.push(case!(vecs::at(vec![1i32, 2, 3], 5usize)));
        cases.push(case!(vecs::at(vec![0i32; 0], 0usize)));
    });
    // The TS throw must say what Rust's panic says.
    let out_of_range = "panic(index out of bounds: the len is 3 but the index is 5)";
    assert!(cases.iter().any(|c| c.rust == out_of_range), "an out-of-range case");
    support::assert_equivalent("vecs", vecs::SOURCE, &cases);
}
