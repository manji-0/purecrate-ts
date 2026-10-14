//! Slice patterns, lowered into a decision tree on the length
//! (`check::tuple`): fixed lengths, `..` at the front, the back, and
//! between, `rest @ ..`, literal, variant, `Some`, tuple, and struct
//! elements, guards that overflow only where their length matched, a tuple
//! of slices, `Some` of a slice, `if let`, `let`-`else`, `matches!`, and
//! `while let`; on every length from empty to past the longest pattern.

use crate::support;

purecrate_canon::fixture!(mod slice_patterns = "fixtures/slice_patterns.rs");

use slice_patterns::{Item, Shape};

/// Every list of `0..=5` elements over `vals`, up to `cap` of them.
fn lists<T: Clone>(vals: &[T], cap: usize) -> Vec<Vec<T>> {
    let mut out: Vec<Vec<T>> = vec![vec![]];
    let mut layer: Vec<Vec<T>> = vec![vec![]];
    for _ in 0..5 {
        let mut next = Vec::new();
        for l in &layer {
            for v in vals {
                let mut l = l.clone();
                l.push(v.clone());
                next.push(l);
            }
        }
        out.extend(next.iter().take(cap).cloned());
        layer = next.into_iter().take(cap).collect();
    }
    out
}

#[test]
fn generated_slice_patterns_match_rust() {
    support::equivalence("slice_patterns", slice_patterns::SOURCE, |cases| {
        for xs in lists(&[0u32, 1, 3, 2_147_483_648, u32::MAX], 200) {
            let xs = xs.as_slice();
            cases.push(case!(slice_patterns::first(xs)));
            cases.push(case!(slice_patterns::last(xs)));
            cases.push(case!(slice_patterns::describe(xs)));
            cases.push(case!(slice_patterns::ends_and_middle(xs)));
            cases.push(case!(slice_patterns::tail(xs)));
            cases.push(case!(slice_patterns::init(xs)));
            cases.push(case!(slice_patterns::long_tail(xs)));
            cases.push(case!(slice_patterns::bumped(xs)));
            cases.push(case!(slice_patterns::owned(xs.to_vec())));
            cases.push(case!(slice_patterns::maybe(Some(xs.to_vec()))));
            cases.push(case!(slice_patterns::pair_if_let(xs)));
            cases.push(case!(slice_patterns::ends_let_else(xs)));
            cases.push(case!(slice_patterns::starts_matches(xs)));
            cases.push(case!(slice_patterns::long_matches(xs)));
            cases.push(case!(slice_patterns::guard_matches(xs)));
            cases.push(case!(slice_patterns::walk(xs)));
            for ys in [&[][..], &[6][..], &[3, 9][..], &[u32::MAX][..]] {
                cases.push(case!(slice_patterns::both(xs, ys)));
                cases.push(case!(slice_patterns::both(ys, xs)));
            }
        }
        cases.push(case!(slice_patterns::maybe(None::<Vec<u32>>)));
        let rows: [&[u32]; 4] = [&[], &[7], &[1, 2], &[u32::MAX, 3, 4]];
        for xss in lists(&rows, 120) {
            cases.push(case!(slice_patterns::nested(xss.as_slice())));
        }
        for xs in lists(&[0u8, 1, 2, 3, 255], 300) {
            cases.push(case!(slice_patterns::code(xs.as_slice())));
        }
        for xs in lists(&[i32::MIN, -1, 0, 6, 11, i32::MAX], 300) {
            cases.push(case!(slice_patterns::guarded(xs.as_slice())));
        }
        for xs in lists(&["", "é", "日本", "abc", "😀x"], 200) {
            cases.push(case!(slice_patterns::words(xs.as_slice())));
        }
        for xs in lists(&['a', 'z', 'A', 'é', '😀', '\u{ffff}'], 300) {
            cases.push(case!(slice_patterns::chars(xs.as_slice())));
            cases.push(case!(slice_patterns::palindrome(xs.as_slice())));
        }
        let shapes = [Shape::Circle(0), Shape::Circle(7), Shape::Square(u32::MAX), Shape::Square(1), Shape::Empty];
        for xs in lists(&shapes, 300) {
            cases.push(case!(slice_patterns::shapes(xs.as_slice())));
        }
        for xs in lists(&[None, Some(0u32), Some(5), Some(u32::MAX)], 300) {
            cases.push(case!(slice_patterns::options(xs.as_slice())));
        }
        for xs in lists(&[(0u32, true), (4, false), (u32::MAX, true)], 200) {
            cases.push(case!(slice_patterns::pairs(xs.as_slice())));
        }
        let items = [
            Item { name: String::from("pen"), qty: 0 },
            Item { name: String::from("paper"), qty: 4 },
            Item { name: String::from("日本語"), qty: u32::MAX },
            Item { name: String::from("ink"), qty: 2 },
        ];
        for xs in lists(&items, 200) {
            cases.push(case!(slice_patterns::items(xs.as_slice())));
        }
    });
}
