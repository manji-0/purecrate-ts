//! `std::cmp::Ordering` against Rust: `cmp` on every integer type at its
//! edges, `char` across the surrogate gap, `bool`, `Uuid`, and strings, all
//! pairs of a set that crosses every UTF-8 width, shares prefixes, and puts
//! U+E000..=U+FFFF beside the supplementary planes (where JS's UTF-16 order
//! differs), with NFC beside NFD; `<`, `<=`, `>`, `>=` on the same pairs;
//! `Ordering`'s methods, `then` evaluating its argument and `then_with` its
//! closure only on `Equal`; `==`, `as`, `match` with guards, and `matches!`.
//! A crate with its own `Ordering`, and one that only calls `cmp`, too.

use crate::support;

use uuid::Uuid;

purecrate_canon::fixture!(mod ordering = "fixtures/ordering.rs");
purecrate_canon::fixture!(mod own_ordering = "fixtures/own_ordering.rs");
purecrate_canon::fixture!(mod cmp_only = "fixtures/cmp_only.rs");

use std::cmp::Ordering;

const STRINGS: &[&str] = &[
    "",
    "\0",
    " ",
    "a",
    "b",
    "A",
    "z",
    "aa",
    "ab",
    "abc",
    "a\0",
    "\u{7f}",
    "\u{80}",
    "é",
    "e\u{301}",
    "\u{7ff}",
    "\u{800}",
    "€",
    "\u{d7ff}",
    "\u{e000}",
    "\u{e000}a",
    "\u{fffd}",
    "\u{ffff}",
    "\u{10000}",
    "\u{10000}a",
    "😀",
    "\u{10ffff}",
    "a\u{e000}",
    "a\u{10000}",
    "a\u{ffff}b",
    "a\u{10000}b",
    "日本",
    "日本語",
];

const CHARS: &[char] =
    &['\0', 'A', 'a', '\u{7f}', 'é', '\u{d7ff}', '\u{e000}', '\u{fffd}', '\u{ffff}', '\u{10000}', '😀', '\u{10ffff}'];

const ORDERINGS: [Ordering; 3] = [Ordering::Less, Ordering::Equal, Ordering::Greater];

