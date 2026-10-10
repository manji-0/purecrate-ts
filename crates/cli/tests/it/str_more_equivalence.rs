//! The `str` methods added for header parsing match Rust on ASCII and
//! non-ASCII text, every whitespace Rust and JS disagree on, empty
//! patterns, and byte offsets past multi-byte characters.

use crate::support;

purecrate_canon::fixture!(mod str_more = "fixtures/str_more.rs");

const TEXTS: &[&str] = &[
    "",
    " ",
    "a",
    "Text/HTML; Q=0.5",
    "  \t gzip ;q=1 \t",
    "\u{85}x\u{85}",
    "\u{feff}x\u{feff}",
    "\u{a0}\u{3000}é\u{2028}",
    "ÉCOLE école",
    "--a--b--",
    "😀,é;;ü,",
    "a1b22c",
];

#[test]
fn str_methods_match_rust() {
    support::equivalence("str_more", str_more::SOURCE, |cases| {
        for &s in TEXTS {
            cases.push(case!(str_more::folded(s)));
            cases.push(case!(str_more::trimmed(s)));
            cases.push(case!(str_more::trimmed_ows(s)));
            cases.push(case!(str_more::pieces(s)));
            cases.push(case!(str_more::collected(s)));
            for c in ['-', ' ', '😀', '\u{85}'] {
                cases.push(case!(str_more::trimmed_by(s, c)));
            }
            for p in ["", "-", "--", "é", "😀,", "x"] {
                cases.push(case!(str_more::trimmed_str(s, p)));
            }
            for (c, p) in [('a', "b"), ('é', "é"), ('😀', ";"), (',', ""), ('\u{feff}', "q=")] {
                cases.push(case!(str_more::found(s, c, p)));
            }
        }
    });
}
