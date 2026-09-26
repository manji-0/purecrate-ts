//! Subset checks that must pass before anything is emitted.
//! Diagnostics point at items by index into `Crate::items`; the caller maps
//! indices to source locations.

mod names;
mod resolve;

use purecrate_ir::Crate;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Index into `Crate::items`.
    pub item: usize,
    /// Other items involved, e.g. the earlier definition in a collision.
    pub also: Vec<usize>,
    pub message: String,
}

impl Diagnostic {
    fn at(item: usize, message: impl Into<String>) -> Self {
        Self {
            item,
            also: Vec::new(),
            message: message.into(),
        }
    }
}

/// Empty when the crate is inside the v0 subset. Ordered by item index.
pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = names::check(krate);
    out.extend(resolve::check(krate));
    out.sort_by_key(|d| d.item);
    out
}
