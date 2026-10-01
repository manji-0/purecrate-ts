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

/// `let x = opt.ok_or(e)?` prints as a guard. A variant literal `e` is
/// written in the `return`; a name is read in the test. A default that may
/// overflow is still bound before the test (it is eager).
#[test]
fn ok_or_then_try_is_a_guard() {
    let krate = purecrate_syntax::parse_source("guard", SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&purecrate_check::prune_unreachable(&typed));
    let src = &pkg
        .files
        .iter()
        .find(|f| f.stem == "total")
        .expect("total")
        .source;
    assert!(!src.contains("(() =>"), "{src}");
    assert!(
        src.contains("if (a === null) return Result.err({ kind: \"Amount\" })"),
        "{src}"
    );
    let or_default = &pkg
        .files
        .iter()
        .find(|f| f.stem == "or-default")
        .expect("or-default")
        .source;
    assert!(
        or_default.contains("x ?? d") || or_default.contains("x !== null"),
        "{or_default}"
    );
    assert!(!or_default.contains("$x"), "{or_default}");
    let eager = &pkg
        .files
        .iter()
        .find(|f| f.stem == "eager")
        .expect("eager")
        .source;
    assert!(
        eager.contains("$xOr") || eager.contains("Int.u8.add"),
        "{eager}"
    );
}

/// A `?` inside an expression is hoisted to one binding, tested in place,
/// and read as its payload: `$half.value` for a `Result`, `$a` for an
/// `Option`; no second binding for the payload. A call's hoisted value is
/// named after the function, apart from the next one.
#[test]
fn a_hoisted_try_binds_once() {
    let source = "pub enum E { Bad }\n\
                  pub fn half(n: i32) -> Result<i32, E> { if n % 2 == 0 { Ok(n / 2) } else { Err(E::Bad) } }\n\
                  pub fn quarter(n: i32) -> Result<i32, E> { Ok(half(half(n)?)? + 0) }\n\
                  pub fn add(a: Option<u8>, b: u8) -> Option<u8> { Some(a?.checked_add(b)? + 0) }\n";
    let krate = purecrate_syntax::parse_source("hoist", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| {
        pkg.files
            .iter()
            .find(|f| f.stem == stem)
            .expect(stem)
            .source
            .clone()
    };
    let quarter = file("quarter");
    assert!(quarter.contains("const $half = half(n);\n  if ($half.kind === \"Err\") return $half;\n  const $half2 = half($half.value);"), "{quarter}");
    assert!(!quarter.contains("$v_"), "{quarter}");
    let add = file("add");
    assert!(add.contains("if (a === null) return null;"), "{add}");
    assert!(add.contains("const $q2 = Int.u8.checkedAdd(a, b);"), "{add}");
    assert!(!add.contains("$v_") && !add.contains(".value"), "{add}");
}
