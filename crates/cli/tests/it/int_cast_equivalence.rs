//! `x as T` between integer types matches Rust's wrapping at every edge of
//! every source width: each type's bounds, the powers of two around 8, 16,
//! 32, 53, and 64 bits, and their neighbours. To `usize`, only results
//! below 2^53: above, TS panics where Rust holds the value (design/01 §3),
//! as for every `usize` that large.

use crate::support;

purecrate_canon::fixture!(mod int_cast = "fixtures/int_cast.rs");

/// The edges every width is tried at, kept where `T` holds them.
const EDGES: &[i128] = &[
    i64::MIN as i128,
    i64::MIN as i128 + 1,
    -(1 << 53) - 1,
    -(1 << 53),
    -(1 << 32) - 1,
    -(1 << 31) - 1,
    i32::MIN as i128,
    -65537,
    -32769,
    i16::MIN as i128,
    -257,
    -129,
    -128,
    -127,
    -1,
    0,
    1,
    31,
    127,
    128,
    255,
    256,
    32767,
    32768,
    65535,
    65536,
    (1 << 31) - 1,
    1 << 31,
    (1 << 32) - 1,
    1 << 32,
    (1 << 53) - 1,
    1 << 53,
    i64::MAX as i128,
    1 << 63,
    u64::MAX as i128,
];

macro_rules! each {
    ($cases:ident, $t:ty, $f:ident) => {
        for x in EDGES.iter().filter_map(|&v| <$t>::try_from(v).ok()) {
            $cases.push(case!(int_cast::$f(x)));
        }
    };
    ($cases:ident, $t:ty, $f:ident, usize) => {
        for x in EDGES.iter().filter_map(|&v| <$t>::try_from(v).ok()).filter(|&x| (x as usize) < 1 << 53) {
            $cases.push(case!(int_cast::$f(x)));
        }
    };
}

#[test]
fn int_casts_match_rust() {
    support::equivalence("int_cast", int_cast::SOURCE, |cases| {
        each!(cases, i8, i8_as_u8);
        each!(cases, i8, i8_as_u16);
        each!(cases, i8, i8_as_u32);
        each!(cases, i8, i8_as_u64);
        each!(cases, i8, i8_as_usize, usize);
        each!(cases, i16, i16_as_i8);
        each!(cases, i16, i16_as_u8);
        each!(cases, i16, i16_as_u16);
        each!(cases, i16, i16_as_u32);
        each!(cases, i16, i16_as_u64);
        each!(cases, i16, i16_as_usize, usize);
        each!(cases, i32, i32_as_i8);
        each!(cases, i32, i32_as_i16);
        each!(cases, i32, i32_as_u8);
        each!(cases, i32, i32_as_u16);
        each!(cases, i32, i32_as_u32);
        each!(cases, i32, i32_as_u64);
        each!(cases, i32, i32_as_usize, usize);
        each!(cases, i64, i64_as_i8);
        each!(cases, i64, i64_as_i16);
        each!(cases, i64, i64_as_i32);
        each!(cases, i64, i64_as_u8);
        each!(cases, i64, i64_as_u16);
        each!(cases, i64, i64_as_u32);
        each!(cases, i64, i64_as_u64);
        each!(cases, i64, i64_as_usize, usize);
        each!(cases, u8, u8_as_i8);
        each!(cases, u8, u8_as_usize, usize);
        each!(cases, u16, u16_as_i8);
        each!(cases, u16, u16_as_i16);
        each!(cases, u16, u16_as_u8);
        each!(cases, u16, u16_as_usize, usize);
        each!(cases, u32, u32_as_i8);
        each!(cases, u32, u32_as_i16);
        each!(cases, u32, u32_as_i32);
        each!(cases, u32, u32_as_u8);
        each!(cases, u32, u32_as_u16);
        each!(cases, u32, u32_as_usize, usize);
        each!(cases, u64, u64_as_i8);
        each!(cases, u64, u64_as_i16);
        each!(cases, u64, u64_as_i32);
        each!(cases, u64, u64_as_i64);
        each!(cases, u64, u64_as_u8);
        each!(cases, u64, u64_as_u16);
        each!(cases, u64, u64_as_u32);
        each!(cases, u64, u64_as_usize, usize);
        // A `usize` argument above 2^53−1 cannot reach TS at all.
        for x in EDGES.iter().filter_map(|&v| usize::try_from(v).ok()).filter(|&x| x < 1 << 53) {
            cases.push(case!(int_cast::usize_as_i8(x)));
            cases.push(case!(int_cast::usize_as_i16(x)));
            cases.push(case!(int_cast::usize_as_i32(x)));
            cases.push(case!(int_cast::usize_as_i64(x)));
            cases.push(case!(int_cast::usize_as_u8(x)));
            cases.push(case!(int_cast::usize_as_u16(x)));
            cases.push(case!(int_cast::usize_as_u32(x)));
            cases.push(case!(int_cast::usize_as_u64(x)));
        }
        for v in [0u64, 1, 0x0123_4567_89AB_CDEF, u64::MAX, 1 << 63] {
            cases.push(case!(int_cast::bytes(v)));
            for shift in [0u32, 5, 35, 59, 60] {
                cases.push(case!(int_cast::digit(v, shift)));
            }
        }
    });
}
