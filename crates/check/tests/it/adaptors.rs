//! Iterator adaptors and ranges (design/01 §7.22): what each takes, and
//! what stays refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects};

#[test]
fn stages_take_an_option_or_a_vec() {
    assert_clean("pub fn f(xs: &[i32]) -> Vec<i32> { xs.iter().filter_map(|x| x.checked_mul(2)).collect() }");
    assert_clean("pub fn f(xs: Vec<Vec<u8>>) -> usize { xs.iter().flatten().count() }");
    assert_clean("pub fn f(xs: Vec<Option<u8>>) -> Vec<u8> { xs.into_iter().flatten().collect() }");
    assert_clean("pub fn f(xs: &[u8]) -> Vec<u8> { xs.iter().flat_map(|x| vec![*x, *x]).collect() }");
    assert_clean(
        "pub fn f(xs: &[u8]) -> u32 { let mut n = 0u32; for x in xs.iter().flat_map(|x| x.checked_add(1)) { n += u32::from(x); } n }",
    );
    // Rust takes any `IntoIterator`; v0 a `Vec` or an `Option`.
    assert_rejects(
        "pub fn f(xs: Vec<String>) -> usize { xs.iter().flat_map(|s| s.chars()).count() }",
        "`flat_map`'s function returns a `Vec` or an `Option` in v0",
    );
    assert_rejects(
        "pub fn f(xs: Vec<Result<u8, String>>) -> usize { xs.into_iter().flatten().count() }",
        "`flatten` takes items that are `Vec`s or `Option`s in v0, found `Result<u8, String>`",
    );
}

#[test]
fn windows_and_chunks_are_sources() {
    assert_clean("pub fn f(xs: &[u32]) -> bool { xs.windows(2).all(|w| w[0] <= w[1]) }");
    assert_clean("pub fn f(xs: Vec<u8>) -> usize { let mut n = 0usize; for c in xs.chunks(3) { n += c.len(); } n }");
    // Only as a sequence: a value of it would be an array where Rust's is
    // an iterator.
    assert_rejects(
        "pub fn f(xs: &[u32]) -> usize { let w = xs.windows(2); w.len() }",
        "`.windows()` on `Vec<u32>` is not on the std allow-list",
    );
    assert_rejects(
        "pub fn f(xs: Vec<u32>) -> usize { xs.iter().map(|x| x + 1).rev().count() }",
        "`rev` comes right after the source in v0",
    );
}

#[test]
fn dedup_writes_a_local_and_compares_with_eq() {
    assert_clean("pub fn f(xs: Vec<String>) -> Vec<String> { let mut v = xs; v.dedup(); v }");
    assert_clean("pub fn f(xs: Vec<String>) -> Vec<String> { let mut v = xs; v.dedup_by_key(|s| s.len()); v }");
    assert_rejects(
        "pub struct S { pub xs: Vec<u8> }\npub fn f(s: S) -> S { let mut s = s; s.xs.dedup(); s }",
        "`dedup` dedups a local `let mut v: Vec<T>` in v0",
    );
    assert_rejects(
        "pub fn f(xs: Vec<&str>) -> usize { let mut v: Vec<Result<u8, std::num::ParseIntError>> = xs.iter().map(|s| s.parse::<u8>()).collect(); v.dedup(); v.len() }",
        "`dedup` compares with `==`",
    );
}

#[test]
fn step_by_steps_a_range() {
    assert_clean("pub fn f(n: u32) -> u32 { let mut s = 0u32; for i in (0..n).step_by(2) { s += i; } s }");
    assert_clean("pub fn f(n: u8) -> Vec<u8> { (0..=n).rev().step_by(3).collect() }");
    assert_clean("pub fn f(n: u8) -> Vec<u8> { (0..=n).step_by(3).rev().collect() }");
    assert_clean("pub fn f(n: u64) -> u64 { (1..=n).map(|x| x * 2).sum() }");
    assert_rejects("pub fn f(xs: Vec<u8>) -> usize { xs.iter().step_by(2).count() }", "`step_by` steps a range in v0");
    assert_rejects(
        "pub fn f(n: u32) -> usize { (0..n).map(|x| x + 1).step_by(2).count() }",
        "`step_by` comes right after the range in v0",
    );
    assert_rejects(
        "pub fn f(xs: Vec<u8>) -> usize { (0..10u8).zip(xs.iter()).count() }",
        "`zip` pairs a `Vec`'s, a slice's, or a string's items in v0, not a range's",
    );
    assert_rejects(
        "pub fn f() -> u32 { let mut s = 0u32; for c in ('a'..='e').step_by(2) { s += u32::from(c); } s }",
        "a range `a..b` is of one integer type, found `char`",
    );
    // A range is a sequence, not a value.
    assert_rejects("pub fn f(n: u32) -> bool { (0..n).contains(&3) }", "a range `a..b` is a sequence in v0");
    assert_rejects(
        "pub fn f(n: u32) -> u32 { let r = (0..n).rev(); r.sum() }",
        "a range's `rev()` is a sequence in v0",
    );
    assert_parse_rejects("pub fn f() -> usize { (0u8..).step_by(2).count() }", "ranges are not in v0");
    assert_parse_rejects(
        "pub fn f(xs: Vec<u8>) -> u32 { let mut n = 0u32; for x in xs.iter().peekable() { n += u32::from(*x); } n }",
        "`for` over `.peekable()` is not in v0",
    );
}

