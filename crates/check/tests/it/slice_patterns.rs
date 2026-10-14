//! Slice patterns (design/01 §7.24): the lengths are checked as rustc
//! splits them, the elements by the decision tree; forms outside v0 are
//! refused by the parser.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects};

fn on(ty: &str, body: &str) -> String {
    format!("pub fn f(xs: &[{ty}]) -> u32 {{ {body} }}")
}

#[test]
fn slice_patterns_are_accepted() {
    assert_clean(&on("u32", "match xs { [] => 0, [x, ..] => *x }"));
    assert_clean(&on("u32", "match xs { [] => 0, [x] => *x, [x, .., y] => x + y }"));
    assert_clean(&on("u32", "match xs { [first, rest @ .., last] => first + last + rest.len() as u32, _ => 0 }"));
    assert_clean(&on("Option<u32>", "match xs { [Some(x), ..] => *x, [None, ..] => 1, [] => 2 }"));
    assert_clean(&on("u32", "if let [a, b] = xs { a + b } else { 0 }"));
    assert_clean(&on("u32", "let [a, ..] = xs else { return 0 }; *a"));
    assert_clean(&on("u32", "if matches!(xs, [1, ..] | [.., 1]) { 1 } else { 0 }"));
    assert_clean("pub fn f(xs: Vec<u32>) -> u32 { match xs.as_slice() { [x, ..] => *x, [] => 0 } }");
    assert_clean("pub fn f(xs: &[u32], n: u32) -> u32 { match (xs, n) { ([], 0) => 0, ([x, ..], _) => *x, _ => 1 } }");
    assert_clean("pub fn f(o: Option<Vec<u32>>) -> u32 { match o.as_deref() { Some([x, ..]) => *x, _ => 0 } }");
}

#[test]
fn every_length_must_be_covered() {
    assert_rejects(&on("u32", "match xs { [] => 0, [x] => *x }"), "match on a slice is missing `[_, _, ..]`");
    assert_rejects(&on("u32", "match xs { [x, ..] => *x, [.., y] => *y }"), "match on a slice is missing `[]`");
    assert_rejects(&on("u32", "match xs { [x, y] => x + y, [] => 0 }"), "is missing `[_]`, `[_, _, _, ..]`");
    // A guarded arm covers nothing.
    assert_rejects(
        &on("u32", "match xs { [] => 0, [x, ..] if *x > 1 => *x }"),
        "match on a slice is missing `[_, ..]`",
    );
}

#[test]
fn elements_left_out_are_refused() {
    // Each length is covered, but `[None]` is not.
    assert_rejects(
        &on("Option<u32>", "match xs { [] => 0, [Some(x)] => *x, [_, _, ..] => 1 }"),
        "this match on a slice may leave some value out; add a last `_` arm",
    );
    // Conservative: rustc accepts this one, as `0..=255` covers every `u8`;
    // the tree cannot tell, and asks for a `_`.
    assert_rejects(&on("u8", "match xs { [] => 0, [0..=255] => 1, [_, _, ..] => 2 }"), "may leave some value out");
}

#[test]
fn arms_follow_the_slice_rules() {
    assert_rejects(&on("u32", "match xs { [] => 0, _ => 1, [x, ..] => *x }"), "`_` must be the last arm");
    assert_rejects(
        "pub fn f(n: u32) -> u32 { match n { [] => 0, _ => 1 } }",
        "a slice pattern does not match a value of type `u32`",
    );
}

#[test]
fn forms_outside_v0_are_refused() {
    assert_parse_rejects(&on("u32", "match xs { [x, ..] | [.., x] => *x, [] => 0 }"), "`|` arms may not bind names");
    assert_parse_rejects(
        &on("u32", "match xs { [_, ref rest @ ..] => rest.len() as u32, [] => 0 }"),
        "`rest @ ..` binds a plain name",
    );
    assert_parse_rejects(&on("u32", "if let [..] = xs { 1 } else { 0 }"), "always matches");
    assert_parse_rejects(
        &on("u32", "if let [rest @ ..] = xs { rest.len() as u32 } else { 0 }"),
        "found binding `rest`",
    );
    assert_parse_rejects(&on("u32", "match xs { [.., x, ..] => *x, _ => 0 }"), "at most one `..`");
}
