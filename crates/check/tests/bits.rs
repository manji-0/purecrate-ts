mod common;

use common::{assert_clean, assert_rejects};

#[test]
fn bitwise_operators_and_shifts_on_fixed_widths_are_accepted() {
    assert_clean("pub fn f(a: u8, b: u8) -> u8 { a & b | a ^ b }");
    assert_clean("pub fn f(a: i64, n: u8) -> i64 { !(a << n) >> 1 }");
    assert_clean("pub fn f(a: u32) -> u32 { let mut x = a; x <<= 3; x |= 1u32; x }");
    assert_clean("pub fn f() -> u64 { 1 << 40 }");
    assert_clean("pub fn f(a: bool) -> bool { !a }");
}

#[test]
fn usize_has_no_bitwise_operators() {
    let why = "not 64 bits";
    assert_rejects("pub fn f(a: usize, b: usize) -> usize { a & b }", why);
    assert_rejects("pub fn f(a: usize) -> usize { a >> 1 }", why);
    assert_rejects("pub fn f(a: usize) -> usize { !a }", why);
}

#[test]
fn bitwise_operators_on_bool_point_to_the_logical_ones() {
    assert_rejects("pub fn f(a: bool, b: bool) -> bool { a & b }", "write `&&`");
    assert_rejects("pub fn f(a: bool, b: bool) -> bool { a | b }", "write `||`");
    assert_rejects("pub fn f(a: bool, b: bool) -> bool { a ^ b }", "write `!=`");
}

#[test]
fn bitwise_operand_types_must_be_known() {
    assert_rejects("pub fn f() { let _x = 1 << 3; }", "cannot tell the integer type");
    assert_rejects("pub fn f(a: f64) -> f64 { a << 1 }", "`<<` on `f64` is not in v0");
}
