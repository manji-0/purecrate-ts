//! The allow-listed `str` methods (design/01 §6): `len` counts UTF-8 bytes,
//! and `is_empty`, `starts_with`, `ends_with`, `contains` agree with Rust on
//! every pairing of empty, ASCII, two- to four-byte, and mixed strings,
//! including needles that share a lead byte or a surrogate half with the
//! haystack.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod strs = "fixtures/str_methods.rs");

const SOURCE: &str = strs::SOURCE;

const TEXTS: [&str; 14] = [
    "", "a", "pm_", "pm_card", "é", "e\u{301}", "日本", "日", "😀", "😁", "a😀é日", "\u{10ffff}", "\u{ffff}", "aa",
];

#[test]
fn str_methods_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in TEXTS {
            cases.push(case!(strs::len_of(s)));
            cases.push(case!(strs::owned_len(String::from(s))));
            cases.push(case!(strs::empty(s)));
            cases.push(case!(strs::classify(s)));
            cases.push(case!(strs::last_byte(s)));
            for t in TEXTS {
                cases.push(case!(strs::starts(s, t)));
                cases.push(case!(strs::ends(s, t)));
                cases.push(case!(strs::has(s, t)));
                cases.push(case!(strs::owned_needle(String::from(s), String::from(t))));
            }
        }
        cases
    });
    assert!(cases.iter().any(|c| c.rust.starts_with("panic(")), "no case panics");
    support::assert_equivalent("str_methods", SOURCE, &cases);
}
