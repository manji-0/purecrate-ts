//! `BTreeMap<K, V>` and `BTreeSet<K>` with keys other than `String`
//! (design/01 §7.26): integer keys `9` and `10`, negative keys, `bigint`
//! keys past 2^53, `i128` and `u128` at their bounds, `char`s outside the
//! BMP, `bool`, `Uuid`, and newtype keys, maps built in reverse order, a key
//! given twice, the caller's map left as it was, overflow at the first value
//! in key order, and derived `==`.

use std::collections::{BTreeMap, BTreeSet};

use crate::support;

purecrate_canon::fixture!(mod keys = "fixtures/btree_keys.rs");

/// Built in reverse, so insertion order is not key order.
fn map<K: Ord + Clone, V: Clone>(pairs: &[(K, V)]) -> BTreeMap<K, V> {
    pairs.iter().rev().cloned().collect()
}

fn uuid(n: u128) -> uuid::Uuid {
    uuid::Uuid::from_u128(n)
}

#[test]
fn generated_btree_keys_match_rust() {
    support::equivalence("btree_keys", keys::SOURCE, |cases| {
        let ints: Vec<Vec<(i32, u8)>> = vec![
            vec![],
            vec![(10, 1), (9, 2), (-1, 3), (-2, 4), (100, 5), (0, 6)],
            vec![(i32::MAX, 1), (i32::MIN, 2), (-10, 3), (2, 4)],
        ];
        for pairs in &ints {
            let m = map(pairs);
            cases.push(case!(keys::keys(&m)));
            cases.push(case!(keys::values(&m)));
        }
        let small: Vec<Vec<(i64, i8)>> = vec![
            vec![],
            vec![(10, 100), (9, 27), (-1, 1)],
            // In key order -5, 3, 9007199254740993: the second sum overflows.
            vec![(9_007_199_254_740_993, -128), (3, 100), (-5, 100)],
            vec![(i64::MIN, -100), (i64::MAX, -100), (0, 50)],
        ];
        for pairs in &small {
            cases.push(case!(keys::total(&map(pairs))));
        }
        let big = map(&[
            (u64::MAX, 1u8),
            (9_007_199_254_740_993, 2),
            (9_007_199_254_740_992, 3),
            (10, 4),
            (9, 5),
            (0, 6),
        ]);
        cases.push(case!(keys::joined(&big)));
        cases.push(case!(keys::joined(&BTreeMap::<u64, u8>::new())));
        let wide = map(&[(i128::MAX, 1u128), (i128::MIN, u128::MAX), (-1, 2), (10, 3), (9, 4)]);
        let wide_set: BTreeSet<u128> = [u128::MAX, 0, 10, 9, 1 << 64, (1 << 64) - 1].into_iter().collect();
        cases.push(case!(keys::wide(&wide, &wide_set)));
        let texts = map(&[(9_007_199_254_740_993u64, "a".to_string()), (10, "😀".to_string()), (9, String::new())]);
        for k in [9u64, 10, 11, 9_007_199_254_740_993, 9_007_199_254_740_992, u64::MAX, 0] {
            cases.push(case!(keys::lookup(&texts, k)));
        }
        cases.push(case!(keys::lookup(&BTreeMap::<u64, String>::new(), 0)));
        let shorts = map(&[(-1i16, 1i64), (10, 2), (9, 3), (i16::MIN, 4)]);
        for (k, v) in [(-1i16, 5i64), (10, -5), (i16::MAX, 0), (-2, i64::MIN)] {
            cases.push(case!(keys::edited(&shorts, k, v)));
        }
        cases.push(case!(keys::edited(&BTreeMap::<i16, i64>::new(), -1, 1)));
        cases.push(case!(keys::untouched(&map(&[(9usize, false), (10, true)]))));
        let letters: BTreeSet<char> = ['a', '😀', '\u{fffd}', 'é', 'Z'].into_iter().collect();
        for c in ['a', 'b', '😀', '\u{10ffff}', '\u{e000}', '\u{ffff}', '\0'] {
            cases.push(case!(keys::set_ops(&letters, c)));
        }
        for s in ["", "banana", "😀\u{fffd}\u{e000}a😀", "\u{10ffff}\u{10000}\u{ffff}"] {
            cases.push(case!(keys::letters(s)));
        }
        for xs in [vec![], vec![10u8, 9, 10, 255, 0, 9, 10], vec![1, 1, 1]] {
            cases.push(case!(keys::counts(xs)));
        }
        for xs in [vec![], vec![(10i8, 1u32), (9, 2), (-1, 3), (10, 4), (-128, 5)], vec![(0, 0), (0, 1)]] {
            cases.push(case!(keys::from_pairs(xs)));
        }
        for xs in [vec![], vec![10u32, 9, 100, 9, u32::MAX, 0], vec![2, 1]] {
            cases.push(case!(keys::p06_btreeset(xs)));
        }
        for pairs in [vec![], vec![(true, 1u32), (false, 2)], vec![(true, 3)], vec![(false, 4)]] {
            cases.push(case!(keys::flags(&map(&pairs))));
        }
        let id_map = map(&[(uuid(u128::MAX), 1u8), (uuid(0), 2), (uuid(0xa0 << 120), 3), (uuid(9), 4), (uuid(10), 5)]);
        for k in [uuid(0), uuid(10), uuid(11), uuid(u128::MAX)] {
            cases.push(case!(keys::ids(&id_map, k)));
        }
        let a = map(&[(10u32, 1u8), (9, 2)]);
        let b: BTreeMap<u32, u8> = [(9u32, 2u8), (10, 1)].into_iter().collect();
        let c = map(&[(10u32, 1u8), (9, 3)]);
        let d = map(&[(10u32, 1u8)]);
        for (p, q) in [(&a, &b), (&a, &c), (&a, &d), (&d, &BTreeMap::<u32, u8>::new())] {
            cases.push(case!(keys::same(p, q)));
        }
        let s1: BTreeSet<i64> = [-1, 10, 9].into_iter().collect();
        let s2: BTreeSet<i64> = [9, -1, 10].into_iter().collect();
        let s3: BTreeSet<i64> = [9, 10].into_iter().collect();
        cases.push(case!(keys::same_sets(&s1, &s2)));
        cases.push(case!(keys::same_sets(&s1, &s3)));
        let ledger = |owner: u64, balances: &[(u64, i64)], seen: &[char]| keys::Ledger {
            owner: keys::UserId(owner),
            balances: balances.iter().rev().map(|(k, v)| (keys::UserId(*k), *v)).collect(),
            seen: seen.iter().copied().collect(),
        };
        let l1 = ledger(1, &[(10, 5), (9, 7), (9_007_199_254_740_993, -1), (0, 0)], &['😀', 'a']);
        let l2 = ledger(1, &[(9_007_199_254_740_993, -1), (0, 0), (9, 7), (10, 5)], &['a', '😀']);
        let l3 = ledger(1, &[(10, 5), (9, 7)], &['a']);
        for (p, q) in [(&l1, &l2), (&l1, &l3)] {
            cases.push(case!(keys::same_ledger(p, q)));
        }
        for (to, n) in [(9u64, 1i64), (11, 2), (u64::MAX, -3), (9, i64::MAX)] {
            cases.push(case!(keys::pay(&l1, keys::UserId(to), n)));
        }
        cases.push(case!(keys::owners(&l1)));
        cases.push(case!(keys::richest(&l1)));
        cases.push(case!(keys::richest(&ledger(0, &[], &[]))));
        let codes: BTreeMap<keys::Code, u8> = ["b", "😀", "\u{fffd}", "a", "", "10", "9"]
            .iter()
            .enumerate()
            .rev()
            .map(|(i, s)| (keys::code_of(s.to_string()), i as u8))
            .collect();
        for k in ["a", "😀", "z", "10"] {
            cases.push(case!(keys::by_code(&codes, keys::code_of(k.to_string()))));
        }
        for words in [vec![], vec!["b", "a", "b", "😀", "\u{fffd}"], vec!["10", "9", "-1"]] {
            let words: Vec<String> = words.into_iter().map(String::from).collect();
            cases.push(case!(keys::code_set(words)));
        }
        for cells in [vec![], vec![(10u8, 9u8, 'x'), (9, 10, 'y'), (10, 10, '😀'), (10, 9, 'z')]] {
            cases.push(case!(keys::grid(cells)));
        }
    });
}
