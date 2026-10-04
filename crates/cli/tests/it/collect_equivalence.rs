//! `s.split(c).collect()` and `.map(f).collect()` into a `Vec` or a
//! `Result<Vec<_>, _>`, and `str::split_once` with a `char` or a `&str`
//! (design/01 §6, §7.12). Empty pieces, an empty needle, and a `collect`
//! that stops at the first `Err` before a later panic.

use crate::support;

purecrate_canon::fixture!(mod lists = "fixtures/collect.rs");

const TEXTS: [&str; 15] =
    ["", "a", "a.b", "a..b", ".a.", "a.b.c", "é", "é.日", "a😀b", "😀", "ab", "bad", "boom", "a,boom", "a,bad,boom"];

#[test]
fn collected_lists_match_rust() {
    let cases = support::cases(|cases| {
        for s in TEXTS {
            cases.push(case!(lists::words(s)));
            cases.push(case!(lists::dotted(s)));
            cases.push(case!(lists::commas(String::from(s))));
            cases.push(case!(lists::tokens(s)));
            cases.push(case!(lists::tokens_inferred(s)));
            cases.push(case!(lists::each_token(s)));
            cases.push(case!(lists::pieces(s)));
        }
        grid!(cases, lists::tokens; s in ["boom", "bad,boom", "ok,boom", "boom,bad"]);
        for s in TEXTS {
            for c in ['.', ',', ' ', 'é', '😀'] {
                cases.push(case!(lists::lengths(s, c)));
                cases.push(case!(lists::once_char(s, c)));
            }
            cases.push(case!(lists::once_head(s)));
            for p in ["", ".", "..", "a", "é", "😀", "ab", "b.c"] {
                cases.push(case!(lists::once_str(s, p)));
                cases.push(case!(lists::once_owned(String::from(s), String::from(p))));
            }
        }
    });
    assert!(cases.iter().any(|c| c.rust.starts_with("panic(")), "a later piece panics");
    assert!(cases.iter().any(|c| c.rust.starts_with("Err(")), "an earlier piece is an Err");
    support::assert_equivalent("collect", lists::SOURCE, &cases);
}

/// `collect` from a split prints as the array or `Iter.tryCollect`, and
/// `split_once` as `Str.splitOnce`, with no turbofish binding left over.
#[test]
fn collect_and_split_once_are_runtime_calls() {
    let source = "pub fn pieces(s: &str) -> Vec<String> { s.split('.').map(|p| String::from(p)).collect() }\n\
                  pub fn raw(s: &str) -> usize { s.split('.').collect::<Vec<&str>>().len() }\n\
                  pub fn nums(s: &str) -> Result<Vec<u8>, bool> { s.split(',').map(|p| Ok(1u8)).collect() }\n\
                  pub fn cut(s: &str) -> Option<(String, String)> {\n\
                      match s.split_once('+') {\n\
                          Some((a, b)) => Some((String::from(a), String::from(b))),\n\
                          None => None,\n\
                      }\n\
                  }\n";
    let krate = purecrate_syntax::parse_source("lists", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| {
        let src = pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
        src.lines().map(str::trim).collect::<Vec<_>>().join(" ")
    };
    let pieces = file("pieces");
    assert!(pieces.contains(".split(") && pieces.contains(".map("), "{pieces}");
    assert!(!pieces.contains("$collect") && !pieces.contains("Iter."), "{pieces}");
    let raw = file("raw");
    assert!(raw.contains(".split(") && raw.contains(".length"), "{raw}");
    assert!(!raw.contains("$collect"), "{raw}");
    let nums = file("nums");
    assert!(nums.contains("Iter.tryCollect("), "{nums}");
    assert!(!nums.contains("$collect"), "{nums}");
    let cut = file("cut");
    assert!(cut.contains("Str.splitOnce(") && cut.contains("[0]") && cut.contains("[1]"), "{cut}");
}

#[test]
fn collect_off_a_split_is_refused() {
    let refuse = |source: &str, needle: &str| {
        let krate = purecrate_syntax::parse_source("no", source).expect("parse");
        let err = purecrate_check::accept(&krate).expect_err("refused");
        assert!(err.iter().any(|d| d.message.contains(needle)), "{needle} not in {err:#?}");
    };
    refuse("pub fn f(xs: Vec<u8>) -> Vec<u8> { xs.iter().rev().collect() }\n", "`collect` builds a `Vec` from");
    refuse("pub fn f(s: &str) -> usize { let parts = s.split('.').collect(); parts.len() }\n", "needs its target");
    refuse("pub fn f(s: &str) -> Vec<u8> { s.split('.').collect() }\n", "expected `Vec<u8>`");
    refuse("pub fn f(s: &str) -> u32 { s.split('.').map(|p| p.len()).collect() }\n", "not `u32`");
    refuse("pub fn f(s: &str) -> Vec<u8> { s.split(\".\").map(|p| 1u8).collect() }\n", "`char` separator");
}
