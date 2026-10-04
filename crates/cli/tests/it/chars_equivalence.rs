//! `char` (design/01 §6): literal and range patterns, ordering by code
//! point, the ASCII methods, `to_digit` / `is_digit` with every radix edge,
//! and the conversions `u32::from`, `u64::from`, `char::from(u8)`,
//! `char::from_u32` agree between Rust and the generated package at every
//! UTF-8 length boundary and around the surrogate gap.

use crate::support;

purecrate_canon::fixture!(mod chars = "fixtures/chars.rs");

const CHARS: [char; 32] = [
    '\0', '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '!', '"', '\'', '/', '0', '9', ':', '@', 'A', 'F', 'G', 'Z', '[',
    '\\', '`', 'a', 'f', 'z', '{', '~', '\u{7f}', '\u{80}', 'é', '\u{7ff}', '\u{800}',
];

const WIDE: [char; 8] = ['\u{d7ff}', '\u{e000}', '\u{ffff}', '\u{10000}', '😀', '\u{10fffe}', '\u{10ffff}', '٣'];

#[test]
fn chars_match_rust() {
    let all: Vec<char> = CHARS.iter().chain(WIDE.iter()).copied().collect();
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for &c in &all {
            cases.push(case!(chars::classify(c)));
            cases.push(case!(chars::ascii_bits(c)));
            cases.push(case!(chars::upper(c)));
            cases.push(case!(chars::utf8_len(c)));
            cases.push(case!(chars::code(c)));
            cases.push(case!(chars::wide_code(c)));
            cases.push(case!(chars::plane(c)));
            cases.push(case!(chars::escapes(c)));
            for radix in [0u32, 1, 2, 8, 10, 16, 36, 37] {
                cases.push(case!(chars::digit(c, radix)));
                cases.push(case!(chars::is_digit_in(c, radix)));
            }
            for &d in &all {
                cases.push(case!(chars::order(c, d)));
                cases.push(case!(chars::at_most(c, d)));
                cases.push(case!(chars::same_letter(c, d)));
            }
        }
        for b in 0u8..=255 {
            cases.push(case!(chars::from_byte(b)));
        }
        for n in [
            0u32,
            0x7f,
            0x80,
            0xd7ff,
            0xd800,
            0xdbff,
            0xdc00,
            0xdfff,
            0xe000,
            0xffff,
            0x10000,
            0x10ffff,
            0x110000,
            u32::MAX,
        ] {
            cases.push(case!(chars::from_code(n)));
        }
        cases
    });
    assert!(cases.iter().any(|c| c.rust.starts_with("panic(")), "a bad radix must panic");
    support::assert_equivalent("chars", chars::SOURCE, &cases);
}
