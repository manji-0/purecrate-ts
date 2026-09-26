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
}
