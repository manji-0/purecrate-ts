//! Std bounds that carry a dictionary against Rust: generic functions with
//! `T: Ord`, `T: PartialOrd`, `T: Display`, and `T: Default` called at the
//! integers (64-bit ones as `bigint`), `bool`, `char`, `String` and `&str`,
//! a tuple, a `Vec`, a crate enum, a hand-written `Ord` whose ties show
//! which of two equals `max`, `min`, the iterator's `max` / `min`, a `>`
//! loop, and a stable `sort` keep, and one whose `cmp` overflows; a generic
//! struct and a recursive generic enum deriving `Ord` and `Default` with
//! their parameters' dictionaries; `clamp` panicking where its bounds cross
//! through a dictionary; a dictionary forwarded only to a recursive call;
//! `Display` of non-ASCII text and of a crate type, in a template with
//! positional and named arguments; and empty input throughout.

use crate::support;

purecrate_canon::fixture!(mod std_bounds = "fixtures/std_bounds.rs");

use std_bounds::{Bumped, Holder, Level, Money, Ranked, Tagged, Tree};

fn tagged(key: u8, tag: &str) -> Tagged {
    Tagged { key, tag: tag.to_string() }
}

fn ranked<T>(rank: u8, value: T, rest: Vec<T>) -> Ranked<T> {
    Ranked { rank, value, rest }
}

fn money(cents: i64, unit: &str) -> Money {
    Money { cents, unit: unit.to_string() }
}

fn trees() -> Vec<Tree<i64>> {
    let leaf = || Tree::Leaf;
    let node = |l: Tree<i64>, n: i64, r: Tree<i64>| Tree::Node(Box::new(l), n, Box::new(r));
    vec![
        leaf(),
        node(leaf(), i64::MIN, leaf()),
        node(leaf(), 0, leaf()),
        node(leaf(), 0, node(leaf(), 1, leaf())),
        node(node(leaf(), -1, leaf()), 0, leaf()),
        node(leaf(), i64::MAX, leaf()),
    ]
}

