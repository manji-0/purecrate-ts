//! Sorting and picking match Rust: stability on ties, the last of the
//! greatest and the first of the least, and strings and `char`s ordered by
//! code point.

use crate::support;

purecrate_canon::fixture!(mod sort_pick = "fixtures/sort_pick.rs");

use sort_pick::Ranked;

fn ranked(rows: &[(&str, u32, u32)]) -> Vec<Ranked> {
    rows.iter().map(|&(n, q, s)| Ranked { name: n.to_string(), q, specificity: s }).collect()
}

fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

#[test]
fn sort_and_pick_match_rust() {
    support::equivalence("sort_pick", sort_pick::SOURCE, |cases| {
        for xs in [vec![], vec![3i32, -1, 3, i32::MIN, 0, i32::MAX, 2], vec![5, 5, 5]] {
            cases.push(case!(sort_pick::sorted(xs)));
        }
        for xs in [&["b", "a", "\u{e000}", "😀", "", "é", "e"][..], &[][..]] {
            cases.push(case!(sort_pick::sorted_text(strings(xs))));
            cases.push(case!(sort_pick::by_length(strings(xs))));
        }
        let tables: [&[(&str, u32, u32)]; 4] = [
            &[],
            &[("a", 1000, 1), ("b", 500, 2), ("c", 1000, 1), ("d", 1000, 3), ("e", 0, 0), ("f", 0, 9)],
            &[("x", 7, 7)],
            &[("p", 0, 0), ("q", 0, 0), ("r", 0, 0)],
        ];
        for rows in tables {
            cases.push(case!(sort_pick::best_first(ranked(rows))));
            cases.push(case!(sort_pick::picked(ranked(rows))));
        }
        for xs in [vec![], vec![3u8, 9, 9, 0, 0, 255]] {
            cases.push(case!(sort_pick::extremes(xs)));
        }
        for s in ["", "abc", "\u{e000}😀a", "zé"] {
            cases.push(case!(sort_pick::text_extremes(s)));
        }
        for xs in [vec![], vec![2i64, -5, -5, i64::MAX]] {
            cases.push(case!(sort_pick::least_by(xs)));
        }
    });
}
