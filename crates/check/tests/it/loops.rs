//! `for` over collections: what it iterates, and what it refuses.

use crate::common::{assert_clean, assert_rejects};

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
    let m =
        parse("pub fn f(xs: Vec<u8>) -> u32 { let mut n = 0u32; for x in xs.iter().rev() { n += u32::from(*x); } n }");
    assert!(m.contains("`for` over `.rev()` is not in v0"), "{m}");
    let m = parse("pub fn f() -> u32 { let mut n = 0u32; for i in 0..=3u32 { n += i; } n }");
    assert!(m.contains("not `a..=b`"), "{m}");
    assert_rejects(
        "pub fn f(x: Option<u8>) -> u32 { let mut n = 0u32; for _v in x { n += 1; } n }",
        "`for x in xs` takes a range `a..b`, a `Vec` or slice",
    );
}

#[test]
fn while_break_and_continue_stand_as_statements() {
    assert_clean("pub fn f(n: u32) -> u32 { let mut i = 0u32; while i < n { i += 1; if i == 7 { break; } } i }");
    assert_clean(
        "pub fn f(xs: Vec<u8>) -> u32 { let mut n = 0u32; for x in &xs { match x { 0 => continue, _ => n += 1 } } n }",
    );
    // A jump inside a value the TS prints as an expression cannot leave the loop.
    assert_rejects(
        "pub fn f(n: u32) -> u32 { let mut i = 0u32; while i < n { i = i + if i > 3 { break } else { 1 }; } i }",
        "`break` or `continue` inside a larger expression is not in v0",
    );
    let parse = |src: &str| purecrate_syntax::parse_source("c", src).expect_err(src).message;
    assert!(parse("pub fn f() -> u32 { loop { break; } 0 }").contains("`loop` is not in v0"));
    assert!(parse("pub fn f() -> u32 { 'a: while true { break 'a; } 0 }").contains("loop labels are not in v0"));
}

#[test]
fn split_takes_a_char_and_stands_in_a_for_head() {
    assert_clean(
        "pub fn f(s: &str) -> u32 { let mut n = 0u32; for t in s.split(',') { if !t.is_empty() { n += 1; } } n }",
    );
    assert_rejects(
        "pub fn f(s: &str) -> u32 { let mut n = 0u32; for _t in s.split(\"ab\") { n += 1; } n }",
        "`split` takes a `char` separator in v0",
    );
    // A consumer may follow it; any other use of the pieces may not.
    assert_clean("pub fn f(s: &str) -> bool { s.split(' ').count() > 1 }");
    assert_rejects(
        "pub fn f(s: &str) -> usize { s.split(' ').len() }",
        "`.split()` on `&str` is not on the std allow-list",
    );
}