/// `take`, `skip`, `take_while`, `skip_while`, and `partition` (design/01
/// §7.28): stages and a consumer, read where they are written.
#[test]
fn cuts_are_stages_and_partition_a_consumer() {
    assert_clean("pub fn f(xs: &[i32]) -> i32 { xs.iter().map(|x| 6 / x).take(2).sum() }");
    assert_clean("pub fn f(xs: &[u8], n: usize) -> Vec<u8> { xs.iter().skip(n).copied().collect() }");
    assert_clean(
        "pub fn f(s: &str) -> String { s.chars().skip_while(|c| *c == ' ').take_while(|c| *c != ';').collect() }",
    );
    assert_clean(
        "pub fn f(xs: &[u8], n: usize) -> u32 { let mut s = 0u32; for x in xs.iter().skip(1).take(n) { s += u32::from(*x); } s }",
    );
    assert_clean("pub fn f(xs: Vec<i32>) -> (Vec<i32>, Vec<i32>) { xs.into_iter().partition(|x| *x > 0) }");
    assert_clean(
        "pub fn f(xs: &[u32]) -> usize { let (a, b): (Vec<u32>, Vec<u32>) = xs.iter().map(|x| x * 2).partition(|x| *x > 3); a.len() + b.len() }",
    );
    // On a range: still a range, so `rev` and `step_by` may follow.
    assert_clean("pub fn f(n: u64, k: usize) -> u64 { (3..n).skip(k).take(4).sum() }");
    assert_clean("pub fn f(n: i32, k: usize) -> Vec<i32> { (0..n).take(k).rev().step_by(2).collect() }");
    // The count is a `usize`; the predicate takes the items.
    assert_rejects("pub fn f(xs: Vec<u8>, n: u8) -> u8 { xs.iter().take(n).sum() }", "expected `usize`, found `u8`");
    assert_rejects(
        "pub fn f(xs: Vec<u8>) -> u8 { xs.iter().take_while(|x: &u16| *x > 2).sum() }",
        "`take_while`'s closure takes `u16`, the items are `u8`",
    );
    // Not values.
    for (src, stage) in [
        ("pub fn f(xs: Vec<u8>) -> u32 { let it = xs.iter().take(2); it.map(|x| u32::from(*x)).sum() }", "take"),
        ("pub fn f() -> usize { let it = (0..5u8).skip(2); it.count() }", "skip"),
        ("pub fn f(xs: Vec<u8>) -> usize { let it = xs.iter().take_while(|x| **x > 1); it.count() }", "take_while"),
    ] {
        assert_rejects(src, &format!("`{stage}` is a stage of a sequence in v0"));
    }
    // A stage before `rev` on a list's items runs out of Rust's order.
    assert_rejects(
        "pub fn f(xs: Vec<u8>) -> u8 { xs.iter().take(2).rev().copied().sum() }",
        "`rev` comes right after the source in v0",
    );
    // `partition` names its result's type, two `Vec`s, written out.
    assert_rejects(
        "pub fn f(xs: Vec<i32>) -> usize { let (a, b) = xs.into_iter().partition(|x| *x > 0); a.len() + b.len() }",
        "`partition` needs its result's type",
    );
    assert_rejects(
        "use std::collections::BTreeSet;\npub fn f(xs: Vec<String>) -> (BTreeSet<String>, BTreeSet<String>) { xs.into_iter().partition(|x| x.len() > 2) }",
        "`partition` builds two `Vec`s in v0, `(Vec<T>, Vec<T>)`, not `(BTreeSet<String>, BTreeSet<String>)`",
    );
    assert_rejects(
        "pub fn f(xs: Vec<u8>) -> (Vec<u16>, Vec<u16>) { xs.into_iter().partition(|x| *x > 2) }",
        "expected `(Vec<u16>, Vec<u16>)`, found `(Vec<u8>, Vec<u8>)`",
    );
    assert_parse_rejects(
        "pub fn f(xs: Vec<u8>) -> usize { let (a, b): (Vec<_>, Vec<_>) = xs.into_iter().partition(|x| *x > 2); a.len() + b.len() }",
        "unsupported type `_`",
    );
    // The adaptors §7.28 leaves out.
    for (src, m) in [
        ("pub fn f(xs: Vec<u8>) -> usize { xs.iter().peekable().count() }", "peekable"),
        ("pub fn f(xs: Vec<u8>) -> u32 { xs.iter().scan(0u32, |_s, x| Some(u32::from(*x))).sum() }", "scan"),
        ("pub fn f(xs: Vec<u8>) -> usize { xs.iter().map_while(|x| x.checked_add(1)).count() }", "map_while"),
    ] {
        assert_rejects(src, &format!("`.{m}()` on an iterator is not on the std allow-list"));
    }
    assert_rejects(
        "pub fn f(xs: Vec<u8>) -> usize { let mut it = xs.iter(); it.by_ref().take(2).count() }",
        "the receiver type of `.by_ref()` is not known here",
    );
}

#[test]
fn an_unsuffixed_start_takes_its_first_use_type() {
    assert_clean("pub fn f(n: u32) -> u32 { let mut s = 0; for i in 0..=n { s += i; } s }");
    assert_clean("pub fn f(xs: &[i16]) -> i16 { let mut lo = 0; for x in xs { if *x < lo { lo = *x; } } lo }");
    assert_clean("fn g(x: u64) -> u64 { x }\npub fn f() -> u64 { let n = 7; g(n) }");
    // No use names a type: rustc's `i32` default is not taken.
    assert_rejects("pub fn f() -> i64 { let x = 1; 2 }", "cannot tell the integer type of `1`");
    assert_rejects("pub fn f() -> u8 { let x = 1; let _y = x; 2 }", "cannot tell the integer type of `1`");
    // Every use is checked against the type found.
    assert_rejects("pub fn f(a: u8, b: u16) -> u16 { let mut s = 0; s += a; s += b; 0 }", "expected `u8`, found `u16`");
}
