//! `for c in s.chars()` (design/01 §6): empty strings, every UTF-8 length,
//! both sides of the surrogate gap and of U+E000 (where JS string order and
//! code-point order part), early `return`, `?` in the body, nesting, a
//! closure over the variable, shadowing, a string evaluated once, and a
//! panic in the body.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod for_chars = "fixtures/for_chars.rs");

const SOURCE: &str = for_chars::SOURCE;

const STRINGS: [&str; 14] = [
    "",
    "a",
    "abc",
    "a1b22c333",
    "é",
    "日本語",
    "𝄞",
    "a𝄞b",
    "\u{7F}\u{80}\u{7FF}\u{800}",
    "\u{D7FF}\u{E000}\u{FFFF}\u{10000}",
    "\u{10FFFF}\u{E000}a",
    "#x#",
    "aaaa",
    "Zz",
];

#[test]
fn for_chars_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in STRINGS {
            let owned = s.to_string();
            cases.push(case!(for_chars::digits(s)));
            cases.push(case!(for_chars::first_non_ascii(owned.clone())));
            cases.push(case!(for_chars::code_sum(&owned)));
            cases.push(case!(for_chars::utf8_len(s)));
            cases.push(case!(for_chars::all_lower(s)));
            cases.push(case!(for_chars::ascending_pairs(s)));
            cases.push(case!(for_chars::last(s)));
            cases.push(case!(for_chars::shadow(s)));
            cases.push(case!(for_chars::captured(s)));
            cases.push(case!(for_chars::from_call(s)));
            for limit in [0u8, 1, 100, 200] {
                cases.push(case!(for_chars::overflow(s, limit)));
            }
        }
        cases
    });
    for s in STRINGS {
        assert_eq!(for_chars::utf8_len(s), s.len(), "{s:?}");
    }
    support::assert_equivalent("for_chars", SOURCE, &cases);
}
