//! `impl Display` is translated to `to_string`, giving what Rust's
//! `to_string()` and `format!` give: a text fixed per value (design/01
//! §7.11), and `write!` with arguments, joined writes, and fields of crate
//! types printed through their own `Display` (§7.25), with the arguments
//! that overflow and the `fmt`s that panic in Rust's order. A `fmt` of
//! another shape, or one that prints a float, is skipped, and the crate is
//! still accepted.

use crate::support;

purecrate_canon::fixture!(mod display = "fixtures/display.rs");

#[test]
fn generated_display_matches_rust() {
    support::equivalence("display", display::SOURCE, |cases| {
        grid!(cases, [display::show_fixed, display::show_arms, display::show_bound]; c in [0u8, 1]);
        cases.push(case!(display::show_plain(3)));
        cases.push(case!(display::show_formatted(display::Formatted::N(3))));
        for cents in [0, 5, -5, 199, -12345, 100, i64::MAX, i64::MIN] {
            cases.push(case!(display::show_money(cents)));
        }
        for n in [0, -1, 7, 12345, i32::MIN, i32::MAX] {
            cases.push(case!(display::show_token(display::Token::Num(n))));
        }
        for (text, upper) in [("", true), ("é😀", false), ("{x}`${y}`", true)] {
            cases.push(case!(display::show_token(display::Token::Word { text: String::from(text), upper })));
        }
        cases.push(case!(display::show_token(display::Token::Pair('a', '😀'))));
        cases.push(case!(display::show_token(display::Token::Sep)));
        cases.push(case!(display::show_token(display::Token::Blank)));
        for (cents, n, qty) in [(250, 3, 1), (-1, i32::MAX, 0), (i64::MIN, 0, 254), (7, 1, 255)] {
            cases.push(case!(display::show_line(cents, n, qty)));
        }
        for depth in [0, 1, 3] {
            cases.push(case!(display::show_tree(depth)));
        }
        for (a, b) in [(1, 2), (0, 2), (1, 0), (0, 0), (i32::MAX, 0), (0, i32::MAX), (i32::MAX, i32::MAX)] {
            cases.push(case!(display::first_panic(a, b)));
        }
        for (a, b) in [(4, 5), (0, 5), (0, i32::MAX), (i32::MAX, 1)] {
            cases.push(case!(display::computed_first(a, b)));
        }
        cases.push(case!(display::show_empty()));
        for n in [0, -3, i32::MAX, i32::MIN / 2] {
            cases.push(case!(display::show_twice(n)));
        }
        cases.push(case!(display::skipped(0.5, -2)));
        for s in ["", "abc", "é😀ß", "`${x}`"] {
            cases.push(case!(display::show_code(String::from(s))));
            cases.push(case!(display::show_loud(String::from(s))));
            cases.push(case!(display::show_label(display::Label::Code(display::Code(String::from(s))))));
            for n in [0, 254, 255] {
                cases.push(case!(display::show_label(display::Label::Tail { text: String::from(s), n })));
            }
            cases.push(case!(display::show_label(display::Label::Pair(String::from(s), String::from(" é ")))));
        }
    });
}

/// `fmt` of a fixed text keeps §7.11's form; one with arguments is a
/// template; one that prints a float, or of another shape, is skipped.
#[test]
fn display_outside_the_subset_is_skipped() {
    let krate = purecrate_syntax::parse_source("display", display::SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    assert!(file("fixed").contains("toString: (self: Fixed): string =>"), "{}", file("fixed"));
    assert!(file("formatted").contains("return `n = ${n}`;"), "{}", file("formatted"));
    assert!(file("code").contains("toString: (self: Code): string =>"), "{}", file("code"));
    assert!(!file("rate").contains("toString"), "{}", file("rate"));
    assert!(!file("signed").contains("toString"), "{}", file("signed"));
}
