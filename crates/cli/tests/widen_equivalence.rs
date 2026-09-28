//! Every lossless integer `From` in the subset, at both ends of the source
//! range: `fixtures/widen.rs` agrees between Rust and the generated package.

#[macro_use]
mod support;

#[allow(dead_code)]
mod widen {
    include!("fixtures/widen.rs");
}

const SOURCE: &str = include_str!("fixtures/widen.rs");

#[test]
fn generated_widening_matches_rust() {
    let cases = support::quietly(|| vec![
        case!(widen::u8_to_u16(0u8)),
        case!(widen::u8_to_u16(u8::MAX)),
        case!(widen::u8_to_u16(7u8)),
        case!(widen::u8_to_u32(0u8)),
        case!(widen::u8_to_u32(u8::MAX)),
        case!(widen::u8_to_u32(7u8)),
        case!(widen::u8_to_u64(0u8)),
        case!(widen::u8_to_u64(u8::MAX)),
        case!(widen::u8_to_u64(7u8)),
        case!(widen::u8_to_usize(0u8)),
        case!(widen::u8_to_usize(u8::MAX)),
        case!(widen::u8_to_usize(7u8)),
        case!(widen::u8_to_i16(0u8)),
        case!(widen::u8_to_i16(u8::MAX)),
        case!(widen::u8_to_i16(7u8)),
        case!(widen::u8_to_i32(0u8)),
        case!(widen::u8_to_i32(u8::MAX)),
        case!(widen::u8_to_i32(7u8)),
        case!(widen::u8_to_i64(0u8)),
        case!(widen::u8_to_i64(u8::MAX)),
        case!(widen::u8_to_i64(7u8)),
        case!(widen::u16_to_u32(0u16)),
        case!(widen::u16_to_u32(u16::MAX)),
        case!(widen::u16_to_u32(7u16)),
        case!(widen::u16_to_u64(0u16)),
        case!(widen::u16_to_u64(u16::MAX)),
        case!(widen::u16_to_u64(7u16)),
        case!(widen::u16_to_usize(0u16)),
        case!(widen::u16_to_usize(u16::MAX)),
        case!(widen::u16_to_usize(7u16)),
        case!(widen::u16_to_i32(0u16)),
        case!(widen::u16_to_i32(u16::MAX)),
        case!(widen::u16_to_i32(7u16)),
        case!(widen::u16_to_i64(0u16)),
        case!(widen::u16_to_i64(u16::MAX)),
        case!(widen::u16_to_i64(7u16)),
        case!(widen::u32_to_u64(0u32)),
        case!(widen::u32_to_u64(u32::MAX)),
        case!(widen::u32_to_u64(7u32)),
        case!(widen::u32_to_i64(0u32)),
        case!(widen::u32_to_i64(u32::MAX)),
        case!(widen::u32_to_i64(7u32)),
        case!(widen::i8_to_i16(i8::MIN)),
        case!(widen::i8_to_i16(i8::MAX)),
        case!(widen::i8_to_i16(7i8)),
        case!(widen::i8_to_i32(i8::MIN)),
        case!(widen::i8_to_i32(i8::MAX)),
        case!(widen::i8_to_i32(7i8)),
        case!(widen::i8_to_i64(i8::MIN)),
        case!(widen::i8_to_i64(i8::MAX)),
        case!(widen::i8_to_i64(7i8)),
        case!(widen::i16_to_i32(i16::MIN)),
        case!(widen::i16_to_i32(i16::MAX)),
        case!(widen::i16_to_i32(7i16)),
        case!(widen::i16_to_i64(i16::MIN)),
        case!(widen::i16_to_i64(i16::MAX)),
        case!(widen::i16_to_i64(7i16)),
        case!(widen::i32_to_i64(i32::MIN)),
        case!(widen::i32_to_i64(i32::MAX)),
        case!(widen::i32_to_i64(7i32)),
    ]);
    support::assert_equivalent("widen", SOURCE, &cases);
}
