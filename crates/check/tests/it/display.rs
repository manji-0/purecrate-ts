//! `impl Display` with arguments (design/01 §7.25): a crate type whose
//! `fmt` is made of writes prints through `format!` and `to_string`; a `fmt`
//! outside the subset is skipped, not refused, and printing its type is
//! refused with why; padding a crate type's value, and an argument that
//! computes after one, are refused.

use crate::common::{assert_clean, assert_rejects, messages};

const POINT: &str = "use std::fmt;\n\
     pub struct Point { pub x: i32, pub y: i32 }\n\
     impl fmt::Display for Point {\n\
         fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
             let y = self.y + 1;\n\
             write!(f, \"({}, \", self.x)?;\n\
             write!(f, \"{y})\")?;\n\
             Ok(())\n\
         }\n\
     }\n";

/// A float, which `format!` does not print, and a `fmt` with an `if`.
const SKIPPED: &str = "use std::fmt;\n\
     pub struct Rate { pub r: f64 }\n\
     impl fmt::Display for Rate {\n\
         fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, \"{}%\", self.r) }\n\
     }\n\
     pub struct Sign { pub n: i32 }\n\
     impl fmt::Display for Sign {\n\
         fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
             if self.n < 0 { f.write_str(\"-\") } else { f.write_str(\"+\") }\n\
         }\n\
     }\n";

fn with(prelude: &str, rest: &str) -> String {
    format!("{prelude}{rest}")
}

#[test]
fn display_with_arguments_prints_through_format_and_to_string() {
    assert_clean(&with(
        POINT,
        "pub enum Shape { Dot(Point), Line { from: Point, to: Point } }\n\
         impl fmt::Display for Shape {\n\
             fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {\n\
                 match self {\n\
                     Shape::Dot(p) => write!(f, \"dot {p}\"),\n\
                     Shape::Line { from, to } => { write!(f, \"{from}\")?; f.write_str(\" -> \")?; write!(f, \"{}\", to) }\n\
                 }\n\
             }\n\
         }\n\
         pub fn f(s: Shape, p: Point, n: u32) -> String {\n\
             let t = s.to_string();\n\
             format!(\"{q}|{t}|{}|{p}|{0}\", s, q = n + 1)\n\
         }",
    ));
}

#[test]
fn a_fmt_outside_the_subset_is_skipped() {
    assert_clean(&with(SKIPPED, "pub fn f(r: Rate, s: Sign) -> i32 { let _x = r.r; s.n }"));
}

#[test]
fn printing_a_type_whose_display_was_skipped_is_refused() {
    let needle = "has none translated";
    assert_rejects(&with(SKIPPED, "pub fn f(r: Rate) -> String { format!(\"{}\", r) }"), needle);
    assert_rejects(&with(SKIPPED, "pub fn f(s: Sign) -> String { s.to_string() }"), needle);
    // One that prints it is skipped in turn.
    assert_rejects(
        &with(
            SKIPPED,
            "pub struct Pair { pub r: Rate }\n\
             impl fmt::Display for Pair {\n\
                 fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, \"[{}]\", self.r) }\n\
             }\n\
             pub fn f(p: Pair) -> String { format!(\"{p}\") }",
        ),
        "`format!` prints `Pair` through its `impl Display`, and `Pair` has none translated",
    );
}

#[test]
fn padding_a_crate_type_is_refused() {
    for spec in ["{:>8}", "{:8}", "{:*^9}", "{:<3}"] {
        let src = with(POINT, &format!("pub fn f(p: Point) -> String {{ format!(\"{spec}\", p) }}"));
        assert_rejects(&src, "a width, alignment, or fill on `Point` is not in v0");
    }
    // Zero padding on a crate type is refused, as on any non-integer.
    let found = messages(&with(POINT, "pub fn f(p: Point) -> String { format!(\"{:08}\", p) }"));
    assert!(found.iter().any(|m| m.contains("on `Point` is not in v0")), "{found:#?}");
}

#[test]
fn an_argument_computed_after_a_crate_type_is_refused() {
    let needle = "runs a crate type's `Display` after evaluating every argument";
    assert_rejects(&with(POINT, "pub fn f(p: Point, n: u8) -> String { format!(\"{} {}\", p, n + 1) }"), needle);
    assert_rejects(&with(POINT, "pub fn f(p: Point, n: u8) -> String { format!(\"{p} {}\", n + 1) }"), needle);
    // Computed first, or a read after it, is as Rust runs it.
    assert_clean(&with(POINT, "pub fn f(p: Point, n: u8) -> String { format!(\"{} {} {n}\", n + 1, p) }"));
}

#[test]
fn std_error_stays_skipped() {
    assert_clean(&with(
        POINT,
        "#[derive(Debug)] pub enum E { Bad(i32) }\n\
         impl fmt::Display for E {\n\
             fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self { E::Bad(n) => write!(f, \"bad {n}\") } }\n\
         }\n\
         impl std::error::Error for E {}\n\
         pub fn f(e: E) -> String { e.to_string() }",
    ));
}
