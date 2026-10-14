//! `BTreeMap<String, V>` and `BTreeSet<String>`: key order by code point
//! (U+FFFD below a supplementary-plane key), maps built in reverse order,
//! a key given twice, `__proto__` as a key, the caller's map left as it
//! was, overflow at the first pair in key order, and derived `==`.

use std::collections::{BTreeMap, BTreeSet};

use crate::support;

purecrate_canon::fixture!(mod btree = "fixtures/btree.rs");

fn texts() -> Vec<&'static str> {
    vec!["", "a", "b", "\u{fffd}", "😀", "__proto__", "é", "Z", "a b", "b a b"]
}

#[test]
fn generated_btree_matches_rust() {
    support::equivalence("btree", btree::SOURCE, |cases| {
        for s in ["", "a b a", "b a  b c", "😀 \u{fffd} 😀", "__proto__ constructor toString __proto__"] {
            cases.push(case!(btree::count_words(s)));
        }
        let ints: Vec<Vec<(&str, i32)>> = vec![
            vec![],
            vec![("😀", 1), ("\u{fffd}", 2), ("a", 3)],
            vec![("b", i32::MAX), ("a", 1)],
            vec![("z", 5), ("__proto__", -1), ("é", 7)],
        ];
        for pairs in ints {
            // Built in reverse, so insertion order is not key order.
            let m: BTreeMap<String, i32> = pairs.iter().rev().map(|(k, v)| (k.to_string(), *v)).collect();
            cases.push(case!(btree::keys(&m)));
            cases.push(case!(btree::values(&m)));
            cases.push(case!(btree::total(&m)));
        }
        let bytes: BTreeMap<String, u8> =
            [("b", 2u8), ("a", 1), ("😀", 9)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        cases.push(case!(btree::joined(&bytes)));
        let m: BTreeMap<String, i64> =
            [("b", 2i64), ("a", 1), ("\u{fffd}", 3)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        for k in texts() {
            cases.push(case!(btree::lookup(&m, k)));
            cases.push(case!(btree::edited(&m, k.to_string(), -5)));
        }
        cases.push(case!(btree::untouched(&m)));
        let empty: BTreeMap<String, i64> = BTreeMap::new();
        cases.push(case!(btree::lookup(&empty, "a")));
        let s: BTreeSet<String> = ["a", "😀", "\u{fffd}"].iter().map(|k| k.to_string()).collect();
        for k in texts() {
            cases.push(case!(btree::set_ops(&s, k.to_string())));
        }
        let lists: Vec<Vec<String>> = vec![
            vec![],
            vec!["b".into(), "a".into(), "b".into()],
            vec!["😀".into(), "\u{fffd}".into(), "__proto__".into(), "".into()],
        ];
        for words in lists {
            cases.push(case!(btree::from_pairs(words.clone())));
            cases.push(case!(btree::from_keys(words.clone())));
        }
        let a: BTreeMap<String, u8> = [("x", 1u8), ("y", 2)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let b: BTreeMap<String, u8> = [("y", 2u8), ("x", 1)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        let c: BTreeMap<String, u8> = [("x", 1u8), ("y", 3)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        for (p, q) in [(&a, &b), (&a, &c), (&a, &empty_u8()), (&c, &b)] {
            cases.push(case!(btree::same(p, q)));
        }
        let stock = |name: &str, counts: &[(&str, u32)], tags: &[&str]| btree::Stock {
            name: name.to_string(),
            counts: counts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
        };
        let s1 = stock("s", &[("apple", 3), ("pear", 1)], &["fruit"]);
        let s2 = stock("s", &[("pear", 1), ("apple", 3)], &["fruit"]);
        let s3 = stock("s", &[("apple", 3)], &["fruit", "green"]);
        for (p, q) in [(&s1, &s2), (&s1, &s3)] {
            cases.push(case!(btree::same_stock(p, q)));
        }
        for (item, n) in [("apple", 1u32), ("kiwi", 2), ("apple", u32::MAX)] {
            cases.push(case!(btree::restock(&s1, item.to_string(), n)));
        }
        let counts: BTreeMap<String, u32> =
            [("b", 5u32), ("a", 5), ("c", 1)].iter().map(|(k, v)| (k.to_string(), *v)).collect();
        cases.push(case!(btree::largest(&counts)));
        cases.push(case!(btree::largest(&BTreeMap::<String, u32>::new())));
    });
}

fn empty_u8() -> BTreeMap<String, u8> {
    BTreeMap::new()
}
