//! `Vec::insert` matches Rust, including the panic past the end, and grows
//! the local's own array, not the caller's.

use crate::support;

purecrate_canon::fixture!(mod vec_insert = "fixtures/vec_insert.rs");

#[test]
fn vec_insert_matches_rust() {
    support::equivalence("vec_insert", vec_insert::SOURCE, |cases| {
        for xs in [vec![], vec![1u32], vec![1, 2, 3]] {
            for at in [vec![], vec![0usize], vec![1], vec![3], vec![4], vec![0, 0, 2], vec![2, 5]] {
                cases.push(case!(vec_insert::inserted(xs.clone(), at, 7)));
            }
            cases.push(case!(vec_insert::untouched(xs.clone())));
            cases.push(case!(vec_insert::doubled(xs.clone())));
        }
        for xs in [vec![], vec![3i32, 1, 2], vec![5, 5, -1, 0, 5, i32::MIN, i32::MAX]] {
            cases.push(case!(vec_insert::sorted(xs)));
        }
        for at in [vec![], vec![0u32], vec![1], vec![u32::MAX]] {
            cases.push(case!(vec_insert::placed(vec![65, 66, 67, 0x1F600], at)));
        }
        for ws in [vec![], vec!["a".to_string()], vec!["é".to_string(), "😀".to_string(), String::new()]] {
            cases.push(case!(vec_insert::mirrored(ws)));
        }
    });
}
