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
    assert_rejects("pub fn f(b: bool) -> u8 { b as u8 }", "or converts between numeric types; nothing else is");
}

/// Floats and integers convert by `as` where std has no `From`, and by
/// `from` where it has one, as between integers.
#[test]
fn floats_convert_as_rust_does() {
    assert_clean("pub fn f(x: f64, n: i64, y: f32) -> (i64, f64, u8, f32) { (x as i64, n as f64, y as u8, x as f32) }");
    assert_clean("pub fn f(n: i32, b: u16, y: f32) -> (f64, f32, f64) { (f64::from(n), f32::from(b), f64::from(y)) }");
    assert_rejects("pub fn f(n: i32) -> f64 { n as f64 }", "`i32 as f64` widens, which v0 writes `f64::from(x)`");
    assert_rejects("pub fn f(y: f32) -> f64 { y as f64 }", "`f32 as f64` widens");
    assert_rejects("pub fn f(n: i64) -> f64 { f64::from(n) }", "`f64::from` does not take `i64`");
    assert_rejects("pub fn f(x: f64) -> f32 { f32::from(x) }", "`f32::from` does not take `f64`");
    assert_clean("pub fn f(x: f64) -> (bool, bool, f64, f64) { (x.is_nan(), x.is_finite(), x.round(), f64::NAN) }");
    assert_rejects("pub fn f(x: f64) -> f64 { x.sqrt() }", "allowed: `round`, `floor`, `ceil`, `trunc`, `abs`");
}

#[test]
fn a_std_constant_is_not_called_a_function() {
    assert_rejects(
        "pub fn f() -> f64 { f64::EPSILON }",
        "`f64::EPSILON` is not in v0: std's associated functions and constants",
    );
}
