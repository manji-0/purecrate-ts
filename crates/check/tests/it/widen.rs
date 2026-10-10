use crate::common::{assert_clean, assert_rejects};

#[test]
fn lossless_integer_from_is_accepted() {
    assert_clean("pub fn f(q: u32, p: i64) -> i64 { p * i64::from(q) }");
    assert_clean("pub fn f(n: u16) -> usize { usize::from(n) }");
    assert_clean("pub fn f(n: i32) -> i32 { i32::from(n) }");
}

#[test]
fn conversions_std_has_no_from_for_are_rejected() {
    let lossy = "std has no lossless conversion";
    assert_rejects("pub fn f(n: u64) -> i64 { i64::from(n) }", lossy);
    assert_rejects("pub fn f(n: i64) -> i32 { i32::from(n) }", lossy);
    assert_rejects("pub fn f(n: i8) -> u16 { u16::from(n) }", lossy);
    assert_rejects("pub fn f(n: u32) -> usize { usize::from(n) }", lossy);
}

#[test]
fn integer_from_needs_a_typed_integer() {
    assert_rejects("pub fn f() -> i64 { i64::from(5) }", "cannot tell the integer type");
    assert_rejects("pub struct Qty(u32); pub fn f(q: Qty) -> i64 { i64::from(q) }", "takes an integer in v0");
    assert_rejects("pub fn f(x: f64) -> i64 { i64::from(x) }", "takes an integer in v0");
    assert_rejects("pub fn f(n: u8) -> u8 { u16::from(n) }", "expected `u8`, found `u16`");
}

#[test]
fn a_refused_cast_names_what_as_does() {
    let float = "no conversion between a float and an integer";
    assert_rejects("pub fn f(x: f64) -> i64 { x as i64 }", float);
    assert_rejects("pub fn f(n: i64) -> f64 { n as f64 }", float);
    assert_rejects("pub fn f(n: i64) -> i32 { n as i32 }", "or widens a `u8`, `u16`, or `u32` to `usize`");
}

#[test]
fn a_std_constant_is_not_called_a_function() {
    assert_rejects(
        "pub fn f() -> f64 { f64::NAN }",
        "`f64::NAN` is not in v0: std's associated functions and constants",
    );
}
