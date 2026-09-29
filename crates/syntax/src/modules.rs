//! A crate of several modules, inline or in their own files, as one flat
//! list of items (design/02 §3.3):
//!
//! - Paths through the crate's modules lose that prefix: `crate::money::Yen`,
//!   `super::Yen` and `money::Yen` all name `Yen`. Names are unique across
//!   the crate (checked later), so this is exact.
//! - An item is exported when it is `pub` and every module around it is
//!   `pub`, or when a `pub use` names it (or a `pub use m::*` its module), as
//!   in Rust's public surface. Otherwise it is lowered as internal.
//! - `pub use a::B as C` would export a name the item does not have, and is
//!   rejected.

use std::collections::HashSet;

use purecrate_ir::{Crate, Reason};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::Item as SynItem;

use crate::item::{self, is_test_only, Cx, LineCol, ParseError};

/// One source file of the crate. `public` is whether every `mod` on the way
/// from the root to it is `pub` (the root's is `true`).
pub struct Source<'a> {
    pub text: &'a str,
    pub public: bool,
}

/// The root and its module files, in any order. Each item's name location
/// comes with the index of its file.
pub fn parse_files_spanned(
    crate_name: &str,
    sources: &[Source<'_>],
) -> Result<(Crate, Vec<(usize, LineCol)>), (usize, ParseError)> {
    let files = sources
        .iter()
        .map(|s| s.text)
        .enumerate()
        .map(|(i, s)| {
            syn::parse_file(s)
                .map_err(|e| (i, ParseError::new(Reason::InvalidSyntax, e.to_string()).or_at(e.span())))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut modules = HashSet::new();
    let mut reexports = Reexports::default();
    for (i, f) in files.iter().enumerate() {
        scan(&f.items, &mut modules, &mut reexports).map_err(|e| (i, e))?;
    }

    let mut flat: Vec<(usize, SynItem)> = Vec::new();
    for (i, f) in files.into_iter().enumerate() {
        flatten(f.items, sources[i].public, None, &reexports, &mut |item| flat.push((i, item)));
    }
    let mut strip = StripModules { modules: &modules };
    for (_, item) in &mut flat {
        strip.visit_item_mut(item);
    }

    let mut cx = Cx::scan_items(flat.iter().map(|(_, i)| i));
    let mut items = Vec::new();
    let mut spans = Vec::new();
    for (file, syn_item) in flat {
        for (item, at) in item::lower_item(&mut cx, syn_item).map_err(|e| (file, e))? {
            items.push(item);
            spans.push((file, at));
        }
    }
    Ok((Crate::new(crate_name, items), spans))
}

#[derive(Default)]
struct Reexports {
    /// Last segments of `pub use a::B`.
    names: HashSet<String>,
    /// Modules of `pub use a::m::*`.
    globs: HashSet<String>,
}

fn is_pub(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// Every module name, and what `pub use` re-exports.
fn scan(items: &[SynItem], modules: &mut HashSet<String>, re: &mut Reexports) -> Result<(), ParseError> {
    for item in items.iter().filter(|i| !is_test_only(i)) {
        match item {
            SynItem::Mod(m) => {
                modules.insert(m.ident.unraw().to_string());
                if let Some((_, inner)) = &m.content {
                    scan(inner, modules, re)?;
                }
            }
            SynItem::Use(u) if is_pub(&u.vis) => use_tree(&u.tree, None, re).map_err(|e| e.or_at(u.span()))?,
            _ => {}
        }
    }
    Ok(())
}

fn use_tree(tree: &syn::UseTree, last: Option<String>, re: &mut Reexports) -> Result<(), ParseError> {
    match tree {
        syn::UseTree::Path(p) => use_tree(&p.tree, Some(p.ident.unraw().to_string()), re),
        syn::UseTree::Name(n) => {
            re.names.insert(n.ident.to_string());
            Ok(())
        }
        syn::UseTree::Glob(_) => {
            if let Some(m) = last {
                re.globs.insert(m);
            }
            Ok(())
        }
        syn::UseTree::Group(g) => g.items.iter().try_for_each(|t| use_tree(t, last.clone(), re)),
        syn::UseTree::Rename(r) => Err(ParseError::new(
            Reason::UnsupportedItem,
            format!("`pub use .. as {}` renames an export, which v0 does not model; export the item under its own name", r.rename),
        )
        .detail("use-rename")),
    }
}

/// Items of `items` and their inline modules, with `pub` taken away where
/// Rust would not export the item.
fn flatten(
    items: Vec<SynItem>,
    public: bool,
    module: Option<&str>,
    re: &Reexports,
    out: &mut impl FnMut(SynItem),
) {
    for mut item in items.into_iter().filter(|i| !is_test_only(i)) {
        match item {
            SynItem::Mod(m) => {
                if let Some((_, inner)) = m.content {
                    let name = m.ident.unraw().to_string();
                    flatten(inner, public && is_pub(&m.vis), Some(&name), re, out);
                }
            }
            SynItem::Use(_) => {}
            _ => {
                let reexported = item_name(&item).is_some_and(|n| re.names.contains(&n))
                    || module.is_some_and(|m| re.globs.contains(m));
                if !public && !reexported {
                    demote(&mut item);
                }
                out(item);
            }
        }
    }
}

fn item_name(item: &SynItem) -> Option<String> {
    match item {
        SynItem::Struct(s) => Some(s.ident.to_string()),
        SynItem::Enum(e) => Some(e.ident.to_string()),
        SynItem::Fn(f) => Some(f.sig.ident.to_string()),
        SynItem::Type(t) => Some(t.ident.to_string()),
        _ => None,
    }
}

fn demote(item: &mut SynItem) {
    let vis = match item {
        SynItem::Struct(s) => &mut s.vis,
        SynItem::Enum(e) => &mut e.vis,
        SynItem::Fn(f) => &mut f.vis,
        SynItem::Type(t) => &mut t.vis,
        _ => return,
    };
    *vis = syn::Visibility::Inherited;
}

/// Drops the leading `crate`, `self`, `super`, and module segments of a path.
struct StripModules<'a> {
    modules: &'a HashSet<String>,
}

impl VisitMut for StripModules<'_> {
    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        if path.leading_colon.is_none() {
            while path.segments.len() > 1 {
                let first = path.segments[0].ident.unraw().to_string();
                let module = first == "crate" || first == "self" || first == "super" || self.modules.contains(&first);
                if !module || !path.segments[0].arguments.is_empty() {
                    break;
                }
                let rest: syn::punctuated::Punctuated<syn::PathSegment, syn::Token![::]> =
                    path.segments.iter().skip(1).cloned().collect();
                path.segments = rest;
            }
        }
        visit_mut::visit_path_mut(self, path);
    }
}
