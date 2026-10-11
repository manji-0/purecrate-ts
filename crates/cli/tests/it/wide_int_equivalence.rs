//! `u128` / `i128` match Rust at the edges of 64 and 128 bits, overflow
//! panics included, and byte strings read the same bytes.

use crate::support;

purecrate_canon::fixture!(mod wide_int = "fixtures/wide_int.rs");

const U: &[u128] = &[0, 1, 2, u64::MAX as u128, 1 << 64, (1 << 64) + 1, 1 << 127, u128::MAX - 1, u128::MAX];
const I: &[i128] = &[i128::MIN, i128::MIN + 1, -(1 << 64), -1, 0, 1, 1 << 64, i128::MAX - 1, i128::MAX];

#[test]
fn wide_ints_match_rust() {
    support::equivalence("wide_int", wide_int::SOURCE, |cases| {
        for &a in U {
            for &b in U {
                cases.push(case!(wide_int::add(a, b)));
                cases.push(case!(wide_int::mul(a, b)));
                cases.push(case!(wide_int::methods(a, b)));
            }
            for n in [0u32, 1, 63, 64, 127, 128] {
                cases.push(case!(wide_int::bitwise(a, !a, n)));
            }
            cases.push(case!(wide_int::halves(a)));
            cases.push(case!(wide_int::signed(a)));
            cases.push(case!(wide_int::encoded(a)));
        }
        for &a in I {
            for &b in I {
                cases.push(case!(wide_int::sub(a, b)));
                cases.push(case!(wide_int::div_rem(a, b)));
            }
            for n in [0u32, 1, 127, 128] {
                cases.push(case!(wide_int::signed_shift(a, n)));
            }
            cases.push(case!(wide_int::signed_methods(a)));
            cases.push(case!(wide_int::signed_ops(a)));
        }
        for (hi, lo) in [(0u64, 0u64), (1, u64::MAX), (u64::MAX, 1), (u64::MAX, u64::MAX)] {
            cases.push(case!(wide_int::joined(hi, lo)));
        }
        for (a, b) in [(i64::MIN, 0u8), (-1, 255), (i64::MAX, 7)] {
            cases.push(case!(wide_int::widened(a, b)));
        }
        for s in [
            "0",
            "-1",
            "+1",
            "340282366920938463463374607431768211455",
            "340282366920938463463374607431768211456",
            "170141183460469231731687303715884105727",
            "-170141183460469231731687303715884105728",
            "-170141183460469231731687303715884105729",
            "",
            "1_0",
        ] {
            cases.push(case!(wide_int::parsed(s)));
        }
        for i in [0usize, 1, 2, 3] {
            cases.push(case!(wide_int::raw(i)));
        }
        cases.push(case!(wide_int::bytes_len()));
        cases.push(case!(wide_int::signed_consts()));
    });
}
