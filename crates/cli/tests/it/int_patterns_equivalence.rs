//! Byte literals (`b'0'`) and integer literal and range patterns in `match`
//! (design/02 §3.5): every case in `fixtures/int_patterns.rs` agrees between
//! Rust and the generated package, for `u8`, `i32`, and the `bigint`-backed
//! `i64`/`u64`, at and around every bound.

use crate::support;

purecrate_canon::fixture!(mod ints = "fixtures/int_patterns.rs");

#[test]
fn integer_patterns_match_rust() {
    let cases = support::cases(|cases| {
        grid!(cases, [ints::class_of, ints::escapes, ints::hex_value]; b in 0u8..=255);
        grid!(
            cases, ints::signed;
            x in [i32::MIN, i32::MIN + 1, -7, -6, -5, -4, -1, 0, 1, 4, 5, 9, 10, 99, 100, 101, 200, 201, i32::MAX]
        );
        grid!(
            cases, ints::wide;
            x in [i64::MIN, i64::MIN + 1, -1, 0, 1, 1000, 1001, i64::MAX], y in [0u64, 9, 10, u64::MAX - 1, u64::MAX]
        );
        grid!(cases, ints::digits; s in ["", "0", "12_3", "4a5", "999999999", "9999999999"]);
    });
    assert!(cases.iter().any(|c| c.rust.starts_with("panic(")), "the overflow case must panic");
    support::assert_equivalent("int_patterns", ints::SOURCE, &cases);
}