#[test]
fn generated_ordering_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in STRINGS {
            for b in STRINGS {
                cases.push(case!(ordering::cmp_str(*a, *b)));
                cases.push(case!(ordering::ops_str(*a, *b)));
            }
        }
        for (a, b) in [("", ""), ("\u{e000}", "\u{10000}"), ("\u{ffff}", "\u{10000}"), ("é", "e\u{301}"), ("ab", "a")] {
            cases.push(case!(ordering::cmp_string(a.to_string(), b.to_string())));
            // Each side as a list of its characters' strings, and of their code points.
            let list = |s: &str| s.chars().map(|c| c.to_string()).collect::<Vec<String>>();
            let codes = |s: &str| s.chars().map(|c| u64::from(u32::from(c))).collect::<Vec<u64>>();
            cases.push(case!(ordering::cmp_vec_string(list(a), list(b))));
            cases.push(case!(ordering::cmp_vec_u64(codes(a), codes(b))));
            cases.push(case!(ordering::ops_string(a.to_string(), b)));
        }
        let mut rng = support::Rng::new(0x0bde_c0de);
        for _ in 0..64 {
            let (a, b) = (rng.edgy(8, true) as i8, rng.edgy(8, true) as i8);
            cases.push(case!(ordering::cmp_i8(a, b)));
            let (a, b) = (rng.edgy(8, false) as u8, rng.edgy(8, false) as u8);
            cases.push(case!(ordering::cmp_u8(a, b)));
            let (a, b) = (rng.edgy(16, true) as i16, rng.edgy(16, true) as i16);
            cases.push(case!(ordering::cmp_i16(a, b)));
            let (a, b) = (rng.edgy(16, false) as u16, rng.edgy(16, false) as u16);
            cases.push(case!(ordering::cmp_u16(a, b)));
            let (a, b) = (rng.edgy(32, true) as i32, rng.edgy(32, true) as i32);
            cases.push(case!(ordering::cmp_i32(a, b)));
            let (a, b) = (rng.edgy(32, false) as u32, rng.edgy(32, false) as u32);
            cases.push(case!(ordering::cmp_u32(&a, &b)));
            let (a, b) = (rng.edgy(64, true) as i64, rng.edgy(64, true) as i64);
            cases.push(case!(ordering::cmp_i64(a, b)));
            let (a, b) = (rng.edgy(64, false) as u64, rng.edgy(64, false) as u64);
            cases.push(case!(ordering::cmp_u64(a, b)));
            // `usize` is checked to 2^53 − 1 in TS.
            let (a, b) = (rng.edgy(53, false) as usize, rng.edgy(53, false) as usize);
            cases.push(case!(ordering::cmp_usize(a, b)));
        }
        for (a, b) in [(i8::MIN, i8::MAX), (i8::MAX, i8::MIN), (-1, 0), (0, 0)] {
            cases.push(case!(ordering::cmp_i8(a, b)));
        }
        for (a, b) in [(i64::MIN, i64::MAX), (i64::MAX, i64::MAX - 1), (-1, 0), (i64::MIN, i64::MIN)] {
            cases.push(case!(ordering::cmp_i64(a, b)));
        }
        for (a, b) in [(u64::MAX, u64::MAX - 1), (0, u64::MAX), (1 << 53, (1 << 53) + 1)] {
            cases.push(case!(ordering::cmp_u64(a, b)));
        }
        for (a, b) in [(0usize, 9_007_199_254_740_991usize), (9_007_199_254_740_991, 9_007_199_254_740_990), (7, 7)] {
            cases.push(case!(ordering::cmp_usize(a, b)));
        }
        for a in CHARS {
            for b in CHARS {
                cases.push(case!(ordering::cmp_char(*a, *b)));
                cases.push(case!(ordering::not_after(*a, *b)));
            }
        }
        for a in [false, true] {
            for b in [false, true] {
                cases.push(case!(ordering::cmp_bool(a, b)));
            }
        }
        let ids = [
            Uuid::nil(),
            Uuid::max(),
            Uuid::from_u128(1),
            Uuid::from_u128(0x0000_0000_0000_0000_ffff_ffff_ffff_ffff),
            Uuid::from_u128(0xa000_0000_0000_0000_0000_0000_0000_0000),
            Uuid::from_u128(0x9fff_ffff_ffff_ffff_ffff_ffff_ffff_ffff),
        ];
        for a in ids {
            for b in ids {
                cases.push(case!(ordering::cmp_uuid(a, b)));
            }
        }
        for o in ORDERINGS {
            cases.push(case!(ordering::preds(o)));
            cases.push(case!(ordering::more_preds(o)));
            cases.push(case!(ordering::reversed(o)));
            cases.push(case!(ordering::as_int(o)));
            for n in [0u8, 4] {
                cases.push(case!(ordering::guarded(o, n)));
            }
            for p in ORDERINGS {
                cases.push(case!(ordering::same(o, p)));
            }
        }
        for (a, b) in [(0i32, 0i32), (1, 2), (2, 1), (i32::MAX, 0), (i32::MIN, 0)] {
            for x in [0i32, -5, i32::MAX] {
                cases.push(case!(ordering::then_eager(a, b, x)));
                cases.push(case!(ordering::then_lazy(a, b, x)));
            }
        }
        for (a, b) in [(1u8, 1u8), (1, 2), (2, 1)] {
            cases.push(case!(ordering::then_named(a, b)));
            cases.push(case!(ordering::orderings(u32::from(a), u32::from(b))));
        }
        let version = |major: u32, minor: u32, patch: u64, pre: &str| ordering::Version { major, minor, patch, pre: pre.to_string() };
        let versions = [
            (0, 0, 0, ""),
            (1, 0, 0, ""),
            (1, 0, 0, "alpha"),
            (1, 0, 0, "alpha.1"),
            (1, 0, 0, "beta"),
            (1, 0, 1, ""),
            (1, 1, 0, ""),
            (1, 1, 0, "\u{e000}"),
            (1, 1, 0, "\u{10000}"),
            (2, 0, 0, ""),
            (1, 0, u64::MAX, ""),
        ];
        for a in versions {
            for b in versions {
                cases.push(case!(ordering::compare(version(a.0, a.1, a.2, a.3), version(b.0, b.1, b.2, b.3))));
            }
        }
        for a in [-1i32, 0, 1] {
            for b in [-1i32, 0, 1] {
                for c in [-1i32, 0, 1] {
                    cases.push(case!(ordering::classify(a, b, c)));
                    cases.push(case!(ordering::same_cmp(i64::from(a), i64::from(b), i64::from(c))));
                }
            }
        }
        cases.push(case!(ordering::size_of(ordering::Size::Less(7))));
        cases.push(case!(ordering::size_of(ordering::Size::More)));
        cases
    });
    support::assert_equivalent("ordering", ordering::SOURCE, &cases);
}

#[test]
fn a_crates_own_ordering_stays_its_own() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for (a, b) in [(1, 2), (2, 2), (3, 2)] {
            cases.push(case!(own_ordering::order(a, b)));
            cases.push(case!(own_ordering::flip(own_ordering::order(a, b))));
        }
        cases
    });
    support::assert_equivalent("own_ordering", own_ordering::SOURCE, &cases);
}

#[test]
fn cmp_alone_gives_an_internal_ordering() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in STRINGS {
            for b in ["", "a", "\u{e000}", "\u{10000}"] {
                cases.push(case!(cmp_only::before(*a, b)));
            }
        }
        for (a, b, c) in [(1u64, 1u64, 1u64), (1, 1, 2), (u64::MAX, u64::MAX, u64::MAX), (0, u64::MAX, 0)] {
            cases.push(case!(cmp_only::settled(a, b, c)));
        }
        cases
    });
    support::assert_equivalent("cmp_only", cmp_only::SOURCE, &cases);
}

/// `cmp` and `then` print as calls to the runtime's `Ord`, nested in Rust's
/// order, not as `if` chains over `$lhs`/`$rhs` and a `match` per `then`.
#[test]
fn cmp_and_then_are_runtime_calls() {
    let source = "use std::cmp::Ordering;\n\
                  pub fn by(n: u8, m: u8, c: char, d: char) -> Ordering { n.cmp(&m).then(c.cmp(&d)) }\n";
    let krate = purecrate_syntax::parse_source("ord", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let by = &pkg.files.iter().find(|f| f.stem == "by").expect("by").source;
    assert!(by.contains("Ord.then(Ord.cmp(n, m), Ord.cmpStr(c, d))"), "{by}");
    assert!(!by.contains("$lhs") && !by.contains("switch"), "{by}");
}
