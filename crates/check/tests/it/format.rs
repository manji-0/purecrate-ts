//! `format!` (design/01 §6.8): what it prints the same in Rust and TS is
//! accepted; every spec, argument type, and argument order that would not
//! is refused with what v0 takes.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects};

#[test]
fn format_on_what_prints_the_same_is_clean() {
    assert_clean(
        "pub fn f(n: u32, s: &str, t: String, c: char, b: bool, w: i128) -> String {\n\
             let a = format!(\"{n:05}|{s:>4}|{t:*^5}|{c}|{b}|{w}\");\n\
             let b2 = format!(\"{}-{}\", n.checked_add(1).unwrap_or(0), s);\n\
             let p = format!(\"{0}{0}{q}\", n, q = c);\n\
             format!(\"{a}{b2}{p}{{}}\")\n\
         }",
    );
}

#[test]
fn specs_outside_v0_are_refused() {
    for spec in ["{:?}", "{:x}", "{:#?}", "{:+}", "{:.2}", "{:1$}", "{:e}"] {
        let src = format!("pub fn f(n: u32) -> String {{ format!(\"{spec}\", n) }}");
        assert_parse_rejects(&src, "is not in v0");
    }
    assert_parse_rejects("pub fn f(w: usize, n: u32) -> String { format!(\"{:w$}\", n) }", "is not in v0");
}

#[test]
fn arguments_that_print_otherwise_are_refused() {
    assert_rejects(
        "pub fn f(x: f64) -> String { format!(\"{}\", x) }",
        "`format!` prints an integer, `bool`, `char`, `&str`, `String`, or `Uuid` in v0, not `f64`",
    );
    assert_rejects("pub fn f(x: Option<u32>) -> String { format!(\"{}\", x) }", "not `Option<u32>`");
    assert_rejects("pub fn f(s: &str) -> String { format!(\"{:05}\", s) }", "zero padding is for integers in v0");
    assert_rejects(
        "use uuid::Uuid;\npub fn f(u: Uuid) -> String { format!(\"{:40}\", u) }",
        "`Uuid` prints without padding in Rust",
    );
}

#[test]
fn an_untyped_literal_needs_its_type() {
    assert_rejects("pub fn f() -> String { format!(\"{}\", 5) }", "cannot tell the integer type of `5`");
}

#[test]
fn arguments_out_of_order_are_refused() {
    let needle = "uses its arguments out of order or more than once";
    assert_parse_rejects("pub fn f(a: u8, b: u8) -> String { format!(\"{1}{0}\", a + 1, b + 1) }", needle);
    assert_parse_rejects("pub fn f(a: u8) -> String { format!(\"{0}{0}\", a + 1) }", needle);
    let needle = "with a statement or an assignment";
    assert_parse_rejects("pub fn f(a: u8) -> String { let mut n = a; format!(\"{n}{}\", { n += 1; n }) }", needle);
    assert_parse_rejects(
        "pub fn f(s: String) -> String { let mut t = s; format!(\"{t}{}\", { t.push('x'); 1u8 }) }",
        needle,
    );
}
