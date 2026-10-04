use crate::common::{assert_clean, assert_rejects};

#[test]
fn chars_compare_and_match() {
    assert_clean("pub fn f(a: char, b: char) -> bool { a < b || a == 'x' }");
    assert_clean("pub fn f(c: char) -> i32 { match c { 'a'..='z' | '_' => 1, '0'..'9' => 2, _ => 3 } }");
    assert_clean("pub fn f(c: char) -> bool { matches!(c, 'a'..='f' | 'A'..='F') }");
    assert_clean("pub fn f(c: char) -> Option<char> { if c == '\\'' { Some('\"') } else { None } }");
}

#[test]
fn char_arms_end_in_a_wildcard() {
    assert_rejects("pub fn f(c: char) -> i32 { match c { 'a' => 1 } }", "a match on chars must end in a `_` arm");
    assert_rejects("pub fn f(c: char) -> i32 { match c { 'a' => 1, 1 => 2, _ => 3 } }", "match mixes");
    assert_rejects(
        "pub fn f(x: u8) -> i32 { match x { 'a' => 1, _ => 2 } }",
        "`char` patterns do not match a value of type `u8`",
    );
    assert_rejects(
        "pub fn f(s: &str) -> i32 { match s { 'a' => 1, _ => 2 } }",
        "`char` patterns do not match a value of type `&str`",
    );
}

#[test]
fn conversions_follow_std_from() {
    assert_clean("pub fn f(c: char) -> u32 { u32::from(c) }");
    assert_clean("pub fn f(c: char) -> u64 { u64::from(c) }");
    assert_clean("pub fn f(b: u8) -> char { char::from(b) }");
    assert_clean("pub fn f(n: u32) -> Option<char> { char::from_u32(n) }");
    assert_rejects("pub fn f(c: char) -> u8 { u8::from(c) }", "`u8::from` does not take `char`");
}

#[test]
fn ascii_methods_are_allowed_and_unicode_ones_are_not() {
    assert_clean("pub fn f(c: char) -> bool { c.is_ascii_digit() && c.eq_ignore_ascii_case(&'a') }");
    assert_clean("pub fn f(c: char) -> Option<u32> { c.to_digit(16u32) }");
    assert_clean("pub fn f(c: char) -> Option<u32> { c.to_digit(10) }");
    assert_clean("pub fn f(c: char) -> usize { c.len_utf8() }");
    assert_rejects("pub fn f(c: char) -> bool { c.is_alphabetic() }", "is_alphabetic");
    assert_rejects(
        "pub fn f(c: char) -> bool { c.is_ascii_digit(1u32) }",
        "`char::is_ascii_digit` takes 0 argument(s)",
    );
}
