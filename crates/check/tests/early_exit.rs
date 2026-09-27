mod common;

use common::{assert_clean, assert_rejects};

const HALF: &str = "pub fn half(n: i32) -> Result<i32, i32> { if n % 2 == 0 { Ok(n / 2) } else { Err(n) } }\n";

fn with_half(f: &str) -> String {
    format!("{HALF}{f}")
}

#[test]
fn try_and_return_in_statement_positions_are_accepted() {
    assert_clean(&with_half("pub fn f(n: i32) -> Result<i32, i32> { let h = half(n)?; Ok(h) }"));
    assert_clean(&with_half("pub fn f(n: i32) -> Result<i32, i32> { Ok(half(n)? + half(n)?) }"));
    assert_clean("pub fn f(a: Option<i32>) -> Option<i32> { Some(a? + 1) }");
    assert_clean(
        "pub fn f(x: Option<i32>) -> i32 { let v = match x { Some(v) => v, None => return 0 }; v }",
    );
}

#[test]
fn return_inside_a_larger_expression_is_rejected() {
    assert_rejects(
        "pub fn f(x: Option<i32>) -> i32 { 1 + match x { Some(v) => v, None => return 0 } }",
        "`return` inside a larger expression is not in v0",
    );
}

#[test]
fn try_that_may_not_run_is_rejected() {
    assert_rejects(
        "pub fn f(b: bool, x: Option<bool>) -> Option<bool> { Some(b && x?) }",
        "`?` inside `&&`, `||`",
    );
}

#[test]
fn try_needs_a_matching_return_type() {
    assert_rejects(
        &with_half("pub fn f(n: i32) -> Result<i32, bool> { Ok(half(n)?) }"),
        "v0 has no `From` conversion",
    );
    assert_rejects(
        "pub fn f(a: Option<i32>) -> i32 { a? }",
        "`?` on `Option<i32>` in a function returning `i32`",
    );
    assert_rejects("pub fn f(a: i32) -> Option<i32> { Some(a?) }", "`?` needs a `Result` or `Option`");
}
