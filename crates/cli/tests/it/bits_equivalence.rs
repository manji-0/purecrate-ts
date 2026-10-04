//! Bitwise operators and shifts: `& | ^ !` wrap to the width and never
//! panic; `<< >>` panic on an amount outside `0..bits`, compared as the whole
//! value of its own type (negative and 2^32 + 1 included). Every case in
//! `fixtures/bits.rs` agrees between Rust and the generated package.

use crate::support;

purecrate_canon::fixture!(mod bits = "fixtures/bits.rs");

#[test]
fn generated_bit_operations_match_rust() {
    support::equivalence("bits", bits::SOURCE, |cases| {
        let u8s = [0u8, 1, 0x0f, 0x80, 0xaa, 0xff];
        let i8s = [0i8, 1, -1, 0x55, i8::MIN, i8::MAX];
        let i16s = [0i16, -1, 0x1234, i16::MIN, i16::MAX];
        let u16s = [0u16, 1, 0xff00, u16::MAX];
        let i32s = [0i32, 1, -1, 0x7fff_0000, i32::MIN, i32::MAX];
        let u32s = [0u32, 1, 0x8000_0000, 0xdead_beef, u32::MAX];
        let i64s = [0i64, -1, 0x7fff_ffff_ffff, i64::MIN, i64::MAX];
        let u64s = [0u64, 1, 1 << 53, 0xdead_beef_cafe_babe, u64::MAX];
        grid!(cases, bits::and_u8; a in u8s, b in u8s);
        grid!(cases, bits::not_u8; a in u8s);
        grid!(cases, bits::or_i8; a in i8s, b in i8s);
        grid!(cases, bits::not_i8; a in i8s);
        grid!(cases, bits::xor_i16; a in i16s, b in i16s);
        grid!(cases, bits::toggles; a in i16s);
        grid!(cases, bits::ops_u16; a in u16s, b in u16s);
        grid!(cases, bits::ops_i32; a in i32s, b in i32s);
        grid!(cases, bits::not_i32; a in i32s);
        grid!(cases, bits::ops_u32; a in u32s, b in u32s);
        grid!(cases, [bits::not_u32, bits::popcount, bits::reverse_bytes]; a in u32s);
        grid!(cases, bits::ops_i64; a in i64s, b in i64s);
        grid!(cases, bits::not_i64; a in i64s);
        grid!(cases, [bits::ops_u64, bits::allows]; a in u64s, b in u64s);
        grid!(cases, [bits::not_u64, bits::literals]; a in u64s);
        grid!(cases, bits::not_bool; a in [false, true], x in [0u16, 1]);
        for n in [0u32, 1, 7, 8, 9, 31, 32, 33, u32::MAX] {
            for a in [1u8, 0x81, 0xff] {
                cases.push(case!(bits::shl_u8(a, n)));
            }
        }
        for n in [0u8, 1, 6, 7, 8, 255] {
            for a in [1i8, -1, i8::MIN, i8::MAX] {
                cases.push(case!(bits::shl_i8(a, n)));
            }
        }
        for n in [0i64, 1, 7, 8, -1, i64::MIN, 1 << 32, (1 << 32) + 1] {
            for a in [1i8, -1, i8::MIN, i8::MAX] {
                cases.push(case!(bits::shr_i8(a, n)));
            }
        }
        for n in [0u64, 15, 16, (1 << 32) + 1, u64::MAX] {
            cases.push(case!(bits::shl_u16(0x8001u16, n)));
        }
        for n in [0i32, 1, 30, 31, 32, -1, i32::MIN] {
            for a in [1i32, -1, i32::MIN, i32::MAX, 0x4000_0000] {
                cases.push(case!(bits::shl_i32(a, n)));
                cases.push(case!(bits::shr_i32(a, n)));
            }
        }
        for n in [0u8, 1, 31, 32, 255] {
            for a in [1u32, 0x8000_0001, u32::MAX] {
                cases.push(case!(bits::shl_u32(a, n)));
                cases.push(case!(bits::shr_u32(a, n)));
            }
        }
        for n in [0u32, 1, 62, 63, 64, u32::MAX] {
            for a in [1i64, -1, i64::MIN, i64::MAX] {
                cases.push(case!(bits::shl_i64(a, n)));
                cases.push(case!(bits::shr_i64(a, u64::from(n))));
            }
        }
        for n in [0i8, 1, 63, 64, -1, i8::MIN] {
            for a in [1u64, u64::MAX, 1 << 63] {
                cases.push(case!(bits::shl_u64(a, n)));
            }
        }
        for n in [0usize, 1, 63, 64, (1 << 53) - 1] {
            cases.push(case!(bits::shr_u64(u64::MAX, n)));
        }
        let hmac = [
            0x1f, 0x86, 0x98, 0x69, 0x0e, 0x02, 0xca, 0x16, 0x61, 0x85, 0x50, 0xef, 0x7f, 0x19, 0xda, 0x8e, 0x94, 0x5b,
            0x55, 0x5a,
        ];
        cases.push(case!(bits::truncate(hmac.as_slice())));
        let mut high = hmac;
        high[19] = 0x0f;
        high[15] = 0xff;
        cases.push(case!(bits::truncate(high.as_slice())));
    });
}

#[test]
fn rfc_4226_truncation_example() {
    // RFC 4226 §5.4: this HMAC-SHA-1 truncates to 0x50ef7f19, 872921 mod 10^6.
    let hmac = [
        0x1f, 0x86, 0x98, 0x69, 0x0e, 0x02, 0xca, 0x16, 0x61, 0x85, 0x50, 0xef, 0x7f, 0x19, 0xda, 0x8e, 0x94, 0x5b,
        0x55, 0x5a,
    ];
    assert_eq!(bits::truncate(&hmac), 0x50ef7f19);
    assert_eq!(bits::truncate(&hmac) % 1_000_000, 872921);
}
