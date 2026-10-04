//! The generated TS parses and binds as intended whatever the source spells:
//! string literals with control characters, object literals behind an erased
//! `Box`, user names that look like the printer's temporaries, and a crate type
//! named like a JS global the printer reads.

use crate::support;

purecrate_canon::fixture!(mod syntax_safety = "fixtures/syntax_safety.rs");

#[test]
fn generated_syntax_matches_rust() {
    support::equivalence("syntax_safety", syntax_safety::SOURCE, |cases| {
        cases.extend([
            case!(syntax_safety::special("q\"\\\n\r\t\u{0}\u{1b}\u{7f}\u{2028}\u{2029}é")),
            case!(syntax_safety::special("q")),
        ]);
        grid!(cases, [syntax_safety::boxed_sum, syntax_safety::some_sum, syntax_safety::boxed_effect]; a in [1i32, -4]);
        grid!(cases, [syntax_safety::lift_temp, syntax_safety::match_temp]; x in [None, Some(3i32)]);
        for i in [0usize, 3, 7] {
            cases.push(case!(syntax_safety::at(vec![10i32, 20], i)));
        }
    });
}
