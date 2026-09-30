//! String literal patterns in `match` and `matches!` (design/02 §3.5), and
//! `String::as_str` to match an owned string: every case in
//! `fixtures/str_patterns.rs` agrees between Rust and the generated package,
//! on the literals themselves and strings that share a prefix, differ in
//! normalization, or lie outside the BMP.

use crate::support;


purecrate_canon::fixture!(mod pats = "fixtures/str_patterns.rs");

const SOURCE: &str = pats::SOURCE;

const TEXTS: [&str; 16] = [
    "", "card", "credit_card", "card ", "Card", "bank", "wallet", "walle", "é", "e\u{301}", "日本", "日", "😀", "😁",
    "\u{ffff}", "a\"b\\c\n",
];

#[test]
fn string_patterns_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in TEXTS {
            cases.push(case!(pats::method_of(s)));
            cases.push(case!(pats::owned(String::from(s))));
            cases.push(case!(pats::score(s)));
        }
        cases
    });
    support::assert_equivalent("str_patterns", SOURCE, &cases);
}
