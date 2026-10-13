//! `format!` matches Rust on every integer width at its edges, `char`s
//! outside the BMP (a width counts `char`s, not UTF-16 units), each
//! alignment and fill, zero padding of negative values, escapes, and the
//! order its arguments panic in.

use uuid::Uuid;

use crate::support;

purecrate_canon::fixture!(mod format = "fixtures/format.rs");

#[test]
fn format_matches_rust() {
    support::equivalence("format", format::SOURCE, |cases| {
        cases.push(case!(format::ints(
            i8::MIN,
            u8::MAX,
            i32::MIN,
            u32::MAX,
            i64::MIN,
            u64::MAX,
            i128::MIN,
            u128::MAX,
            0
        )));
        cases.push(case!(format::ints(-1, 0, -0, 7, -7, 1, -1, 1, (1usize << 53) - 1)));
        for n in [0, 7, -7, 42, -42, 12345, -12345, 123456, i64::MIN, i64::MAX] {
            cases.push(case!(format::padded_int(n)));
        }
        for n in [0, -1, i128::MIN, i128::MAX] {
            cases.push(case!(format::padded_wide(n)));
        }
        for s in ["", "a", "ab", "abcdef", "abcdefg", "é", "😀😀", "𝄞é\u{301}"] {
            for c in ['x', '😀', '\u{301}'] {
                cases.push(case!(format::padded_text(s, c)));
            }
        }
        for (ok, n) in [(true, 0), (false, 7), (true, 999), (false, 1000), (true, u16::MAX)] {
            cases.push(case!(format::flags(ok, n)));
        }
        for s in ["", "`", "${", "\\", "a}b{c", "😀"] {
            cases.push(case!(format::escapes(String::from(s))));
        }
        for u in [Uuid::nil(), Uuid::max(), Uuid::from_u128(0x67e5_5044_10b1_426f_9247_bb68_0e5f_e0c8)] {
            cases.push(case!(format::ids(u, "id")));
        }
        for (q, p) in [(0, 0), (3, -250), (u32::MAX, i64::MAX), (2, i64::MAX)] {
            cases.push(case!(format::named(q, p)));
        }
        cases.push(case!(format::positional(1, 2)));
        for (a, b) in [(1, 2), (255, 2), (1, 128), (255, 128)] {
            cases.push(case!(format::order(a, b)));
        }
        for (y, s) in [(2026, 1), (9, 999999), (65535, 1234567)] {
            cases.push(case!(format::invoice_no(y, s)));
        }
        cases.push(case!(format::built(Vec::<u32>::new())));
        cases.push(case!(format::built(vec![1, 22, 333, 4444])));
        cases.push(case!(format::only_text()));
    });
}
