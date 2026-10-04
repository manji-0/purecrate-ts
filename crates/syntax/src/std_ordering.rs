//! `std::cmp::Ordering` (design/01 §6.1). A crate names it after `use
//! std::cmp::Ordering;` (or `core::`, alone or in a group), or as
//! `std::cmp::Ordering` in full; every full path is shortened to
//! `Ordering`, and the IR crate gets the enum (`Enum::std_ordering`), so
//! `Ordering::Less` is a variant like any other. `x.cmp(&y)` gives one too,
//! so a crate that only calls `cmp` gets it as well, internal. rustc still
//! compiles the original source against std's type.
//!
//! Refused: a crate `Ordering` beside std's, a renaming or glob `use`, the
//! variants imported bare (a bare `Less` would be a binding here), and
//! `cmp::Ordering` through `use std::cmp;`.

use std::collections::HashSet;

use purecrate_ir::{Reason, Vis, ORDERING};
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::Item as SynItem;

use crate::item::{is_test_only, LineCol, ParseError};

/// A file index and a position in it.
type At = (usize, LineCol);

/// Where, by file index and position, the source first names std's
/// `Ordering` or calls `cmp`, and whether the crate has its own `Ordering`.
#[derive(Default)]
pub(crate) struct StdOrdering {
    named: Option<At>,
    cmp_call: Option<At>,
    own: bool,
}

impl StdOrdering {
    /// Shortens the paths to std's `Ordering` in the items of file `file`
    /// and notes what they name. `modules` are the crate's own: `cmp::`
    /// through one of them is not std's.
    pub(crate) fn rewrite<'a>(
        &mut self,
        file: usize,
        items: impl IntoIterator<Item = &'a mut SynItem>,
        modules: &HashSet<String>,
    ) -> Result<(), ParseError> {
        let mut visit = Visit { found: self, file, modules, error: None };
        for item in items {
            visit.visit_item_mut(item);
        }
        visit.error.map_or(Ok(()), Err)
    }

    /// The enum to add, and where the crate names it: exported when it
    /// does, else internal when a `cmp` call may give one. A crate `Ordering`
    /// beside std's is refused.
    pub(crate) fn injected(&self) -> Result<Option<(Vis, At)>, (usize, ParseError)> {
        match (self.named, self.cmp_call) {
            (Some((file, at)), _) if self.own => {
                let mut e = ParseError::new(
                    Reason::NameCollision,
                    format!("the crate defines its own `{ORDERING}` and also names `std::cmp::{ORDERING}`; rename the crate's type"),
                );
                e.at = Some(at);
                Err((file, e))
            }
            (Some(at), _) => Ok(Some((Vis::Pub, at))),
            (None, Some(at)) if !self.own => Ok(Some((Vis::Internal, at))),
            _ => Ok(None),
        }
    }
}

struct Visit<'a> {
    found: &'a mut StdOrdering,
    file: usize,
    modules: &'a HashSet<String>,
    error: Option<ParseError>,
}

