//! `collect` only from `s.split(c)` or `s.split(c).map(f)`, into a `Vec` or
//! a `Result<Vec<_>, _>` the context names. `split_once` takes a `char` or
//! a `&str`.

use crate::common::{assert_clean, assert_rejects};

#[test]
fn split_collects_into_the_named_vec_or_result() {
    assert_clean("pub fn f(s: &str) -> Vec<&str> { s.split('.').collect() }");
    assert_clean("pub fn f(s: &str) -> Vec<&str> { s.split('.').collect::<Vec<&str>>() }");
    assert_clean("pub fn f(s: &str) -> Vec<&str> { let parts: Vec<&str> = s.split('.').collect(); parts }");
    assert_clean("fn id(p: &str) -> String { String::from(p) } pub fn f(s: &str) -> Vec<String> { s.split(',').map(id).collect() }");
    assert_clean(
        "fn id(p: &str) -> Result<String, bool> { if p.is_empty() { Err(true) } else { Ok(String::from(p)) } } \
         pub fn f(s: &str) -> Result<Vec<String>, bool> { s.split(',').map(id).collect::<Result<Vec<String>, bool>>() }",
    );
    assert_clean(
        "pub fn f(s: &str, c: char) -> Option<(String, String)> { \
            match s.split_once(c) { Some(t) => { let (a, b) = t; Some((String::from(a), String::from(b))) } None => None } }",
    );
    assert_clean(
        "pub fn f(s: String, p: &str) -> bool { match s.split_once(p) { Some(t) => { let (a, _) = t; a.is_empty() } None => false } }",
    );
}

#[test]
fn collect_refuses_a_list_that_is_not_read_from_text() {
    assert_rejects(
        "pub fn f(xs: Vec<u8>) -> Vec<u8> { xs.iter().map(|x| *x).collect() }",
        "`collect` builds a `Vec` only from `s.split(c)`",
    );
    assert_rejects(
        "pub fn f(s: &str) -> Vec<char> { s.chars().collect() }",
        "`collect` builds a `Vec` only from `s.split(c)`",
    );
    assert_rejects(
        "pub fn f(s: &str) -> usize { let parts = s.split('.').collect(); parts.len() }",
        "`collect` needs its target",
    );
    assert_rejects(
        "pub fn f(s: &str) -> Result<Vec<&str>, bool> { s.split('.').collect() }",
        "collecting into a `Result` takes `.map(f)`",
    );
    assert_rejects(
        "pub fn f(s: &str) -> u32 { s.split(',').collect() }",
        "`collect` builds a `Vec<T>` or a `Result<Vec<T>, E>`",
    );
}

#[test]
fn split_once_refuses_a_needle_that_is_not_text() {
    assert_rejects(
        "pub fn f(s: &str) -> bool { s.split_once(1u32).is_some() }",
        "`str::split_once` takes a `&str` pattern",
    );
    assert_rejects(
        "pub fn f(s: &str) -> bool { s.split_once().is_some() }",
        "takes 1 argument",
    );
}

#[test]
fn map_of_collect_refuses_a_variant_constructor() {
    // A tuple variant is a function in Rust (`PreId::Numeric`). Here it is
    // built with no fields, so it never reaches `map`.
    assert_rejects(
        "pub enum E { A(u8) } pub fn f(s: &str) -> Vec<E> { s.split(',').map(E::A).collect() }",
        "`E::A` is constructed with the wrong shape",
    );
    assert_rejects(
        "pub enum E { A } pub fn f(s: &str) -> Vec<E> { s.split(',').map(E::A).collect() }",
        "`map` takes a closure",
    );
}
