//! Subset checks that must pass before anything is emitted.
//! Diagnostics point at items by index into `Crate::items`; the caller maps
//! indices to source locations.

mod binds;
mod complete;
mod consts;
mod defs;
mod exhaustive;
mod grow;
mod lift;
mod names;
mod position;
mod reach;
mod rename;
mod resolve;
mod rest;
mod tuple;
mod types;
mod unused;
mod wire;

use purecrate_ir::{Crate, Item, Pos, Reason};

pub use reach::prune_unreachable;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Index into `Crate::items`.
    pub item: usize,
    /// Other items involved, e.g. the earlier definition in a collision.
    pub also: Vec<usize>,
    /// The call, statement, block tail or `match` arm the problem is in,
    /// when the input carries positions (`parse_source_spanned`). Otherwise the item.
    pub at: Option<Pos>,
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
            at: None,
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

/// Gives `at` to the diagnostics a pass reported while inside `Expr::At`.
/// The innermost `At` is left first, so it wins.
fn locate(found: &mut [Diagnostic], at: Pos) {
    for d in found {
        d.at.get_or_insert(at);
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
    out.extend(wire::check(krate));
    if out.is_empty() {
        match types::elaborate(krate) {
            Ok(mut typed) => {
                // Typing rebuilds the bodies without `At`; clones of untyped
                // subtrees may still hold one.
                for item in &mut typed.items {
                    if let Item::Fn(f) = item {
                        f.body.strip_positions();
                    }
                }
                match complete::check(&typed) {
                    missing if missing.is_empty() => {
                        let (renamed, homes) = rename::rename(rest::expand(typed));
                        let renamed = match grow::grow(renamed) {
                            Ok(k) => k,
                            Err(e) => return Err(e),
                        };
                        let done = unused::drop_unused(binds::merge(lift::lift(renamed)));
                        // The emitter finds the same homes by name alone.
                        debug_assert_eq!(done.homes(|item| done.fns_named(item)), homes, "helper homes differ after renaming");
                        return Ok(done);
                    }
                    missing => out = missing,
                }
            }
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
