//! `str::as_bytes` is the UTF-8 bytes on both sides (design/01 §6): empty,
//! ASCII, two-, three- and four-byte code points, and an index past the end.

use crate::support;

purecrate_canon::fixture!(mod strings = "fixtures/strings.rs");

const TEXTS: [&str; 7] =
    ["", "a@b", "é", "日本", "😀", "a😀é日", "\u{7f}\u{80}\u{7ff}\u{800}\u{ffff}\u{10000}\u{10ffff}"];

#[test]
fn utf8_bytes_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in TEXTS {
            cases.push(case!(strings::bytes(s)));
            cases.push(case!(strings::byte_len(String::from(s))));
            cases.push(case!(strings::code_points(s)));
            cases.push(case!(strings::nth_byte(s, 0usize)));
            cases.push(case!(strings::nth_byte(s, 3usize)));
        }
        cases
    });
    support::assert_equivalent("strings", strings::SOURCE, &cases);
}
