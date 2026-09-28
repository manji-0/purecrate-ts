//! Subset checks that must pass before anything is emitted.
//! Diagnostics point at items by index into `Crate::items`; the caller maps
//! indices to source locations.

mod complete;
mod defs;
mod exhaustive;
mod lift;
mod names;
mod position;
mod reach;
mod rename;
mod resolve;
mod types;

use purecrate_ir::{Crate, Reason};

pub use reach::prune_unreachable;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Index into `Crate::items`.
    pub item: usize,
    /// Other items involved, e.g. the earlier definition in a collision.
    pub also: Vec<usize>,
    pub reason: Reason,
    /// What the reason is about when that varies, e.g. the undefined name.
    pub detail: Option<String>,
    pub message: String,
}

impl Diagnostic {
    fn at(item: usize, reason: Reason, message: impl Into<String>) -> Self {
        Self {
            item,
            also: Vec::new(),
            reason,
            detail: None,
            message: message.into(),
        }
    }

    fn about(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    fn also(mut self, other: usize) -> Self {
        self.also.push(other);
        self
    }
}

/// The crate ready to print, or why it is outside the v0 subset.
/// Numeric code is rewritten so the printed TS keeps Rust's debug-build
/// semantics; run this once, on parser output.
pub fn accept(krate: &Crate) -> Result<Crate, Vec<Diagnostic>> {
    let mut out = names::check(krate);
    out.extend(resolve::check(krate));
    out.extend(exhaustive::check(krate));
    out.extend(position::check(krate));
    if out.is_empty() {
        match types::elaborate(krate) {
            Ok(typed) => match complete::check(&typed) {
                missing if missing.is_empty() => return Ok(lift::lift(rename::rename(typed))),
                missing => out = missing,
            },
            Err(errors) => out = errors,
        }
    }
    out.sort_by_key(|d| d.item);
    Err(out)
}

/// Empty when the crate is inside the v0 subset. Ordered by item index.
pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    accept(krate).err().unwrap_or_default()
}
