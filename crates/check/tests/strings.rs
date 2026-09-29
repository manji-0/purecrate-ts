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

#[test]
fn string_patterns_match_a_str() {
    assert_clean("pub fn f(s: &str) -> i32 { match s { \"a\" => 1, \"b\" | \"c\" => 2, _ => 3 } }");
    assert_clean("pub fn f(s: String) -> i32 { match s.as_str() { \"a\" => 1, _ => 2 } }");
    assert_clean("pub fn f(s: &str) -> bool { matches!(s, \"a\" | \"b\") }");
    assert_rejects(
        "pub fn f(s: String) -> i32 { match s { \"a\" => 1, _ => 2 } }",
        "string patterns match a `&str`, found `String`; match on `s.as_str()`",
    );
    assert_rejects("pub fn f(x: i32) -> i32 { match x { \"a\" => 1, _ => 2 } }", "string patterns do not match a value of type `i32`");
}

#[test]
fn a_match_on_strings_ends_in_a_wildcard() {
    assert_rejects("pub fn f(s: &str) -> i32 { match s { \"a\" => 1 } }", "a match on strings must end in a `_` arm");
    assert_rejects("pub fn f(s: &str) -> i32 { match s { _ => 0, \"a\" => 1 } }", "`_` must be the last arm");
    assert_rejects("pub fn f(s: &str) -> i32 { match s { \"a\" => 1, 2 => 2, _ => 3 } }", "match mixes integer arms with other arms");
}

#[test]
fn as_str_is_only_on_a_string() {
    assert_clean("pub fn f(s: String) -> bool { s.as_str() == \"a\" }");
    assert_rejects("pub fn f(s: &str) -> bool { s.as_str() == \"a\" }", "as_str");
}
