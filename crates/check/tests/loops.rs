//! `for` over collections: what it iterates, and what it refuses.

mod common;

use common::{assert_clean, assert_rejects};

#[test]
fn for_walks_vecs_slices_and_string_bytes() {
    assert_clean("pub fn f(xs: Vec<u8>) -> u8 { let mut m = 0u8; for x in &xs { if *x > m { m = *x; } } m }");
    assert_clean("pub fn f(xs: &[u8]) -> u32 { let mut n = 0u32; for _x in xs.iter() { n += 1; } n }");
    assert_clean("pub fn f(s: &str) -> u32 { let mut n = 0u32; for b in s.bytes() { n += u32::from(b); } n }");
    assert_clean("pub fn f(s: &str) -> u32 { let mut n = 0u32; for b in s.as_bytes() { n += u32::from(*b); } n }");
}

#[test]
fn for_refuses_adaptors_and_non_collections() {
    let parse = |src: &str| purecrate_syntax::parse_source("c", src).expect_err(src).message;
    let m = parse("pub fn f(xs: Vec<u8>) -> u32 { let mut n = 0u32; for x in xs.iter().rev() { n += u32::from(*x); } n }");
    assert!(m.contains("`for` over `.rev()` is not in v0"), "{m}");
    let m = parse("pub fn f() -> u32 { let mut n = 0u32; for i in 0..=3u32 { n += i; } n }");
    assert!(m.contains("not `a..=b`"), "{m}");
    assert_rejects(
        "pub fn f(x: Option<u8>) -> u32 { let mut n = 0u32; for _v in x { n += 1; } n }",
        "`for x in xs` takes a range `a..b`, a `Vec` or slice",
    );
}
