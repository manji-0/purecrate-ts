//! `const` items and `as` on enums: what is folded, and what is refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects, diagnostics};

#[test]
fn consts_fold_from_literals_consts_and_discriminants() {
    assert_clean("pub const A: u32 = 1 << 4;\npub const B: u32 = A * 2 + 1;\npub fn f() -> u32 { B }");
    assert_clean(
        "#[derive(Clone, Copy)]\npub enum E { X = 3, Y }\npub const M: u8 = E::X as u8 | E::Y as u8;\npub fn f() -> u8 { M }",
    );
    assert_clean("pub const S: &str = \"x\";\npub const T: &str = S;\npub fn f(s: &str) -> bool { s == T }");
}

#[test]
fn a_const_in_a_pattern_is_refused() {
    // Rust compares with the const's value; a binding would match anything.
    // A bare `LIMIT =>` arm is already refused as a binding arm; nested, a
    // binding is allowed, so the const must be caught by name.
    assert_rejects(
        "pub const LIMIT: u32 = 3;\npub fn f(x: Option<u32>) -> u32 { match x { Some(LIMIT) => 1, _ => 0 } }",
        "`LIMIT` is a const: matching against a const is not in v0",
    );
    assert_rejects(
        "pub const LIMIT: u32 = 3;\npub fn f(x: Option<u32>) -> u32 { if let Some(LIMIT) = x { 1 } else { 0 } }",
        "`LIMIT` is a const: matching against a const is not in v0",
    );
}

#[test]
fn const_expressions_outside_v0_are_refused() {
    let code = |src: &str| diagnostics(src).into_iter().map(|d| (d.reason.code(), d.message)).collect::<Vec<_>>();
    let found = code("pub fn g() -> u32 { 1 }\npub const A: u32 = g();\npub fn f() -> u32 { A }");
    assert!(found.iter().any(|(c, m)| *c == "check/const-expr" && m.contains("const `A`")), "{found:?}");
    assert_rejects("pub const A: u8 = 200 + 100;\npub fn f() -> u8 { A }", "300 does not fit `u8`");
    assert_rejects("pub const A: u32 = 1 << 32;\npub fn f() -> u32 { A }", "shift by 32 overflows a `u32`");
    assert_rejects(
        "pub const A: u32 = 1;\npub const B: u64 = A;\npub fn f() -> u64 { B }",
        "`A` is a `u32`, where a `u64` is expected",
    );
    assert_rejects(
        "pub struct P { pub x: i32 }\npub const O: P = P { x: 0 };\npub fn f() -> i32 { 0 }",
        "a const of type `P` is not in v0",
    );
}

#[test]
fn as_reads_only_a_fieldless_enums_discriminant() {
    // A widening std's `From` takes has that one spelling.
    assert_rejects("pub fn f(x: u8) -> u32 { x as u32 }", "`u8 as u32` widens, which v0 writes `u32::from(x)`");
    assert_rejects("pub fn f(x: i32) -> i64 { x as i64 }", "`i32 as i64` widens");
    // Unsigned to `usize` within 32 bits holds every value: Rust has no
    // `usize::from(u32)`, so `as` is how Rust writes it.
    assert_clean("pub fn f(a: u8, b: u16, c: u32) -> usize { a as usize + b as usize + c as usize }");
    // Every other pair of integer types wraps, as Rust's `as` does.
    assert_clean("pub fn f(x: u64, y: i32, z: usize) -> (usize, usize, u32) { (x as usize, y as usize, z as u32) }");
    assert_clean("pub fn f(x: u64, y: i64) -> (i64, u64, u8, i8) { (x as i64, y as u64, x as u8, y as i8) }");
    assert_clean("pub fn f(x: u32) -> u32 { x as u32 }");
    assert_rejects("pub enum E { A(i32), B }\npub fn f(e: E) -> u8 { e as u8 }", "`E as u8` is not in v0");
    assert_rejects(
        "#[derive(Clone, Copy)]\npub enum E { A = 1, B = 300 }\npub fn f(e: E) -> u8 { e as u8 }",
        "`E as u8` would not hold `E::B` = 300",
    );
    assert_clean("#[derive(Clone, Copy)]\npub enum E { A = 1, B = 300 }\npub fn f(e: E) -> u16 { e as u16 }");
    assert_rejects(
        "#[repr(u8)]\n#[derive(Clone, Copy)]\npub enum E { A = 255, B }\npub fn f(e: E) -> u8 { e as u8 }",
        "`E::B` would follow the largest `u8`",
    );
}

#[test]
fn consts_share_one_file() {
    // `MAX` and `fn max` are two files; only an item named like the file meets it.
    assert_clean("pub const MAX: u32 = 9;\npub fn max() -> u32 { MAX }");
    assert_rejects(
        "pub const MAX: u32 = 9;\npub fn consts() -> u32 { MAX }",
        "would both be emitted as `consts.ts`, which holds the crate's consts",
    );
}

#[test]
fn a_local_const_is_a_let_at_the_top_of_its_block() {
    assert_clean("pub fn f(x: u32) -> u32 { let y = x * K; const K: u32 = 3; y }");
    assert_parse_rejects(
        "pub fn f(x: Option<u32>) -> u32 { const LIMIT: u32 = 3; match x { Some(LIMIT) => 1, _ => 0 } }",
        "`LIMIT` is a const: matching against a const is not in v0",
    );
    assert_parse_rejects(
        "pub fn f() -> u32 { const K: u32 = 3; let K = 4; K }",
        "`K` is a const: matching against a const is not in v0",
    );
    assert_parse_rejects(
        "pub fn f() -> u32 { fn g() -> u32 { 1 } g() }",
        "items inside blocks other than `const` are not in v0",
    );
}

/// A `&[u8]` const is a byte string; a `u128` literal stops below 2^127, as
/// literals are held as `i128`.
#[test]
fn byte_strings_and_wide_literals() {
    assert_clean("const A: &[u8] = b\"0123\";\nconst B: &[u8] = b\"\\xff\\x00\";\npub fn f(i: usize) -> (u8, u8) { (A[i], B[i]) }");
    assert_rejects(
        "const A: &[u8] = &[1, 2];\npub fn f() -> u8 { A[0] }",
        "a `&[u8]` const is a byte string `b\"..\"` in v0",
    );
    assert_clean("pub fn f(x: u128) -> u128 { x ^ 170141183460469231731687303715884105727 }");
    assert_parse_rejects(
        "pub fn f() -> u128 { 170141183460469231731687303715884105728 }",
        "is 2^127 or more, which v0 does not hold",
    );
}
