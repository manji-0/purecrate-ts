//! `impl Display` whose text is fixed per value is translated to
//! `to_string`, giving what Rust's `to_string()` gives; one with formatting
//! arguments is skipped as before, and the crate is still accepted.

use crate::support;

purecrate_canon::fixture!(mod display = "fixtures/display.rs");

#[test]
fn generated_display_matches_rust() {
    support::equivalence("display", display::SOURCE, |cases| {
        grid!(cases, [display::show_fixed, display::show_arms, display::show_bound]; c in [0u8, 1]);
        cases.push(case!(display::show_plain(3)));
        cases.push(case!(display::show_formatted(display::Formatted::N(3))));
    });
}

#[test]
fn display_with_arguments_is_skipped() {
    let krate = purecrate_syntax::parse_source("display", display::SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    assert!(file("fixed").contains("toString: (self: Fixed): string =>"), "{}", file("fixed"));
    assert!(!file("formatted").contains("toString"), "{}", file("formatted"));
}
