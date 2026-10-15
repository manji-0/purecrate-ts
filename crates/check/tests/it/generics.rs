//! Type parameters: what a generic item may hold and do, and the
//! instantiations refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects};

#[test]
fn generic_items_are_accepted() {
    assert_clean("pub struct Page<T> { pub items: Vec<T>, pub total: u32 }\npub fn len<T>(p: &Page<T>) -> usize { p.items.len() }");
    assert_clean(
        "pub fn id<T: Clone + PartialEq + std::fmt::Debug>(x: &T) -> T { x.clone() }"
            .replace("std::fmt::Debug", "Eq")
            .as_str(),
    );
    assert_clean("pub enum E<A, B> { L(A), R(B) }\npub fn l(n: u8) -> E<u8, String> { E::L(n) }");
    assert_clean("#[derive(Clone)]\npub struct W<T> { pub v: T }\nimpl<T: Clone> W<T> { pub fn get(&self) -> T { self.v.clone() } }\npub fn f(w: W<i32>) -> i32 { w.get() }");
    assert_clean("pub fn same<T>(a: &T, b: &T) -> bool where T: PartialEq { a == b }");
}

#[test]
fn bounds_other_than_the_structural_ones_are_refused() {
    assert_parse_rejects("pub fn f<T: std::hash::Hash>(x: T) -> T { x }", "the bound `std::hash::Hash` is not in v0");
    assert_parse_rejects("pub fn f<T: Into<u32>>(x: T) -> u32 { x.into() }", "is not in v0");
    assert_parse_rejects("pub fn f<const N: usize>() -> usize { N }", "const generics are not in v0");
}

#[test]
fn a_type_parameter_is_ordered_through_its_bound() {
    assert_clean("pub fn max<T: Ord + Clone>(a: &T, b: &T) -> T { if a > b { a.clone() } else { b.clone() } }");
    // On a struct's parameter a std bound carries nothing, as in 0.20.0.
    assert_clean("pub struct S<T: Ord + std::fmt::Display> { pub t: T }\npub fn f(s: &S<u8>) -> u8 { s.t }");
}

#[test]
fn an_instantiation_that_nests_null_is_refused() {
    assert_rejects(
        "fn first<T: Clone>(xs: &[T]) -> Option<T> { xs.first().cloned() }\npub fn f(xs: Vec<Option<u8>>) -> bool { first(&xs).is_some() }",
        "takes `Option<u8>` as `T`",
    );
    assert_rejects("pub struct W<T> { pub v: T }\npub fn f() -> W<()> { W { v: () } }", "takes `()` as `T`");
}

#[test]
fn an_uninferred_parameter_asks_for_a_type() {
    assert_rejects(
        "fn empty<T>() -> Vec<T> { Vec::new() }\npub fn f() -> usize { empty().len() }",
        "is not known here",
    );
}

#[test]
fn a_generic_type_derives_serde() {
    // No serde in the test's rustc: the subset's checks alone.
    let found = crate::common::messages("use serde::{Deserialize, Serialize};\n#[derive(Serialize, Deserialize)]\npub struct Page<T> { pub items: Vec<T> }\n#[derive(Serialize, Deserialize)]\npub struct Users { pub page: Page<String> }");
    assert!(found.is_empty(), "{found:#?}");
}
