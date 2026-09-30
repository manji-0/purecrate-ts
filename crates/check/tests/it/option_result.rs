
use crate::common::{assert_clean, assert_rejects};

#[test]
fn option_and_result_matches_are_accepted() {
    assert_clean("pub fn f(x: Option<i32>) -> i32 { match x { Some(v) => v, None => 0 } }");
    assert_clean("pub fn f(x: Result<i32, bool>) -> i32 { match x { Err(_) => 0, Ok(v) => v } }");
    assert_clean("pub fn f(x: Option<i32>) -> i32 { if let Some(v) = x { v } else { 0 } }");
}

#[test]
fn missing_and_repeated_cases_are_rejected() {
    assert_rejects(
        "pub fn f(x: Option<i32>) -> i32 { match x { Some(v) => v, Some(_) => 0 } }",
        "`Some` is matched more than once",
    );
    assert_rejects(
        "pub fn f(x: Result<i32, i32>) -> i32 { match x { Ok(v) => v, Ok(w) => w } }",
        "match on `Result` is missing `Err`",
    );
}

#[test]
fn mixed_families_are_rejected() {
    assert_rejects(
        "pub fn f(x: Option<i32>) -> i32 { match x { Some(v) => v, Err(_) => 0 } }",
        "match mixes cases of `Option` and `Result`",
    );
}

#[test]
fn pattern_must_fit_the_scrutinee() {
    assert_rejects(
        "pub fn f(x: Result<i32, i32>) -> i32 { match x { Some(v) => v, None => 0 } }",
        "pattern `Some(..)` does not match a value of type `Result<i32, i32>`",
    );
}

#[test]
fn nested_option_is_rejected() {
    assert_rejects("pub fn f(x: Option<Option<i32>>) -> i32 { 0 }", "`Option<Option<_>>` is not in v0");
    assert_rejects(
        "pub type Maybe = Option<i32>;\npub struct S { pub m: Option<Maybe> }",
        "`Option<Option<_>>` is not in v0",
    );
}
