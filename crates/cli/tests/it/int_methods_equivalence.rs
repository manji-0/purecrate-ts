//! The integer methods against Rust at every type's edges: `min`, `max`,
//! `abs` (panicking on `MIN`), `pow` (panicking on overflow), and the
//! `checked_*`, `saturating_*`, and `wrapping_*` forms, on `i8`, `u8`,
//! `i16`, `i32`, `u32`, `i64`, `u64`, and `usize`.

use crate::support;

purecrate_canon::fixture!(mod int_methods = "fixtures/int_methods.rs");

const SOURCE: &str = int_methods::SOURCE;

#[test]
fn generated_int_methods_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in [0u8, 1, 2, 3, 15, 16, 128, 255] {
            for b in [0u8, 1, 127, 255] {
                cases.push(case!(int_methods::clamp_add(a, b, 200)));
            }
            for e in [0u32, 1, 2, 5, 7, 8, 9, 64, u32::MAX] {
                cases.push(case!(int_methods::power_u8(a, e)));
            }
        }
        for a in [i8::MIN, -128 + 1, -3, -2, -1, 0, 1, 2, 11, 12, i8::MAX] {
            for e in [0u32, 1, 2, 3, 5, 6, 7, 8, 9, 100, u32::MAX] {
                cases.push(case!(int_methods::power_i8(a, e)));
            }
            for b in [i8::MIN, -1, 0, 1, 2, i8::MAX] {
                cases.push(case!(int_methods::saturating(a, b)));
                cases.push(case!(int_methods::wrapping(a, b)));
            }
        }
        for a in [i32::MIN, i32::MIN + 1, -7, -1, 0, 1, 7, 46_341, i32::MAX] {
            for b in [i32::MIN, -2, -1, 0, 1, 2, 46_341, i32::MAX] {
                cases.push(case!(int_methods::spread(a, b)));
                cases.push(case!(int_methods::checked(a, b)));
                cases.push(case!(int_methods::checked_div(a, b)));
                cases.push(case!(int_methods::wrapping_div(a, b)));
            }
            for e in [0u32, 1, 2, 30, 31, 32] {
                cases.push(case!(int_methods::power(a, e)));
            }
        }
        for a in [-2i32, 2] {
            cases.push(case!(int_methods::power(a, 31)));
        }
        for a in [i16::MIN, -1, 0, 5, i16::MAX] {
            cases.push(case!(int_methods::magnitude(a)));
        }
        for a in [i64::MIN, -3, 0, 3, 3_037_000_500, i64::MAX] {
            for b in [i64::MIN, -1, 0, 3, 3_037_000_500, i64::MAX] {
                cases.push(case!(int_methods::wide(a, b)));
            }
        }
        for a in [0u64, 1, u64::MAX] {
            for b in [0u64, 1, u64::MAX] {
                cases.push(case!(int_methods::unsigned_wide(a, b)));
            }
        }
        for len in [0usize, 1, 10] {
            for used in [0usize, 1, 10, 11] {
                cases.push(case!(int_methods::remaining(len, used)));
            }
        }
        for a in [0u32, 1, u32::MAX] {
            cases.push(case!(int_methods::unsigned_neg(a)));
        }
        cases
    });
    support::assert_equivalent("int_methods", SOURCE, &cases);
}