impl Visit<'_> {
    fn fail(&mut self, e: ParseError) {
        self.error.get_or_insert(e);
    }

    fn name(&mut self, span: proc_macro2::Span) {
        self.found.named.get_or_insert((self.file, LineCol::of(span)));
    }

    fn cmp_call(&mut self, span: proc_macro2::Span) {
        self.found.cmp_call.get_or_insert((self.file, LineCol::of(span)));
    }

    /// The leaves of `use` tree `tree` under `prefix`.
    fn use_tree(&mut self, tree: &syn::UseTree, prefix: &mut Vec<String>) {
        let std_cmp = |p: &[String]| p.len() >= 2 && (p[0] == "std" || p[0] == "core") && p[1] == "cmp";
        let fix = format!("write `use std::cmp::{ORDERING};` and name `{ORDERING}::Less`");
        match tree {
            syn::UseTree::Path(p) => {
                prefix.push(p.ident.to_string());
                self.use_tree(&p.tree, prefix);
                prefix.pop();
            }
            syn::UseTree::Name(n) if std_cmp(prefix) && prefix.len() == 2 && n.ident == ORDERING => self.name(n.span()),
            syn::UseTree::Name(n) if std_cmp(prefix) && prefix.len() == 3 && prefix[2] == ORDERING => {
                self.fail(ParseError::new(Reason::UnsupportedItem, format!(
                    "importing the variants of `std::cmp::{ORDERING}` is not in v0: a bare `{}` in a pattern would read as a binding; {fix}",
                    n.ident
                ))
                .detail("use-ordering")
                .or_at(n.span()));
            }
            syn::UseTree::Rename(r) if std_cmp(prefix) && prefix.len() == 2 && r.ident == ORDERING => {
                self.fail(ParseError::new(Reason::UnsupportedItem, format!(
                    "`use std::cmp::{ORDERING} as {}` renames std's `{ORDERING}`, which v0 does not model; write `use std::cmp::{ORDERING};`",
                    r.rename
                ))
                .detail("use-rename")
                .or_at(r.span()));
            }
            syn::UseTree::Glob(g) if std_cmp(prefix) && prefix.len() <= 3 => {
                self.fail(
                    ParseError::new(
                        Reason::UnsupportedItem,
                        format!("`use {}::*` is not in v0; {fix}", prefix.join("::")),
                    )
                    .detail("use-ordering")
                    .or_at(g.span()),
                );
            }
            syn::UseTree::Group(g) => g.items.iter().for_each(|t| self.use_tree(t, prefix)),
            _ => {}
        }
    }

    /// `tokens` with `std::cmp::` (or `core::cmp::`) dropped before each
    /// `Ordering`, in nested groups too.
    fn tokens(&mut self, tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        use proc_macro2::{Group, TokenTree};
        let tts: Vec<TokenTree> = tokens.into_iter().collect();
        let ident = |t: Option<&TokenTree>, name: &str| matches!(t, Some(TokenTree::Ident(i)) if i == name);
        let colon = |t: Option<&TokenTree>| matches!(t, Some(TokenTree::Punct(p)) if p.as_char() == ':');
        let path_sep = |i: usize| colon(tts.get(i)) && colon(tts.get(i + 1));
        let mut out: Vec<TokenTree> = Vec::new();
        let mut i = 0;
        while i < tts.len() {
            let std_cmp = (ident(tts.get(i), "std") || ident(tts.get(i), "core"))
                && path_sep(i + 1)
                && ident(tts.get(i + 3), "cmp")
                && path_sep(i + 4)
                && ident(tts.get(i + 6), ORDERING);
            if std_cmp {
                self.name(tts[i].span());
                // A leading `::` goes with the path.
                if out.len() >= 2 && colon(out.last()) && colon(out.get(out.len() - 2)) {
                    out.truncate(out.len() - 2);
                }
                i += 6;
                continue;
            }
            let dot = i >= 1 && matches!(tts.get(i - 1), Some(TokenTree::Punct(p)) if p.as_char() == '.');
            if dot && ident(tts.get(i), "cmp") && matches!(tts.get(i + 1), Some(TokenTree::Group(_))) {
                self.cmp_call(tts[i].span());
            }
            let after_sep = i >= 2 && path_sep(i - 2);
            if ident(tts.get(i), "cmp")
                && !after_sep
                && path_sep(i + 1)
                && ident(tts.get(i + 3), ORDERING)
                && !self.modules.contains("cmp")
            {
                self.fail(
                    ParseError::new(
                        Reason::QualifiedPath,
                        format!(
                        "`cmp::{ORDERING}` is not in v0; write `use std::cmp::{ORDERING};` and name it `{ORDERING}`"
                    ),
                    )
                    .detail(format!("cmp::{ORDERING}"))
                    .or_at(tts[i].span()),
                );
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

impl VisitMut for Visit<'_> {
    fn visit_item_mut(&mut self, item: &mut SynItem) {
        if is_test_only(item) {
            return;
        }
        match item {
            SynItem::Use(u) => self.use_tree(&u.tree, &mut Vec::new()),
            SynItem::Enum(e) if e.ident == ORDERING => self.found.own = true,
            SynItem::Struct(s) if s.ident == ORDERING => self.found.own = true,
            SynItem::Type(t) if t.ident == ORDERING => self.found.own = true,
            _ => {}
        }
        visit_mut::visit_item_mut(self, item);
    }

    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        match segs.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
            ["std" | "core", "cmp", o, ..]
                if *o == ORDERING && path.segments.iter().take(2).all(|s| s.arguments.is_empty()) =>
            {
                self.name(path.span());
                path.leading_colon = None;
                path.segments = path.segments.iter().skip(2).cloned().collect();
            }
            ["cmp", o, ..] if *o == ORDERING && path.leading_colon.is_none() && !self.modules.contains("cmp") => {
                self.fail(
                    ParseError::new(
                        Reason::QualifiedPath,
                        format!(
                        "`cmp::{ORDERING}` is not in v0; write `use std::cmp::{ORDERING};` and name it `{ORDERING}`"
                    ),
                    )
                    .detail(format!("cmp::{ORDERING}"))
                    .or_at(path.span()),
                );
            }
            _ => {}
        }
        visit_mut::visit_path_mut(self, path);
    }

    /// A macro's body is parsed only when it is lowered (`matches!`,
    /// `vec!`), so its paths are shortened here as tokens.
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        let tokens = std::mem::take(&mut mac.tokens);
        mac.tokens = self.tokens(tokens);
        visit_mut::visit_macro_mut(self, mac);
    }

    fn visit_expr_method_call_mut(&mut self, call: &mut syn::ExprMethodCall) {
        if call.method == "cmp" {
            self.cmp_call(call.method.span());
        }
        visit_mut::visit_expr_method_call_mut(self, call);
    }
}
