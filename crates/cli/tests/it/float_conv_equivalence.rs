//! Float methods and conversions match Rust bit for bit: `round` at every
//! half, `-0.0` kept, NaN and the infinities, saturation at each integer
//! type's bounds, and an integer rounded once to the nearest float (ties to
//! even), wide ones included.

use crate::support;

purecrate_canon::fixture!(mod float_conv = "fixtures/float_conv.rs");

const F: &[f64] = &[
    0.0,
    -0.0,
    0.4,
    -0.4,
    0.5,
    -0.5,
    1.5,
    -1.5,
    2.5,
    -2.5,
    0.49999999999999994,
    -0.49999999999999994,
    4503599627370495.5,
    -4503599627370495.5,
    4503599627370496.0,
    9007199254740993.0,
    1e300,
    -1e300,
    127.9,
    128.0,
    -128.9,
    -129.0,
    255.5,
    256.0,
    2147483647.5,
    -2147483648.9,
    4294967295.9,
    4294967296.0,
    9007199254740991.0,
    9223372036854775807.0,
    -9223372036854775808.0,
    18446744073709551615.0,
    1.7e38,
    3.4028235e38,
    3.5e38,
    -3.5e38,
    1e-46,
    f64::MIN_POSITIVE,
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
];

#[test]
fn float_conversions_match_rust() {
    support::equivalence("float_conv", float_conv::SOURCE, |cases| {
        cases.push(case!(float_conv::consts()));
        for &x in F {
            let y = x as f32;
            cases.push(case!(float_conv::rounded(x)));
            cases.push(case!(float_conv::rounded32(y)));
            cases.push(case!(float_conv::tests(x)));
            cases.push(case!(float_conv::tests32(y)));
            cases.push(case!(float_conv::narrowed(x)));
            cases.push(case!(float_conv::not_infinite(x)));
            for k in [0i32, 1] {
                cases.push(case!(float_conv::kept_test(x, k)));
            }
            for b in [false, true] {
                cases.push(case!(float_conv::lint_shapes(y, x, b, 7)));
            }
            for places in [5, 6] {
                cases.push(case!(float_conv::scaled(x, places)));
            }
            cases.push(case!(float_conv::f64_as_i8(x)));
            cases.push(case!(float_conv::f32_as_i8(y)));
            cases.push(case!(float_conv::f64_as_i16(x)));
            cases.push(case!(float_conv::f32_as_i16(y)));
            cases.push(case!(float_conv::f64_as_i32(x)));
            cases.push(case!(float_conv::f32_as_i32(y)));
            cases.push(case!(float_conv::f64_as_i64(x)));
            cases.push(case!(float_conv::f32_as_i64(y)));
            cases.push(case!(float_conv::f64_as_i128(x)));
            cases.push(case!(float_conv::f32_as_i128(y)));
            cases.push(case!(float_conv::f64_as_u8(x)));
            cases.push(case!(float_conv::f32_as_u8(y)));
            cases.push(case!(float_conv::f64_as_u16(x)));
            cases.push(case!(float_conv::f32_as_u16(y)));
            cases.push(case!(float_conv::f64_as_u32(x)));
            cases.push(case!(float_conv::f32_as_u32(y)));
            cases.push(case!(float_conv::f64_as_u64(x)));
            cases.push(case!(float_conv::f32_as_u64(y)));
            cases.push(case!(float_conv::f64_as_u128(x)));
            cases.push(case!(float_conv::f32_as_u128(y)));
            if (x as usize) < 1 << 53 {
                cases.push(case!(float_conv::f64_as_usize(x)));
            }
            if (y as usize) < 1 << 53 {
                cases.push(case!(float_conv::f32_as_usize(y)));
            }
        }
        for (x, n, b) in [(0.1f32, i32::MIN, u16::MAX), (-0.0, -1, 0), (f32::NAN, i32::MAX, 7)] {
            cases.push(case!(float_conv::widened(x, n, b)));
        }
        // Each width's edges, 2^24 and 2^53 with their neighbours (ties to
        // even), and values that round once differently from twice.
        let wide: &[i128] = &[
            0,
            1,
            -1,
            16777215,
            16777216,
            16777217,
            16777219,
            (1 << 53) - 1,
            1 << 53,
            (1 << 53) + 1,
            (1 << 53) + 3,
            (1 << 54) + 2,
            i64::MIN as i128,
            i64::MAX as i128,
            u64::MAX as i128,
            i32::MIN as i128,
            u32::MAX as i128,
            0x0000_0100_0000_1000_0000_0001,
            0x0000_0100_0000_1000_0000_0000,
            (1 << 64) + (1 << 40) + 1,
            i128::MIN,
            i128::MAX,
        ];
        for &n in wide {
            if let Ok(v) = i32::try_from(n) {
                cases.push(case!(float_conv::i32_as_f32(v)));
            }
            if let Ok(v) = i64::try_from(n) {
                cases.push(case!(float_conv::i64_as_f64(v)));
                cases.push(case!(float_conv::i64_as_f32(v)));
            }
            {
                let v = n;
                cases.push(case!(float_conv::i128_as_f64(v)));
                cases.push(case!(float_conv::i128_as_f32(v)));
            }
            if let Ok(v) = u32::try_from(n) {
                cases.push(case!(float_conv::u32_as_f32(v)));
            }
            if let Ok(v) = u64::try_from(n) {
                cases.push(case!(float_conv::u64_as_f64(v)));
                cases.push(case!(float_conv::u64_as_f32(v)));
            }
            if let Ok(v) = u128::try_from(n) {
                cases.push(case!(float_conv::u128_as_f64(v)));
                cases.push(case!(float_conv::u128_as_f32(v)));
            }
            if let Some(v) = usize::try_from(n).ok().filter(|&v| v < 1 << 53) {
                cases.push(case!(float_conv::usize_as_f64(v)));
                cases.push(case!(float_conv::usize_as_f32(v)));
            }
            if let Ok(v) = u128::try_from(n) {
                cases.push(case!(float_conv::u128_as_f32(v | (u128::MAX << 100))));
            }
        }
    });
}
