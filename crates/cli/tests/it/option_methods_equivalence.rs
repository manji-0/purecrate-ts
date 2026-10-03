//! `Option::unwrap_or`, `ok_or`, and `map`: an argument that overflows is
//! evaluated even on `Some` (as Rust evaluates it), `map` calls its closure
//! only on `Some`, chains, `?` on `ok_or`, falsy payloads, and `map(f)`
//! then `unwrap_or(d)` as one test.

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
                cases.push(case!(option_methods::bumped(x, d)));
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
        for x in [0u32, 1, 9, u32::MAX] {
            for d in [0u32, 7] {
                cases.push(case!(option_methods::doubled_less(x, d)));
            }
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
/// and read as its payload: `halfResult.value` for a `Result`, `a` for an
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
    assert!(quarter.contains("const halfResult = half(n);\n  if (halfResult.kind === \"Err\") return halfResult;\n  const halfResult2 = half(halfResult.value);"), "{quarter}");
    assert!(!quarter.contains('$'), "{quarter}");
    let add = file("add");
    assert!(add.contains("if (a === null) return null;"), "{add}");
    assert!(add.contains("const opt2 = Int.u8.checkedAdd(a, b);"), "{add}");
    assert!(!add.contains('$') && !add.contains(".value"), "{add}");
}

/// `let t = match f() { Some(t) => t, None => return .. }` holds the
/// `Option` in `t` itself, which the exit narrows: no temporary and copy.
#[test]
fn an_unwrapped_option_is_held_in_its_binding() {
    let source = "fn half(n: u32) -> Option<u32> { if n % 2 == 0 { Some(n / 2) } else { None } }\n\
                  pub fn quarter(n: u32) -> u32 {\n\
                      let h = match half(n) { Some(h) => h, None => return 0 };\n\
                      let mut q = match half(h) { Some(q) => q, None => return 1 };\n\
                      q += 1;\n\
                      q\n\
                  }\n";
    let krate = purecrate_syntax::parse_source("unwrapped", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let quarter = &pkg.files.iter().find(|f| f.stem == "quarter").expect("quarter").source;
    assert!(quarter.contains("const h = half(n);\n") && quarter.contains("if (h === null) return 0 as U32;"), "{quarter}");
    // A `let mut` keeps its own type, which a held `null` would widen.
    assert!(quarter.contains("let q: U32 = option;"), "{quarter}");
}

/// `o.map(f).unwrap_or(d)` with a name or literal `d` is one test: no
/// `Option` held between them.
#[test]
fn map_then_unwrap_or_is_one_test() {
    let krate = purecrate_syntax::parse_source("fused", SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    let falsy = file("falsy");
    assert!(falsy.contains("=> x !== null && !x;") || falsy.contains("x === null ? false"), "{falsy}");
    let bumped = file("bumped");
    assert!(bumped.contains("x !== null ? Int.u8.add(x, 1 as U8) : d"), "{bumped}");
    assert!(!bumped.contains("opt"), "{bumped}");
}
