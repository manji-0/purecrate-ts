//! `impl From<S> for X` is `X.fromS`, called by `X::from`, `From::from`,
//! `into()`, and `?` across error types, which converts only on `Err`,
//! after its operand; a `?` whose error types match prints as before.

use crate::support;

purecrate_canon::fixture!(mod from = "fixtures/from.rs");

#[test]
fn generated_from_matches_rust() {
    support::equivalence("from", from::SOURCE, |cases| {
        let ids = ["", "00000000-0000-0000-0000-000000000000", "67e55044-10b1-426f-9247-bb680e5fe0c8", "nope"];
        for len in ["", "ab", "abcd", "abcde", "éé", "ééééé"] {
            for n in ["", "0", "12", "-1", "4294967295", "4294967296", "١"] {
                for id in ids {
                    cases.push(case!(from::read(len, n, id)));
                }
            }
        }
        for a in ["", "1", "-7", "x", "2147483647"] {
            for b in ["", "2", "y", "1"] {
                cases.push(case!(from::sum(a, b)));
            }
        }
        grid!(cases, [from::coded, from::same]; n in [0u8, 9, 10, 127, 128, 254, 255]);
        grid!(cases, from::mixed; a in ["", "3", "255", "256"], n in [0u8, 9, 10, 200]);
        grid!(cases, from::spelled; n in [0u8, 1, 127, 128, 255], how in [0u8, 1, 2]);
        for e in [from::LowError::Blank, from::LowError::TooLong(0)] {
            cases.push(case!(from::wrap(e.clone())));
            cases.push(case!(from::passed(e.clone())));
        }
        for s in ["", "abc", "straße", "ǅ😀"] {
            cases.push(case!(from::labelled(s.to_string())));
        }
        grid!(cases, from::numbered; n in [0i64, -1, i64::MIN, i64::MAX - 1, i64::MAX]);
    });
}

/// `?` with matching error types keeps its old output; a converting one
/// calls the member on `Err` only.
#[test]
fn from_members_and_try_print_as_documented() {
    let krate = purecrate_syntax::parse_source("from", from::SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    let app = file("app-error");
    for member in ["fromParseIntError: (", "fromUuidError: (", "fromLowError: (", "fromU8: ("] {
        assert!(app.contains(member), "{member}: {app}");
    }
    assert!(file("label").contains("fromString: ("), "{}", file("label"));
    assert!(file("label").contains("fromI64: ("), "{}", file("label"));
    let coded = file("coded");
    assert!(coded.contains("return Result.err(AppError.fromU8("), "{coded}");
    let same = file("same");
    assert!(!same.contains("AppError"), "{same}");
}
