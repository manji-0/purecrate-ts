//! Building a `String` (design/01 §7.14): `String::new`, `push` and
//! `push_str` on a local, `collect::<String>()` over `char`s, on empty,
//! ASCII, two- to four-byte, and surrogate-edge text.

use crate::support;

purecrate_canon::fixture!(mod strbuild = "fixtures/string_build.rs");

const TEXTS: [&str; 8] = ["", "a", "abc", "é", "日本", "😀", "a😀é日", "\u{ffff}\u{10000}"];

#[test]
fn built_strings_match_rust() {
    support::equivalence("string_build", strbuild::SOURCE, |cases| {
        for s in TEXTS {
            cases.push(case!(strbuild::shouted(s)));
            cases.push(case!(strbuild::every_other(s)));
            cases.push(case!(strbuild::from_chars(s.chars().collect::<Vec<_>>())));
            for keep in [false, true] {
                cases.push(case!(strbuild::prefixed(s, keep)));
            }
            for n in [0u32, 1, 3] {
                cases.push(case!(strbuild::built_len(s, n)));
            }
        }
        for sep in ['.', '日', '😀'] {
            for pieces in [vec![], vec!["a"], vec!["a", "", "😀é"]] {
                let pieces: Vec<String> = pieces.into_iter().map(String::from).collect();
                cases.push(case!(strbuild::joined(pieces.clone(), sep)));
            }
        }
    });
}