#[test]
fn std_bounds_match_rust() {
    support::equivalence("std_bounds", std_bounds::SOURCE, |cases| {
        for xs in [vec![], vec![0i32], vec![3, -1, 3, i32::MIN, i32::MAX, 0]] {
            for a in [i32::MIN, -1, 0, 1] {
                for b in [-1i32, 0] {
                    cases.push(case!(std_bounds::ints(xs.clone(), a, b)));
                }
            }
        }
        for xs in [vec![], vec![u64::MAX, 0, 1 << 53, (1 << 53) + 1, 7]] {
            for (x, lo, hi) in [(5u64, 1u64, 9u64), (0, 1, 9), (u64::MAX, 0, u64::MAX - 1), (3, 3, 3), (4, 9, 1)] {
                cases.push(case!(std_bounds::wide(xs.clone(), x, lo, hi)));
            }
        }
        let texts: Vec<String> = ["", "a", "B", "é", "\u{e000}", "😀", "ab", "a"].iter().map(|s| s.to_string()).collect();
        cases.push(case!(std_bounds::texts(Vec::<String>::new(), String::new(), String::new())));
        for a in &texts {
            for b in ["", "é", "😀", "a"] {
                cases.push(case!(std_bounds::texts(texts.clone(), a.clone(), b.to_string())));
            }
        }
        grid!(cases, std_bounds::chars; a in ['a', 'é', '😀', '\u{e000}', '\0'], b in ['é', '\u{ffff}', '😀']);
        for xs in [vec![], vec![true, false, true], vec![false]] {
            cases.push(case!(std_bounds::bools(xs.clone())));
        }
        let levels = [Level::Low, Level::Mid, Level::High];
        for a in levels {
            for b in levels {
                cases.push(case!(std_bounds::levels(vec![b, a, Level::Mid], a, b)));
            }
        }
        cases.push(case!(std_bounds::levels(Vec::<Level>::new(), Level::High, Level::Low)));
        // Two of one key are equal: which one comes back shows std's rule.
        cases.push(case!(std_bounds::tagged_ones(Vec::<Tagged>::new())));
        cases.push(case!(std_bounds::tagged_ones(vec![tagged(1, "a"), tagged(2, "b"), tagged(2, "c"), tagged(1, "d")])));
        cases.push(case!(std_bounds::tagged_ones(vec![tagged(0, "é"), tagged(0, "😀"), tagged(0, "")])));
        cases.push(case!(std_bounds::bumped_ones(vec![Bumped(1), Bumped(254)])));
        cases.push(case!(std_bounds::bumped_ones(vec![Bumped(1), Bumped(255)])));
        cases.push(case!(std_bounds::bumped_ones(vec![Bumped(255)])));
        cases.push(case!(std_bounds::pairs(vec![(1, 'b'), (0, 'z'), (1, 'a'), (255, '\0')])));
        cases.push(case!(std_bounds::pairs(Vec::<(u8, char)>::new())));
        cases.push(case!(std_bounds::lists(vec![vec![1, 2], vec![], vec![1], vec![1, 2, 0], vec![-1, 9]])));
        cases.push(case!(std_bounds::lists(Vec::<Vec<i32>>::new())));
        let rs = vec![
            ranked(0, 5u32, vec![]),
            ranked(0, 5, vec![0]),
            ranked(0, 4, vec![9, 9]),
            ranked(1, 0, vec![]),
            ranked(0, u32::MAX, vec![]),
        ];
        for a in &rs {
            for b in &rs {
                cases.push(case!(std_bounds::ranked_ones(rs.clone(), a.clone(), b.clone())));
            }
        }
        cases.push(case!(std_bounds::ranked_ones(Vec::<Ranked<u32>>::new(), rs[0].clone(), rs[0].clone())));
        cases.push(case!(std_bounds::ranked_texts(vec![
            ranked(0, "é".to_string(), vec![]),
            ranked(0, "e".to_string(), vec!["😀".to_string()]),
            ranked(0, "e".to_string(), vec!["\u{e000}".to_string()]),
            ranked(0, String::new(), vec![]),
        ])));
        for a in trees() {
            for b in trees() {
                cases.push(case!(std_bounds::trees(a.clone(), b.clone())));
            }
        }
        let holder = |rank: u8, value: u32, level: Level| Holder { ranked: ranked(rank, value, vec![value]), level };
        for a in [holder(0, 0, Level::High), holder(0, 1, Level::Low), Holder::default()] {
            for b in [holder(0, 0, Level::Low), holder(1, 0, Level::Low)] {
                cases.push(case!(std_bounds::holders(a.clone(), b)));
            }
        }
        cases.push(case!(std_bounds::keyed(vec![("b".to_string(), 1), ("é".to_string(), 2), ("é".to_string(), 3), ("a".to_string(), 4)])));
        cases.push(case!(std_bounds::keyed(Vec::<(String, u8)>::new())));
        for xs in [vec![], vec![Level::Low, Level::High, Level::Mid]] {
            for n in [0u32, 3] {
                cases.push(case!(std_bounds::counted(xs.clone(), n)));
            }
        }
        for items in [vec![], vec![i64::MIN, i64::MAX, 0]] {
            cases.push(case!(std_bounds::shelf_top(items.clone())));
        }
        for (n, s) in [(i32::MIN, ""), (0, "ünï😀"), (i32::MAX, "`${x}`\\")] {
            for (b, c) in [(true, '\0'), (false, '😀')] {
                cases.push(case!(std_bounds::shown(n, b, c, s.to_string(), money(i64::from(n), s))));
            }
        }
        for (n, s) in [(0u8, ""), (255, "é")] {
            cases.push(case!(std_bounds::shown_more(n, 7, s.to_string(), money(-150, "€"))));
        }
        // `n + 1` overflows before the template runs any `toString`.
        cases.push(case!(std_bounds::shown_more(1, u32::MAX, "x".to_string(), money(i64::MAX, "$"))));
        cases.push(case!(std_bounds::defaults()));
        cases.push(case!(std_bounds::crate_defaults()));
    });
}
