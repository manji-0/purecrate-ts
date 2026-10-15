//! `BTreeMap` and `BTreeSet` keys other than `String` (design/01 §7.26):
//! the primitives and newtypes of them that a JS `Map` finds by value are
//! accepted; a key that is an object in TS, a float, and a newtype ordered
//! by hand are refused.

use crate::common::{assert_clean, assert_rejects};

const USE: &str = "use std::collections::{BTreeMap, BTreeSet};\n";

#[test]
fn integer_char_bool_uuid_and_newtype_keys_are_accepted() {
    assert_clean(&format!(
        "{USE}#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
         pub struct UserId(pub u64);
         #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
         pub struct Code(String);
         pub type Small = u8;
         pub fn f(a: &BTreeMap<i128, u8>, b: &BTreeSet<char>, c: &BTreeMap<bool, u32>, d: &BTreeMap<UserId, Code>,
                  e: &BTreeSet<Code>, g: &BTreeMap<Small, usize>) -> usize {{
             a.len() + b.len() + c.len() + d.len() + e.len() + g.len()
         }}
         pub fn h(xs: Vec<u32>) -> BTreeSet<u32> {{ xs.into_iter().collect() }}
         pub fn k(m: &BTreeMap<UserId, i64>, id: u64) -> Option<i64> {{ m.get(&UserId(id)).copied() }}
         pub fn l() -> BTreeMap<i16, u8> {{
             let mut m: BTreeMap<i16, u8> = BTreeMap::new();
             m.insert(-1, 2);
             m.remove(&-1);
             m
         }}"
    ));
}

#[test]
fn a_key_that_is_an_object_in_ts_is_refused() {
    let cases = [
        (
            "#[derive(PartialEq, Eq, PartialOrd, Ord)] pub enum Color { Red, Blue }
             pub fn f(m: &BTreeMap<Color, u8>) -> usize { m.len() }",
            "the key `Color` is an enum, an object in TS",
        ),
        (
            "#[derive(PartialEq, Eq, PartialOrd, Ord)] pub struct Pos { pub x: i32, pub y: i32 }
             pub fn f(m: &BTreeMap<Pos, u8>) -> usize { m.len() }",
            "the key `Pos` is a struct with named fields or more than one",
        ),
        (
            "#[derive(PartialEq, Eq, PartialOrd, Ord)] pub struct One { pub id: u8 }
             pub fn f(s: &BTreeSet<One>) -> usize { s.len() }",
            "the key `One` is a struct with named fields or more than one",
        ),
        ("pub fn f(m: &BTreeMap<(u8, u8), u8>) -> usize { m.len() }", "the key `(u8, u8)` is an object in TS"),
        ("pub fn f(s: &BTreeSet<Option<u8>>) -> usize { s.len() }", "the key `Option<u8>` is an object in TS"),
        ("pub fn f(m: &BTreeMap<Vec<u8>, u8>) -> usize { m.len() }", "the key `Vec<u8>` is an object in TS"),
        (
            "#[derive(PartialEq, Eq, PartialOrd, Ord)] pub enum Color { Red, Blue }
             #[derive(PartialEq, Eq, PartialOrd, Ord)] pub struct Hue(pub Color);
             pub fn f(m: &BTreeMap<Hue, u8>) -> usize { m.len() }",
            "the key `Hue` is a newtype of `Color`, which is no key",
        ),
        (
            "#[derive(PartialEq, Eq, PartialOrd, Ord)] pub struct Ids(pub Vec<u8>);
             pub fn f(s: &BTreeSet<Ids>) -> usize { s.len() }",
            "the key `Ids` is a newtype of `Vec<u8>`, which is no key",
        ),
        (
            "pub fn f<T: Ord>(m: &BTreeMap<T, u8>) -> usize { m.len() }",
            "the key is the type parameter `T`, which may be an object in TS",
        ),
    ];
    for (body, needle) in cases {
        assert_rejects(&format!("{USE}{body}"), needle);
        assert_rejects(&format!("{USE}{body}"), "a JS `Map` compares an object key by identity");
    }
}

#[test]
fn a_float_key_is_refused() {
    assert_rejects(
        &format!("{USE}pub fn f(m: &BTreeMap<u8, f64>, s: &BTreeSet<f64>) -> usize {{ m.len() + s.len() }}"),
        "the key `f64` is a float, which Rust does not order",
    );
}

#[test]
fn a_newtype_key_ordered_by_hand_or_not_at_all_is_refused() {
    assert_rejects(
        &format!(
            "{USE}#[derive(PartialEq, Eq)] pub struct Rev(pub u32);
             impl Ord for Rev {{ fn cmp(&self, other: &Self) -> std::cmp::Ordering {{ other.0.cmp(&self.0) }} }}
             impl PartialOrd for Rev {{ fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {{ Some(self.cmp(other)) }} }}
             pub fn f(m: &BTreeMap<Rev, u8>) -> usize {{ m.len() }}"
        ),
        "the key `Rev` has a hand-written `Ord`, and the output sorts a newtype key by its content",
    );
    assert_rejects(
        &format!("{USE}pub struct Plain(pub u32); pub fn f(m: &BTreeMap<Plain, u8>) -> usize {{ m.len() }}"),
        "the key `Plain` has no derived `Ord`",
    );
}

#[test]
fn a_key_argument_of_another_type_is_refused() {
    assert_rejects(
        &format!("{USE}pub fn f(m: &BTreeMap<u64, u8>, k: u32) -> bool {{ m.contains_key(&k) }}"),
        "a key is a `u64`, found `u32`",
    );
}
