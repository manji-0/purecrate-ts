//! Lower a single Rust source file into a flattened PureCrate IR.
//! The caller supplies the crate name and the source text.

mod expr;
mod item;
mod ty;

use purecrate_ir::{Crate, Item};
use syn::parse_file;

pub use item::{LineCol, ParseError};

pub fn parse_source(crate_name: &str, source: &str) -> Result<Crate, ParseError> {
    parse_source_spanned(crate_name, source).map(|(krate, _)| krate)
}

/// Also returns where each item's name is, parallel to `Crate::items`.
pub fn parse_source_spanned(
    crate_name: &str,
    source: &str,
) -> Result<(Crate, Vec<LineCol>), ParseError> {
    let file = parse_file(source).map_err(|e| ParseError::new(e.to_string()).or_at(e.span()))?;
    let mut cx = item::Cx::scan(&file);
    let mut items: Vec<Item> = Vec::new();
    let mut spans: Vec<LineCol> = Vec::new();
    for syn_item in file.items {
        for (item, at) in item::lower_item(&mut cx, syn_item)? {
            items.push(item);
            spans.push(at);
        }
    }
    Ok((Crate::new(crate_name, items), spans))
}

#[cfg(test)]
mod tests {
    use super::*;
    use purecrate_ir::counter_example;

    const COUNTER: &str = include_str!("../../../examples/counter/src/lib.rs");

    #[test]
    fn counter_matches_handwritten_ir() {
        let parsed = parse_source("counter", COUNTER).expect("parse");
        assert_eq!(parsed, counter_example());
    }

    fn with_arms(arms: &str) -> String {
        format!(
            "pub enum Cmd {{ Move(i32, i32), Stop }}
             pub enum Dir {{ Up }}
             pub fn run(cmd: Cmd) -> i32 {{ match cmd {{ {arms} }} }}"
        )
    }

    fn rejects(arms: &str, needle: &str) {
        let err = parse_source("c", &with_arms(arms)).expect_err(arms);
        assert!(err.message.contains(needle), "{arms}: {}", err.message);
    }

    #[test]
    fn variant_arms_with_name_bindings_are_accepted() {
        parse_source("c", &with_arms("Cmd::Move(a, _) => a, Cmd::Stop => 0")).expect("parse");
    }

    fn error_at(source: &str) -> (usize, usize, String) {
        let err = parse_source("c", source).expect_err(source);
        let at = err.at.unwrap_or_else(|| panic!("no location: {}", err.message));
        (at.line, at.col, err.message)
    }

    #[test]
    fn errors_point_at_the_offending_node() {
        let src = "pub struct S { pub n: i32 }\n\
                   pub fn f(s: S) -> i32 {\n    let r = &s;\n    0\n}\n";
        let (line, col, msg) = error_at(src);
        assert_eq!((line, col), (3, 13), "{msg}");

        assert_eq!(error_at(src).2, "unsupported expression `&s`");

        let (line, col, msg) = error_at("pub fn f(x: Box<i32>) -> i32 { 0 }");
        assert_eq!((line, col), (1, 13), "{msg}");
        assert_eq!(msg, "`Box` is not allowed in v0");

        let (_, _, msg) = error_at("pub fn f(x: std::fs::File) -> i32 { 0 }");
        assert!(msg.contains("qualified type path `std::fs::File`"), "{msg}");

        let (line, col, msg) = error_at(&with_arms("Cmd::Stop => 0,\n _ => 1"));
        assert_eq!(line, 4, "{msg}");
        assert_eq!(col, 2, "{msg}");

        let (line, _, msg) = error_at("pub fn f( -> i32 { 0 }");
        assert_eq!(line, 1, "{msg}");
    }

    #[test]
    fn methods_get_the_same_signature_checks_as_free_fns() {
        let src = "pub struct S { pub n: i32 }\n\
                   impl S {\n    pub async fn f(self) -> S { self }\n}\n";
        let (line, col, msg) = error_at(src);
        assert!(msg.contains("async"), "{msg}");
        assert_eq!((line, col), (3, 9));

        let (_, _, msg) = error_at("pub fn f(self) -> i32 { 0 }");
        assert!(msg.contains("outside an impl"), "{msg}");
    }

    #[test]
    fn unsupported_arm_patterns_are_rejected() {
        rejects("Cmd::Stop => 0, _ => 1", "found `_`");
        rejects("Cmd::Stop => 0, other => 1", "found binding `other`");
        rejects("Cmd::Move(1, b) => b, Cmd::Stop => 0", "found a literal");
        rejects(
            "Cmd::Move(a, Dir::Up) => a, Cmd::Stop => 0",
            "found nested variant `Dir::Up`",
        );
    }
}
