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
use crate::std_ordering;

/// One source file of the crate. `public` is whether every `mod` on the way
/// from the root to it is `pub` (the root's is `true`).
pub struct Source<'a> {
    pub text: &'a str,
    pub public: bool,
}

/// The root and its module files, in any order. Each item's name location
/// comes with the index of its file.
/// Each item's file index and name location, in item order.
pub type ItemSpans = Vec<(usize, LineCol)>;

pub fn parse_files_spanned(
    crate_name: &str,
    sources: &[Source<'_>],
) -> Result<(Crate, ItemSpans), (usize, ParseError)> {
    let mut files = sources
        .iter()
        .enumerate()
        .map(|(i, s)| item::parse(s.text).map_err(|e| (i, e)))
        .collect::<Result<Vec<_>, _>>()?;

    let mut modules = HashSet::new();
    let mut reexports = Reexports::default();
    for (i, f) in files.iter().enumerate() {
        scan(&f.items, &mut modules, &mut reexports).map_err(|e| (i, e))?;
    }

    // Before `flatten`, which drops the `use` items.
    let mut ordering = std_ordering::StdOrdering::default();
    for (i, f) in files.iter_mut().enumerate() {
        ordering.rewrite(i, &mut f.items, &modules).map_err(|e| (i, e))?;
    }
    let injected = ordering.injected()?;

    let mut flat: Vec<(usize, SynItem)> = Vec::new();
    for (i, f) in files.into_iter().enumerate() {
        flatten(f.items, sources[i].public, None, &reexports, &mut |item| flat.push((i, item)));
    }
    for (_, item) in &mut flat {
        strip_module_paths(item, &modules);
    }

    let mut cx = Cx::scan_items(flat.iter().map(|(_, i)| i));
    if injected.is_some() {
        cx.add_std_ordering();
    }
    let mut items = Vec::new();
    let mut spans = Vec::new();
    for (file, syn_item) in flat {
        cx.set_source(sources[file].text);
        for (item, at) in item::lower_item(&mut cx, syn_item).map_err(|e| (file, e))? {
            items.push(item);
            spans.push((file, at));
        }
    }
    if let Some((vis, (file, at))) = injected {
        items.push(purecrate_ir::Item::Enum(purecrate_ir::Enum::std_ordering(vis)));
        spans.push((file, at));
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
            format!(
                "`pub use .. as {}` renames an export, which v0 does not model; export the item under its own name",
                r.rename
            ),
        )
        .detail("use-rename")),
    }
}

/// Items of `items` and their inline modules, with `pub` taken away where
/// Rust would not export the item.
fn flatten(items: Vec<SynItem>, public: bool, module: Option<&str>, re: &Reexports, out: &mut impl FnMut(SynItem)) {
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
                let reexported = item::item_ident(&item).is_some_and(|n| re.names.contains(&n.to_string()))
                    || module.is_some_and(|m| re.globs.contains(m));
                if !public && !reexported {
                    demote(&mut item);
                }
                out(item);
            }
        }
    }
}

fn demote(item: &mut SynItem) {
    let vis = match item {
        SynItem::Struct(s) => &mut s.vis,
        SynItem::Enum(e) => &mut e.vis,
        SynItem::Fn(f) => &mut f.vis,
        SynItem::Type(t) => &mut t.vis,
        SynItem::Const(c) => &mut c.vis,
        SynItem::Trait(t) => &mut t.vis,
        _ => return,
    };
    *vis = syn::Visibility::Inherited;
}

/// Drops the leading `crate`, `self`, `super`, and module segments (of the
/// crate's `modules`) of every path in `item`.
pub(crate) fn strip_module_paths(item: &mut SynItem, modules: &HashSet<String>) {
    StripModules { modules }.visit_item_mut(item);
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

    /// A macro's body is parsed only when it is lowered (`matches!`,
    /// `assert!`), so its paths are shortened here as tokens.
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        let tokens = std::mem::take(&mut mac.tokens);
        mac.tokens = self.tokens(tokens);
        visit_mut::visit_macro_mut(self, mac);
    }
}

impl StripModules<'_> {
    /// `tokens` with the module segments that start a path dropped, in
    /// nested groups too. A segment after `::` (`::m::f`, `a::m`) or `.`
    /// does not start one.
    fn tokens(&self, tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        use proc_macro2::{Group, TokenTree};
        let tts: Vec<TokenTree> = tokens.into_iter().collect();
        let punct = |t: Option<&TokenTree>, c: char| matches!(t, Some(TokenTree::Punct(p)) if p.as_char() == c);
        let path_sep = |i: usize| punct(tts.get(i), ':') && punct(tts.get(i + 1), ':');
        let module = |i: usize| match tts.get(i) {
            Some(TokenTree::Ident(id)) => {
                let name = id.unraw().to_string();
                name == "crate" || name == "self" || name == "super" || self.modules.contains(&name)
            }
            _ => false,
        };
        let mut out: Vec<TokenTree> = Vec::new();
        let mut i = 0;
        while i < tts.len() {
            let starts = !(i >= 2 && path_sep(i - 2)) && !(i >= 1 && punct(tts.get(i - 1), '.'));
            if starts {
                // `m::` before another segment, as many times as it appears.
                while module(i) && path_sep(i + 1) && matches!(tts.get(i + 3), Some(TokenTree::Ident(_))) {
                    i += 3;
                }
            }
            out.push(match &tts[i] {
                TokenTree::Group(g) => {
                    let mut inner = Group::new(g.delimiter(), self.tokens(g.stream()));
                    inner.set_span(g.span());
                    TokenTree::Group(inner)
                }
                other => other.clone(),
            });
            i += 1;
        }
        out.into_iter().collect()
    }
}
