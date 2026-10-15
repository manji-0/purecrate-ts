// `BTreeMap<K, V>` and `BTreeSet<K>` with keys other than `String`
// (design/01 §7.26): every integer type, `char`, `bool`, `Uuid`, and
// newtypes of them and of `String`, open and closed. Each is built, read,
// and written on a local, iterated in the key type's order (`9` before
// `10`, `-2` before `-1`, a `bigint` past 2^53 in place, a `char` outside
// the BMP after U+FFFD), compared, collected, and written on the wire.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UserId(pub u64);

/// Closed: built only by `code_of`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Code(String);

pub fn code_of(s: String) -> Code {
    Code(s)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ledger {
    pub owner: UserId,
    pub balances: BTreeMap<UserId, i64>,
    pub seen: BTreeSet<char>,
}

pub fn keys(m: &BTreeMap<i32, u8>) -> Vec<i32> {
    m.keys().copied().collect()
}

pub fn values(m: &BTreeMap<i32, u8>) -> Vec<u8> {
    m.values().copied().collect()
}

/// Overflows at the first value in key order whose sum does.
pub fn total(m: &BTreeMap<i64, i8>) -> i8 {
    m.values().sum()
}

pub fn joined(m: &BTreeMap<u64, u8>) -> String {
    let mut out = String::new();
    for (k, v) in m {
        out.push_str(&format!("{k}={v};"));
    }
    out
}

pub fn wide(m: &BTreeMap<i128, u128>, s: &BTreeSet<u128>) -> (Vec<(i128, u128)>, Vec<u128>) {
    (m.iter().map(|(k, v)| (*k, *v)).collect(), s.iter().copied().collect())
}

pub fn lookup(m: &BTreeMap<u64, String>, k: u64) -> (Option<String>, bool, usize, bool) {
    (m.get(&k).cloned(), m.contains_key(&k), m.len(), m.is_empty())
}

pub fn edited(m: &BTreeMap<i16, i64>, k: i16, v: i64) -> (Option<i64>, Option<i64>, BTreeMap<i16, i64>) {
    let mut n = m.clone();
    let old = n.insert(k, v);
    let gone = n.remove(&-1);
    (old, gone, n)
}

/// The caller's map is not written.
pub fn untouched(m: &BTreeMap<usize, bool>) -> (usize, usize) {
    let mut n = m.clone();
    n.insert(7, true);
    (m.len(), n.len())
}

pub fn set_ops(s: &BTreeSet<char>, c: char) -> (bool, bool, bool, Vec<char>) {
    let mut t = s.clone();
    let a = t.insert(c);
    let b = t.contains(&c);
    let d = t.remove(&'a');
    (a, b, d, t.iter().copied().collect())
}

pub fn letters(text: &str) -> BTreeSet<char> {
    text.chars().collect()
}

pub fn counts(xs: Vec<u8>) -> BTreeMap<u8, usize> {
    let mut m: BTreeMap<u8, usize> = BTreeMap::new();
    for x in xs {
        let n = m.get(&x).copied().unwrap_or(0);
        m.insert(x, n + 1);
    }
    m
}

/// A later key's value replaces an earlier one's.
pub fn from_pairs(xs: Vec<(i8, u32)>) -> BTreeMap<i8, u32> {
    xs.into_iter().collect()
}

pub fn p06_btreeset(xs: Vec<u32>) -> BTreeSet<u32> {
    xs.into_iter().collect()
}

pub fn flags(m: &BTreeMap<bool, u32>) -> (Vec<bool>, Option<u32>, Option<u32>) {
    (m.keys().copied().collect(), m.get(&false).copied(), m.get(&true).copied())
}

pub fn ids(m: &BTreeMap<Uuid, u8>, k: Uuid) -> (Vec<Uuid>, Option<u8>) {
    (m.keys().copied().collect(), m.get(&k).copied())
}

pub fn same(a: &BTreeMap<u32, u8>, b: &BTreeMap<u32, u8>) -> (bool, bool) {
    (a == b, a != b)
}

pub fn same_sets(a: &BTreeSet<i64>, b: &BTreeSet<i64>) -> bool {
    a == b
}

pub fn same_ledger(a: &Ledger, b: &Ledger) -> bool {
    a == b
}

/// A newtype key: in its content's order, found by an equal value.
pub fn pay(l: &Ledger, to: UserId, n: i64) -> Ledger {
    let mut balances = l.balances.clone();
    let have = balances.get(&to).copied().unwrap_or(0);
    balances.insert(to, have + n);
    let mut seen = l.seen.clone();
    seen.insert('$');
    Ledger { owner: l.owner, balances, seen }
}

pub fn owners(l: &Ledger) -> Vec<u64> {
    l.balances.keys().map(|k| k.0).collect()
}

pub fn richest(l: &Ledger) -> Option<UserId> {
    l.balances.iter().max_by_key(|(_, v)| **v).map(|(k, _)| *k)
}

pub fn by_code(m: &BTreeMap<Code, u8>, k: Code) -> (Vec<Code>, Option<u8>, bool) {
    (m.keys().cloned().collect(), m.get(&k).copied(), m.contains_key(&k))
}

pub fn code_set(words: Vec<String>) -> BTreeSet<Code> {
    words.into_iter().map(code_of).collect()
}

/// A map whose values are maps, each keyed by an integer.
pub fn grid(cells: Vec<(u8, u8, char)>) -> BTreeMap<u8, BTreeMap<u8, char>> {
    let mut rows: BTreeMap<u8, BTreeMap<u8, char>> = BTreeMap::new();
    for (r, c, x) in cells {
        let mut row: BTreeMap<u8, char> = rows.get(&r).cloned().unwrap_or(BTreeMap::new());
        row.insert(c, x);
        rows.insert(r, row);
    }
    rows
}

/// On the wire: an object keyed by each key's text, sets as arrays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyed {
    pub small: BTreeMap<i8, u8>,
    pub bytes: BTreeMap<u8, u8>,
    pub wide: BTreeMap<u64, u8>,
    pub signed: BTreeMap<i64, u8>,
    pub huge: BTreeMap<u128, u8>,
    pub least: BTreeMap<i128, u8>,
    pub count: BTreeMap<usize, u8>,
    pub flags: BTreeMap<bool, u8>,
    pub letters: BTreeMap<char, u8>,
    pub ids: BTreeMap<Uuid, u8>,
    pub users: BTreeMap<UserId, String>,
    pub codes: BTreeMap<Code, u8>,
    pub tags: BTreeSet<u32>,
    pub chars: BTreeSet<char>,
    pub members: BTreeSet<UserId>,
    pub nested: BTreeMap<i32, BTreeMap<char, Vec<u8>>>,
}
