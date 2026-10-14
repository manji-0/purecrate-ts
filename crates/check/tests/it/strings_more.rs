//! `to_lowercase` / `to_uppercase`, `split` with a `&str`,
//! `split_whitespace`, `lines`, `join`, `repeat`, and `to_string`
//! (design/01 §7.21): where each stands, and what is refused.

use crate::common::{assert_clean, assert_rejects};

#[test]
fn text_methods_stand_where_split_does() {
    assert_clean("pub fn f(s: &str) -> (String, String) { (s.to_lowercase(), s.to_uppercase()) }");
    assert_clean("pub fn f(s: String) -> String { s.to_uppercase().to_lowercase() }");
    for source in ["s.split(p)", "s.split(\"ab\")", "s.split_whitespace()", "s.lines()"] {
        assert_clean(&format!(
            "pub fn f(s: &str, p: &str) -> (usize, bool, Vec<String>, Vec<usize>) {{\n\
                 let mut n: usize = 0;\n\
                 for t in {source} {{ n += t.len(); }}\n\
                 let all: Vec<&str> = {source}.collect();\n\
                 (n + all.len() + {source}.count(), {source}.any(|t| t.is_empty()), {source}.map(|t| String::from(t)).collect(), {source}.map(|t| t.len()).collect::<Vec<usize>>())\n\
             }}"
        ));
    }
    assert_clean("pub fn f(s: &str, p: String) -> usize { s.split(&p).count() }");
}

#[test]
fn an_empty_literal_separator_is_refused() {
    assert_rejects(
        "pub fn f(s: &str) -> usize { s.split(\"\").count() }",
        "`split(\"\")` is not in v0: Rust gives an empty piece at each end",
    );
    assert_rejects(
        "pub fn f(s: &str) -> u32 { let mut n = 0u32; for _t in s.split(\"\") { n += 1; } n }",
        "`split(\"\")` is not in v0",
    );
}

#[test]
fn join_takes_text_and_a_str() {
    assert_clean(
        "pub fn f(xs: Vec<String>, ys: &[&str], sep: String) -> (String, String) { (xs.join(\",\"), ys.join(&sep)) }",
    );
    assert_rejects(
        "pub fn f(xs: Vec<u32>) -> String { xs.join(\",\") }",
        "`join` on `Vec<u32>` is not in v0: it joins `String`s or `&str`s",
    );
    assert_rejects(
        "pub fn f(xs: Vec<String>) -> String { xs.join(',') }",
        "`join` takes a `&str` separator in v0, found `char`",
    );
}

#[test]
fn repeat_takes_a_usize() {
    assert_clean("pub fn f(s: &str, n: usize) -> String { s.repeat(n) }");
    assert_rejects("pub fn f(s: &str) -> String { s.repeat(2u32) }", "expected `usize`, found `u32`");
    assert_rejects("pub fn f(s: &str) -> String { s.repeat() }", "`str::repeat` takes 1 argument");
}

#[test]
fn to_string_is_what_format_prints() {
    assert_clean(
        "pub fn f(n: u8, m: i128, b: bool, c: char, s: &str, t: String) -> Vec<String> {\n\
             vec![n.to_string(), m.to_string(), b.to_string(), c.to_string(), s.to_string(), t.to_string()]\n\
         }",
    );
    assert_rejects(
        "pub fn f(x: f64) -> String { x.to_string() }",
        "`.to_string()` on `f64` is not in v0: `Display` of a float differs from JS",
    );
    assert_rejects("pub fn f(x: f32) -> String { x.to_string() }", "`.to_string()` on `f32` is not in v0");
}
