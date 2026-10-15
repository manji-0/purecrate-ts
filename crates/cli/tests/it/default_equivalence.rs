//! `#[derive(Default)]` and `impl Default` are the companion's `default`,
//! called by `X::default()`, `Default::default()`, and
//! `..Default::default()`; each field's default has its type's TS form.

use crate::support;

purecrate_canon::fixture!(mod default = "fixtures/default.rs");

#[test]
fn generated_default_matches_rust() {
    support::equivalence("default", default::SOURCE, |cases| {
        cases.push(case!(default::all_widths()));
        cases.push(case!(default::all_settings()));
        cases.push(case!(default::no_meters()));
        cases.push(case!(default::default_mode()));
        grid!(cases, default::window_of; w in [0u32, 1, u32::MAX]);
        grid!(cases, default::taller; w in [0u32, 1, 2]);
        grid!(cases, default::strict_of; d in [0i32, 1]);
        for name in ["", "x", "ünï😀"] {
            cases.push(case!(default::named(name.to_string())));
        }
        cases.push(case!(default::is_default(default::all_widths())));
        cases.push(case!(default::is_default(default::Widths { k: 1, ..default::all_widths() })));
        cases.push(case!(default::is_default(default::Widths { j: u128::MAX, ..default::all_widths() })));
        for xs in [vec![], vec![0u8], vec![1, 0, 2]] {
            cases.push(case!(default::count(xs.clone())));
        }
        for xs in [vec![], vec![i64::MIN], vec![3i64, 4]] {
            cases.push(case!(default::first_or_default(xs.clone())));
        }
    });
}

#[test]
fn defaults_print_in_their_type_s_form() {
    let krate = purecrate_syntax::parse_source("default", default::SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    let widths = file("widths");
    for field in ["a: 0 as I8", "d: 0n as I64", "e: 0n as I128", "i: 0n as U64", "j: 0n as U128", "k: 0 as Usize"] {
        assert!(widths.contains(field), "{field}: {widths}");
    }
    assert!(file("mode").contains("default: (): Mode =>"), "{}", file("mode"));
}
