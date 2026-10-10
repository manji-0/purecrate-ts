//! `Vec::remove` and `v[i] = x` match Rust, including the panics at and past
//! the end and the order an assignment evaluates in, and write the local's
//! own array, not the caller's.

use crate::support;

purecrate_canon::fixture!(mod vec_edit = "fixtures/vec_edit.rs");

#[test]
fn vec_edit_matches_rust() {
    support::equivalence("vec_edit", vec_edit::SOURCE, |cases| {
        for xs in [vec![], vec![1u32], vec![1, 2, 3]] {
            for at in [vec![], vec![0usize], vec![1], vec![2], vec![3], vec![0, 0], vec![2, 0, 0]] {
                cases.push(case!(vec_edit::removed(xs.clone(), at.clone())));
                cases.push(case!(vec_edit::written(xs.clone(), at, 7)));
            }
            cases.push(case!(vec_edit::untouched(xs.clone())));
            cases.push(case!(vec_edit::drained(xs.clone())));
        }
        for (n, at, by) in [(0usize, vec![], 1u8), (3, vec![0, 2, 2], 1), (2, vec![1, 1], 200), (2, vec![2], 1)] {
            cases.push(case!(vec_edit::counted(n, at, by)));
        }
        for (at, j) in [(0usize, 0usize), (2, 1), (3, 0), (0, 2), (3, 2)] {
            cases.push(case!(vec_edit::ordered(at, j)));
            cases.push(case!(vec_edit::stepped(at, j)));
        }
        for xs in [vec![], vec![1i32, 2, 1, 1, 3, 1], vec![1, 1, 1]] {
            cases.push(case!(vec_edit::without(xs, 1)));
        }
        for ws in [vec![], vec!["a", "a", "b", "b", "b", "a"], vec!["é", "é"]] {
            let ws: Vec<String> = ws.into_iter().map(String::from).collect();
            cases.push(case!(vec_edit::marked(ws)));
        }
    });
}
