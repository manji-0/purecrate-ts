// `BTreeMap<String, V>` and `BTreeSet<String>`: built, read, and written on
// a local, iterated in Rust's key order (by code point: U+FFFD sorts below
// a key outside the BMP, which JS's UTF-16 `sort()` puts the other way),
// compared, and collected, with a later key's value replacing an earlier
// one's.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Stock {
    pub name: String,
    pub counts: BTreeMap<String, u32>,
    pub tags: BTreeSet<String>,
}

pub fn count_words(text: &str) -> BTreeMap<String, u32> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for w in text.split(' ') {
        if w.is_empty() {
            continue;
        }
        let n = counts.get(w).copied().unwrap_or(0);
        counts.insert(String::from(w), n + 1);
    }
    counts
}

pub fn keys(m: &BTreeMap<String, i32>) -> Vec<String> {
    m.keys().cloned().collect()
}

pub fn values(m: &BTreeMap<String, i32>) -> Vec<i32> {
    m.values().copied().collect()
}

/// Overflows at the first pair in key order whose sum does.
pub fn total(m: &BTreeMap<String, i32>) -> i32 {
    m.values().sum()
}

pub fn joined(m: &BTreeMap<String, u8>) -> String {
    let mut out = String::new();
    for (k, v) in m {
        out.push_str(&format!("{k}={v};"));
    }
    out
}

pub fn lookup(m: &BTreeMap<String, i64>, k: &str) -> (Option<i64>, bool, usize, bool) {
    (m.get(k).copied(), m.contains_key(k), m.len(), m.is_empty())
}

pub fn edited(m: &BTreeMap<String, i64>, k: String, v: i64) -> (Option<i64>, Option<i64>, BTreeMap<String, i64>) {
    let mut n = m.clone();
    let old = n.insert(k, v);
    let gone = n.remove("b");
    (old, gone, n)
}

/// The caller's map is not written.
pub fn untouched(m: &BTreeMap<String, i64>) -> (usize, usize) {
    let mut n = m.clone();
    n.insert(String::from("new"), 1);
    (m.len(), n.len())
}

pub fn set_ops(s: &BTreeSet<String>, k: String) -> (bool, bool, bool, Vec<String>) {
    let mut t = s.clone();
    let a = t.insert(k.clone());
    let b = t.contains(&k);
    let c = t.remove("a");
    (a, b, c, t.iter().cloned().collect())
}

pub fn from_pairs(words: Vec<String>) -> BTreeMap<String, usize> {
    words.iter().map(|w| (w.clone(), w.len())).collect()
}

pub fn from_keys(words: Vec<String>) -> BTreeSet<String> {
    words.into_iter().collect()
}

pub fn same(a: &BTreeMap<String, u8>, b: &BTreeMap<String, u8>) -> (bool, bool) {
    (a == b, a != b)
}

pub fn same_stock(a: &Stock, b: &Stock) -> bool {
    a == b
}

pub fn restock(s: &Stock, item: String, n: u32) -> Stock {
    let mut counts = s.counts.clone();
    let have = counts.get(&item).copied().unwrap_or(0);
    counts.insert(item.clone(), have + n);
    let mut tags = s.tags.clone();
    tags.insert(item);
    Stock { name: s.name.clone(), counts, tags }
}

pub fn largest(m: &BTreeMap<String, u32>) -> Option<String> {
    m.iter().max_by_key(|(_, v)| **v).map(|(k, _)| k.clone())
}
