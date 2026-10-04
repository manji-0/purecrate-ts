//! The integer methods against Rust at every type's edges: `min`, `max`,
//! `abs` (panicking on `MIN`), `pow` (panicking on overflow), and the
//! `checked_*`, `saturating_*`, and `wrapping_*` forms, on `i8`, `u8`,
//! `i16`, `i32`, `u32`, `i64`, `u64`, and `usize`.

use crate::support;

purecrate_canon::fixture!(mod int_methods = "fixtures/int_methods.rs");

#[test]
fn generated_int_methods_match_rust() {
    support::equivalence("int_methods", int_methods::SOURCE, |cases| {
        for a in [0u8, 1, 2, 3, 15, 16, 128, 255] {
            for b in [0u8, 1, 127, 255] {
                cases.push(case!(int_methods::clamp_add(a, b, 200)));
            }
            for e in [0u32, 1, 2, 5, 7, 8, 9, 64, u32::MAX] {
                cases.push(case!(int_methods::power_u8(a, e)));
            }
        }
        grid!(
            cases, int_methods::power_i8;
            a in [i8::MIN, -128 + 1, -3, -2, -1, 0, 1, 2, 11, 12, i8::MAX],
            e in [0u32, 1, 2, 3, 5, 6, 7, 8, 9, 100, u32::MAX],
        );
        grid!(
            cases, [int_methods::saturating, int_methods::wrapping];
            a in [i8::MIN, -128 + 1, -3, -2, -1, 0, 1, 2, 11, 12, i8::MAX], b in [i8::MIN, -1, 0, 1, 2, i8::MAX]
        );
        grid!(
            cases, [int_methods::spread, int_methods::checked, int_methods::checked_div, int_methods::wrapping_div];
            a in [i32::MIN, i32::MIN + 1, -7, -1, 0, 1, 7, 46_341, i32::MAX],
            b in [i32::MIN, -2, -1, 0, 1, 2, 46_341, i32::MAX],
        );
        grid!(
            cases, int_methods::power;
            a in [i32::MIN, i32::MIN + 1, -7, -1, 0, 1, 7, 46_341, i32::MAX], e in [0u32, 1, 2, 30, 31, 32]
        );
        for a in [-2i32, 2] {
            cases.push(case!(int_methods::power(a, 31)));
        }
        grid!(cases, int_methods::magnitude; a in [i16::MIN, -1, 0, 5, i16::MAX]);
        grid!(
            cases, int_methods::wide;
            a in [i64::MIN, -3, 0, 3, 3_037_000_500, i64::MAX], b in [i64::MIN, -1, 0, 3, 3_037_000_500, i64::MAX]
        );
        grid!(cases, int_methods::unsigned_wide; a in [0u64, 1, u64::MAX], b in [0u64, 1, u64::MAX]);
        grid!(cases, int_methods::remaining; len in [0usize, 1, 10], used in [0usize, 1, 10, 11]);
        grid!(cases, int_methods::unsigned_neg; a in [0u32, 1, u32::MAX]);
    });
}

/// Random operands, half at the edges of each width: 400 draws per function.
#[test]
fn random_int_methods_match_rust() {
    support::equivalence("int_methods_random", int_methods::SOURCE, |cases| {
        let mut rng = support::Rng::new(0x01a7_0001);
        for _ in 0..400 {
            let i8s = [rng.edgy(8, true) as i8, rng.edgy(8, true) as i8];
            let u8s = [rng.edgy(8, false) as u8, rng.edgy(8, false) as u8, rng.edgy(8, false) as u8];
            let i32s = [rng.edgy(32, true) as i32, rng.edgy(32, true) as i32];
            let i64s = [rng.edgy(64, true) as i64, rng.edgy(64, true) as i64];
            let u64s = [rng.edgy(64, false) as u64, rng.edgy(64, false) as u64];
            let e = rng.pick(&[0u32, 1, 2, 3, 7, 8, 15, 16, 31, 32, 63, 64, 65]);
            cases.push(case!(int_methods::saturating(i8s[0], i8s[1])));
            cases.push(case!(int_methods::wrapping(i8s[0], i8s[1])));
            cases.push(case!(int_methods::power_i8(i8s[0], e)));
            cases.push(case!(int_methods::power_u8(u8s[0], e)));
            cases.push(case!(int_methods::clamp_add(u8s[0], u8s[1], u8s[2])));
            cases.push(case!(int_methods::checked(i32s[0], i32s[1])));
            cases.push(case!(int_methods::checked_div(i32s[0], i32s[1])));
            cases.push(case!(int_methods::wrapping_div(i32s[0], i32s[1])));
            cases.push(case!(int_methods::spread(i32s[0], i32s[1])));
            cases.push(case!(int_methods::power(i32s[0], e)));
            cases.push(case!(int_methods::wide(i64s[0], i64s[1])));
            cases.push(case!(int_methods::unsigned_wide(u64s[0], u64s[1])));
        }
    });
}
