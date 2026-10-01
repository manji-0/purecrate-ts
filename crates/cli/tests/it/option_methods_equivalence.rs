//! `Option::unwrap_or`, `ok_or`, and `map`: an argument that overflows is
//! evaluated even on `Some` (as Rust evaluates it), `map` calls its closure
//! only on `Some`, chains, `?` on `ok_or`, and falsy payloads.

use crate::support;


purecrate_canon::fixture!(mod option_methods = "fixtures/option_methods.rs");

const SOURCE: &str = option_methods::SOURCE;

#[test]
fn generated_option_methods_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for x in [None, Some(0u8), Some(7)] {
            for d in [0u8, 254, 255] {
                cases.push(case!(option_methods::or_default(x, d)));
                cases.push(case!(option_methods::eager(x, d)));
            }
        }
        for x in [None, Some(0i64), Some(-5), Some(i64::MAX)] {
            cases.push(case!(option_methods::required(x)));
            for b in [None, Some(1i64), Some(i64::MAX)] {
                cases.push(case!(option_methods::total(x, b)));
            }
        }
        for x in [None, Some(0i32), Some(3), Some(i32::MAX)] {
            for n in [0, i32::MAX] {
                cases.push(case!(option_methods::doubled(x, n)));
            }
        }
        for x in [None, Some(0u32), Some(9), Some(u32::MAX)] {
            cases.push(case!(option_methods::chain(x)));
        }
        for x in [None, Some(false), Some(true)] {
            cases.push(case!(option_methods::falsy(x)));
        }
        cases
    });
    support::assert_equivalent("option_methods", SOURCE, &cases);
}

/// `let x = opt.ok_or(e)?` prints as a guard, with `e` bound before the
/// test (it is eager), not as an inline function building a `Result`.
#[test]
fn ok_or_then_try_is_a_guard() {
    let krate = purecrate_syntax::parse_source("guard", SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&purecrate_check::prune_unreachable(&typed));
    let src = &pkg.files.iter().find(|f| f.stem == "total").expect("total").source;
    assert!(!src.contains("(() =>"), "{src}");
    let arg = src.find("const $arg").expect("the argument is bound");
    let guard = src.find("=== null)) return Result.err($arg").expect("one-line guard");
    assert!(arg < guard, "{src}");
}
