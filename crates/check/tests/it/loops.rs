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
    assert_clean(
        "pub fn f(xs: Vec<u8>) -> u32 { let mut n = 0u32; for x in xs.iter().rev() { n += u32::from(*x); } n }",
    );
    let m = parse(
        "pub fn f(xs: Vec<u8>) -> u32 { let mut n = 0u32; for x in xs.iter().peekable() { n += u32::from(*x); } n }",
    );
    assert!(m.contains("`for` over `.peekable()` is not in v0"), "{m}");
    let m = parse("pub fn f() -> u32 { let mut n = 0u32; for (i, _) in (0..3u32).rev().enumerate() { n += i; } n }");
    assert!(m.contains("the range's own value is the index"), "{m}");
    assert_clean(
        "pub fn f() -> u32 { let mut n = 0u32; for i in 0..=3u32 { n += i; } for i in (0..=3u32).rev() { n += i; } n }",
    );
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
    assert_clean("pub fn f() -> u32 { loop { break; } 0 }");
    assert_clean("pub fn f(n: u32) -> u32 { let mut i = 0u32; loop { if i * i > n { return i; } i += 1; } }");
    assert_clean("pub fn f(n: u32) -> u32 { let mut i = 0u32; let r = loop { if i > n { break i * 2; } i += 1; }; r }");
    assert!(parse("pub fn f() -> u32 { 'a: while true { break 'a; } 0 }").contains("loop labels are not in v0"));
    assert!(parse("pub fn f() -> u32 { 'a: loop { break 'a 1; } }").contains("loop labels are not in v0"));
}

#[test]
fn split_takes_a_char_or_a_str_and_stands_in_a_for_head() {
    assert_clean(
        "pub fn f(s: &str) -> u32 { let mut n = 0u32; for t in s.split(',') { if !t.is_empty() { n += 1; } } n }",
    );
    assert_clean("pub fn f(s: &str) -> u32 { let mut n = 0u32; for _t in s.split(\"ab\") { n += 1; } n }");
    assert_rejects(
        "pub fn f(s: &str) -> u32 { let mut n = 0u32; for _t in s.split(1u8) { n += 1; } n }",
        "`split` takes a `char`, a `&str`, or a closure on a `char` in v0, found `u8`",
    );
    // A consumer may follow it; any other use of the pieces may not.
    assert_clean("pub fn f(s: &str) -> bool { s.split(' ').count() > 1 }");
    assert_rejects(
        "pub fn f(s: &str) -> usize { s.split(' ').len() }",
        "`.len()` on an iterator is not on the std allow-list",
    );
}

#[test]
fn if_let_and_let_else_on_variants() {
    let e = "pub enum E { A(u8), B { n: u8 }, C }\n";
    assert_clean(&format!(
        "{e}pub fn f(x: E) -> u8 {{ if let E::A(n) = x {{ n }} else if let E::B {{ n }} = x {{ n }} else {{ 0 }} }}"
    ));
    assert_clean(&format!("{e}pub fn f(x: E) -> u8 {{ let E::A(mut n) = x else {{ return 0 }}; n += 1; n }}"));
    assert_clean(&format!("{e}pub fn f(x: E) -> u8 {{ let E::C = x else {{ return 1 }}; 2 }}"));
    assert_clean("pub fn f(x: Option<(u8, u8)>) -> u8 { let Some((a, b)) = x else { return 0 }; a + b }");
    assert_clean(&format!(
        "{e}pub fn f(xs: Vec<E>) -> u32 {{ let mut n = 0u32; for x in xs {{ let E::A(v) = x else {{ continue }}; n += u32::from(v); }} n }}"
    ));
    let parse = |src: &str| purecrate_syntax::parse_source("c", src).expect_err(src).message;
    assert!(parse(
        "pub struct P { pub a: u8 } pub fn f(x: Option<P>) -> u8 { let Some(P { a }) = x else { return 0 }; a }"
    )
    .contains("not `let-else`"));
    assert!(parse(&format!("{e}pub fn f(x: E) -> u8 {{ let E::A(n): E = x else {{ return 0 }}; n }}"))
        .contains("takes no type annotation"));
    assert!(parse("pub fn f(x: u8) -> u8 { let _ = x else { return 0 }; 1 }").contains("always matches"));
}

#[test]
fn while_let_and_loop_values() {
    assert_clean(
        "pub fn f(s: &str) -> usize { let mut t: &str = s; while let Some(r) = t.strip_prefix(\"a\") { t = r; } t.len() }",
    );
    assert_clean("pub fn f(n: u8) -> u8 { let mut i = 0u8; let v = loop { if i == n { break i; } i += 1; }; v }");
    // A `loop` with no `break` is `!`: it may end a function of any type.
    assert_clean("pub fn f(n: u8) -> Option<u8> { let mut i = 0u8; loop { if i > n { return Some(i); } i += 1; } }");
    assert_rejects(
        "pub fn f(n: u8) -> u8 { let v = loop { if n > 1 { break 1u8; } break 2u16; }; v }",
        "expected `u8`",
    );
}

#[test]
fn an_unsuffixed_range_takes_its_first_use_type() {
    // An index asks a `usize`, as rustc infers it.
    assert_clean("pub fn f(b: [u8; 3]) -> u8 { let mut a = [0u8; 3]; for i in 0..3 { a[i] = b[i]; } a[0] }");
    assert_clean("pub fn f(v: Vec<u8>) -> u32 { let mut s: u32 = 0; for i in 0..3 { s += u32::from(v[i]); } s }");
    assert_clean(
        "pub fn f(v: &[u8]) -> u32 { let mut s: u32 = 0; for i in (0..=2).rev() { s += u32::from(v[i]); } s }",
    );
    assert_clean("pub fn f(n: u8) -> u8 { let mut s: u8 = 0; for i in 0..3 { s = s.wrapping_add(i) ^ n; } s }");
    // No use asks one (rustc's `i32` default), or the first asks another.
    assert_rejects(
        "pub fn f() -> u32 { let mut s: u32 = 0; for _ in 0..3 { s += 1; } s }",
        "cannot tell the integer type of `3`",
    );
    assert_rejects(
        "pub fn f(v: Vec<u8>) -> u8 { let mut s: u8 = 0; for i in 0..3 { s = s.wrapping_add(i); s = v[i]; } s }",
        "expected `usize`, found `u8`",
    );
}
