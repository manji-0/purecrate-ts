mod common;

use common::{assert_clean, assert_rejects};

#[test]
fn string_from_builds_a_string_from_a_literal() {
    assert_clean("pub struct Sku(String); pub fn f() -> Sku { Sku(String::from(\"a\")) }");
    assert_clean("pub fn f(s: String) -> String { String::from(s) }");
    assert_clean("pub fn f(s: &str) -> String { String::from(s) }");
    assert_clean("pub fn f() -> Option<String> { Some(String::from(\"a\")) }");
}

#[test]
fn a_literal_is_not_a_string() {
    let hint = "a string literal is `&str`, write `String::from(\"..\")`";
    assert_rejects("pub struct Sku(String); pub fn f() -> Sku { Sku(\"a\") }", hint);
    assert_rejects("pub fn f() -> String { let s: String = \"a\"; s }", hint);
    assert_rejects("pub fn g(s: String) -> String { s } pub fn f() -> String { g(\"a\") }", hint);
    assert_rejects("pub fn f() -> String { \"a\" }", hint);
    assert_rejects("pub fn f(s: &str) -> String { s }", "expected `String`, found `&str`");
}

#[test]
fn a_string_derefs_where_str_is_wanted() {
    assert_clean("pub fn g(s: &str) -> bool { s == \"\" } pub fn f(s: String) -> bool { g(&s) }");
    assert_clean("pub fn f() -> bool { let s = \"a\"; s == \"a\" }");
}

#[test]
fn string_and_str_compare_in_either_order() {
    assert_clean("pub fn f(s: String) -> bool { s == \"\" }");
    assert_clean("pub fn f(s: String) -> bool { \"\" != s }");
    assert_clean("pub fn f(a: &str, b: String) -> bool { a == b }");
    assert_clean("pub fn f(a: String, b: String) -> bool { a == b }");
    assert_rejects("pub fn f(s: String) -> bool { s < \"b\" }", "ordering on `String`");
}

#[test]
fn string_from_takes_one_argument() {
    assert_rejects("pub fn f() -> String { String::from(\"a\", \"b\") }", "`String::from` takes 1 argument");
}
