//! Subset checks that must pass before anything is emitted.
//! Diagnostics point at items by index into `Crate::items`; the caller maps
//! indices to source locations.

mod binds;
mod complete;
mod consts;
mod defs;
mod exhaustive;
mod grow;
mod keys;
mod lift;
mod names;
mod order;
mod position;
mod reach;
mod rename;
mod resolve;
mod rest;
mod std_dicts;
mod traits;
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
        Self { item, also: Vec::new(), at: None, reason, detail: None, message: message.into() }
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
///
/// A `to_string` read from an `impl Display` that does not pass is dropped
/// rather than refused, as a `fmt` of another shape is skipped (design/01
/// §7.25), and the rest checked again: one that printed it fails in turn.
/// Diagnostics index `krate`'s items all the same.
pub fn accept(krate: &Crate) -> Result<Crate, Vec<Diagnostic>> {
    // The derived `cmp`s are items past the input's: a diagnostic about one
    // points at the type that derives it.
    let (derived, origin, mut found) = order::derive(krate);
    let at = |i: usize| i.checked_sub(krate.items.len()).map_or(i, |j| origin[j]);
    match accept_derived(&derived) {
        Ok(done) if found.is_empty() => Ok(done),
        Ok(_) => Err(found),
        Err(more) => {
            found.extend(more.into_iter().map(|d| Diagnostic {
                item: at(d.item),
                also: d.also.iter().map(|&i| at(i)).collect(),
                ..d
            }));
            found.sort_by_key(|d| d.item);
            Err(found)
        }
    }
}

/// The display loop of `accept` over the crate with its derived `cmp`s.
fn accept_derived(krate: &Crate) -> Result<Crate, Vec<Diagnostic>> {
    let mut kept: Vec<usize> = (0..krate.items.len()).collect();
    let mut trial = None;
    loop {
        let current = trial.as_ref().unwrap_or(krate);
        let mut out = match accept_all(current) {
            Ok(done) => return Ok(done),
            Err(out) => out,
        };
        let failed: std::collections::BTreeSet<usize> = out
            .iter()
            .map(|d| d.item)
            .filter(|i| matches!(current.items.get(*i), Some(Item::Fn(f)) if f.display))
            .collect();
        if failed.is_empty() {
            for d in &mut out {
                d.item = kept.get(d.item).copied().unwrap_or(d.item);
                for a in &mut d.also {
                    *a = kept.get(*a).copied().unwrap_or(*a);
                }
            }
            return Err(out);
        }
        kept = kept.iter().enumerate().filter(|(j, _)| !failed.contains(j)).map(|(_, i)| *i).collect();
        trial = Some(Crate::new(krate.name.as_str(), kept.iter().map(|i| krate.items[*i].clone()).collect()));
    }
}

fn accept_all(krate: &Crate) -> Result<Crate, Vec<Diagnostic>> {
    let mut out = names::check(krate);
    out.extend(resolve::check(krate));
    out.extend(exhaustive::check(krate));
    out.extend(position::check(krate));
    out.extend(wire::check(krate));
    if out.is_empty() {
        match types::elaborate(krate) {
            Ok(typed) => {
                let mut typed = traits::forward(typed);
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
                        let renamed = grow::grow(renamed)?;
                        let done = unused::drop_unused(binds::merge(lift::lift(renamed)));
                        // The emitter finds the same homes by name alone.
                        debug_assert_eq!(
                            done.homes(|item| done.fns_named(item)),
                            homes,
                            "helper homes differ after renaming"
                        );
                        return Ok(std_dicts::read_forwarded(std_dicts::hoist(done)));
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
