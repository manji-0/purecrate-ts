//! Lower a single Rust source file into a flattened PureCrate IR.
//! The caller supplies the crate name and the source text.

mod expr;
mod item;
mod ty;

use purecrate_ir::{Crate, Item};
use syn::parse_file;

pub use item::ParseError;

pub fn parse_source(crate_name: &str, source: &str) -> Result<Crate, ParseError> {
    let file = parse_file(source).map_err(|e| ParseError::new(e.to_string()))?;
    let mut cx = item::Cx::scan(&file);
    let mut items: Vec<Item> = Vec::new();
    for syn_item in file.items {
        items.extend(item::lower_item(&mut cx, syn_item)?);
    }
    Ok(Crate::new(crate_name, items))
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
