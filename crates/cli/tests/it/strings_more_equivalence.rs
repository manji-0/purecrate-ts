//! `to_lowercase` / `to_uppercase`, `split` with a `&str`, `split_whitespace`,
//! `lines`, `join`, `repeat`, and `to_string` match Rust (design/01 §7.21):
//! final sigma, `ß`, `İ`, and ligatures; empty separators and inputs; every
//! White_Space code point and the near misses U+FEFF, U+180E, U+200B; and
//! `\r` and `\n` in each order.

use uuid::Uuid;

use crate::support;

purecrate_canon::fixture!(mod more = "fixtures/strings_more.rs");

const TEXTS: &[&str] = &[
    "",
    " ",
    "a",
    "ab",
    "a, b, c",
    ", a,, b, ",
    "a--b----c--",
    "ΟΔΟΣ ΟΔΟΣ.",
    "Σ",
    "ΑΣ\u{301}Β ὈΔΥΣΣΕΎΣ",
    "straße İstanbul ﬁx ŉ ǅ",
    "ÉCOLE école 日本 😀x",
    "\u{10400}\u{10428}",
    "\t\n\u{b}\u{c}\r x \u{20}",
    "\u{85}a\u{85}b\u{a0}\u{1680}c",
    "\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}y",
    "\u{2028}z\u{2029}\u{202f}\u{205f}\u{3000}w",
    "\u{feff}z\u{180e}y\u{200b}x",
    "a\n",
    "a\r\nb",
    "a\rb",
    "\n",
    "a\n\n",
    "a\r",
    "a\n\r",
    "\r\n",
    "\r",
    "\r\r\n",
    "foo\nbar\n\r\nbaz\r",
    " x \r\n\r\n y\t\n",
];

const SEPS: &[&str] = &["", ",", ", ", "--", "-", "a", "ΟΣ", "😀", "\n", "\r\n", "zz"];

#[test]
fn string_methods_match_rust() {
    support::equivalence("strings_more", more::SOURCE, |cases| {
        for &s in TEXTS {
            cases.push(case!(more::cased(s)));
            cases.push(case!(more::cased_owned(String::from(s))));
            cases.push(case!(more::by_comma(s)));
            cases.push(case!(more::words(s)));
            cases.push(case!(more::word_stats(s)));
            cases.push(case!(more::lines(s)));
            cases.push(case!(more::line_stats(s)));
            cases.push(case!(more::rejoined(s)));
            cases.push(case!(more::joined_strs(&vec![s, s, "x"])));
            for &p in SEPS {
                cases.push(case!(more::by_str(s, p)));
                cases.push(case!(more::by_owned(s, String::from(p))));
                cases.push(case!(more::walked(s, p)));
            }
            for n in [0usize, 1, 3] {
                cases.push(case!(more::repeated(s, n)));
            }
            cases.push(case!(more::shown_text(s, String::from(s))));
        }
        for xs in [vec![], vec![""], vec!["", ""], vec!["a"], vec!["a", "é", "😀"], vec!["x", "", "y"]] {
            let owned: Vec<String> = xs.iter().map(|x| String::from(*x)).collect();
            for sep in ["", ",", ", ", "😀"] {
                cases.push(case!(more::joined(owned.clone(), sep)));
                cases.push(case!(more::joined_slice(&owned, String::from(sep))));
            }
            cases.push(case!(more::joined_strs(&xs.clone())));
        }
        grid!(cases, more::shown; n in [0u32, 7, u32::MAX], m in [i64::MIN, -12, 0, i64::MAX], b in [false, true], c in ['a', 'é', '😀', '\u{10ffff}']);
        for u in [Uuid::nil(), Uuid::max(), Uuid::from_u128(0x67e5_5044_10b1_426f_9247_bb68_0e5f_e0c8)] {
            cases.push(case!(more::shown_id(u)));
        }
        grid!(cases, more::shown_wide; a in [i8::MIN, 0, i8::MAX], b in [0, u128::MAX], c in [i128::MIN, -1, i128::MAX], d in [0usize, 9_007_199_254_740_991]);
    });
}
