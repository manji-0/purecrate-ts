//! Lowering for measurement: every item of a module tree is lowered on its
//! own, so one unsupported item does not hide the rest.

use std::collections::HashSet;

use purecrate_ir::{Enum, Item, Reason, ORDERING};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::Item as SynItem;

use crate::item::{self, is_test_only, Cx, LineCol, ParseError};
use crate::std_ordering::StdOrdering;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Fn,
    Method { owner: String },
    Struct,
    Enum,
    Alias,
    /// Anything else that may be referenced or may hold code, e.g. `const`,
    /// `trait`, a trait impl. `what` names it for tallying.
    Other { what: &'static str },
}

#[derive(Clone, Debug)]
pub struct Unit {
    /// Index into the files given to `survey_files`.
    pub file: usize,
    pub at: LineCol,
    pub kind: UnitKind,
    pub name: String,
    pub public: bool,
    pub lowered: Result<Item, ParseError>,
    /// With `all_causes`: every expression or statement of the item that
    /// could not be lowered (and was stood in for), in source order. The
    /// item is then outside the subset even though `lowered` is `Ok`.
    pub causes: Vec<ParseError>,
}

/// Out-of-line `mod x;` declarations, as paths relative to the declaring
/// file's module directory: `mod a { mod b; }` gives `["a", "b"]`.
pub fn module_decls(source: &str) -> Result<Vec<Vec<String>>, ParseError> {
    let file = parse(source)?;
    let mut out = Vec::new();
    collect_mods(&file.items, &mut Vec::new(), &mut out);
    Ok(out)
}

/// `module_decls` with whether each is reached through `pub` modules only.
pub fn module_decls_vis(source: &str) -> Result<Vec<(Vec<String>, bool)>, ParseError> {
    fn walk(items: &[SynItem], prefix: &mut Vec<String>, public: bool, out: &mut Vec<(Vec<String>, bool)>) {
        for item in items.iter().filter(|i| !is_test_only(i)) {
            if let SynItem::Mod(m) = item {
                let public = public && matches!(m.vis, syn::Visibility::Public(_));
                prefix.push(m.ident.unraw().to_string());
                match &m.content {
                    Some((_, inner)) => walk(inner, prefix, public, out),
                    None => out.push((prefix.clone(), public)),
                }
                prefix.pop();
            }
        }
    }
    let file = parse(source)?;
    let mut out = Vec::new();
    walk(&file.items, &mut Vec::new(), true, &mut out);
    Ok(out)
}

fn collect_mods(items: &[SynItem], prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    for item in items.iter().filter(|i| !is_test_only(i)) {
        if let SynItem::Mod(m) = item {
            prefix.push(m.ident.unraw().to_string());
            match &m.content {
                Some((_, inner)) => collect_mods(inner, prefix, out),
                None => out.push(prefix.clone()),
            }
            prefix.pop();
        }
    }
}

/// A file `syn` cannot parse fails the whole survey; everything else is
/// reported per unit. With `all_causes`, an item is lowered past the
/// expressions it cannot take, and each of them is a cause (`Unit::causes`).
pub fn survey_files(sources: &[&str], all_causes: bool) -> Result<Vec<Unit>, (usize, ParseError)> {
    let mut files = sources
        .iter()
        .enumerate()
        .map(|(i, s)| parse(s).map_err(|e| (i, e)))
        .collect::<Result<Vec<_>, _>>()?;
    // std's `Ordering` as the parser adds it. A form it refuses stays as
    // written, and the units that use it are refused when lowered.
    let mut modules = HashSet::new();
    for f in &files {
        module_names(&f.items, &mut modules);
    }
    let mut ordering = StdOrdering::default();
    for (i, f) in files.iter_mut().enumerate() {
        let _ = ordering.rewrite(i, &mut f.items, &modules);
    }
    let injected = ordering.injected().ok().flatten();
    let mut all = Vec::new();
    for f in &files {
        flatten(&f.items, &mut all);
    }
    let mut cx = Cx::scan_items(all.iter().copied());
    let mut units = Vec::new();
    if let Some((vis, (file, at))) = injected {
        cx.add_std_ordering();
        units.push(Unit {
            file,
            at,
            kind: UnitKind::Enum,
            name: ORDERING.to_string(),
            // Not the crate's own, so not counted among its public types.
            public: false,
            lowered: Ok(Item::Enum(Enum::std_ordering(vis))),
            causes: Vec::new(),
        });
    }
    for (i, f) in files.iter().enumerate() {
        let mut items = Vec::new();
        flatten(&f.items, &mut items);
        for item in items {
            units.extend(units_of(&mut cx, i, item, all_causes));
        }
    }
    Ok(units)
}

fn module_names(items: &[SynItem], out: &mut HashSet<String>) {
    for item in items {
        if let SynItem::Mod(m) = item {
            out.insert(m.ident.unraw().to_string());
            if let Some((_, inner)) = &m.content {
                module_names(inner, out);
            }
        }
    }
}

fn parse(source: &str) -> Result<syn::File, ParseError> {
    syn::parse_file(source).map_err(|e| ParseError::new(Reason::InvalidSyntax, e.to_string()).or_at(e.span()))
}

/// Items of the file and of its inline modules, test-only ones left out.
fn flatten<'a>(items: &'a [SynItem], out: &mut Vec<&'a SynItem>) {
    for item in items.iter().filter(|i| !is_test_only(i)) {
        match item {
            SynItem::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    flatten(inner, out);
                }
            }
            SynItem::Use(_) => {}
            other => out.push(other),
        }
    }
}

/// Lowers `item` to the one IR item it makes, recording in recovery mode
/// the causes it stood in for.
fn lower_one(cx: &mut Cx, item: SynItem, all_causes: bool, what: &str) -> (Result<Item, ParseError>, Vec<ParseError>) {
    let what = what.to_string();
    let run = move |cx: &mut Cx| {
        item::lower_item(cx, item).and_then(|mut items| match items.len() {
            1 => Ok(items.remove(0).0),
            _ => Err(ParseError::new(Reason::UnsupportedItem, format!("expected one {what}"))),
        })
    };
    if all_causes {
        cx.recovering(run)
    } else {
        (run(cx), Vec::new())
    }
}

fn units_of(cx: &mut Cx, file: usize, item: &SynItem, all_causes: bool) -> Vec<Unit> {
    let unit = |kind, name: String, public, at, (lowered, causes): (Result<Item, ParseError>, Vec<ParseError>)| Unit {
        file,
        at,
        kind,
        name,
        public,
        lowered,
        causes,
    };
    let one = |cx: &mut Cx, kind, name: String, public, at| {
        vec![unit(kind, name, public, at, lower_one(cx, item.clone(), all_causes, "item"))]
    };
    let public = |vis: &syn::Visibility| matches!(vis, syn::Visibility::Public(_));
    match item {
        SynItem::Fn(f) => one(cx, UnitKind::Fn, f.sig.ident.to_string(), public(&f.vis), LineCol::of(f.sig.ident.span())),
        SynItem::Struct(s) => one(cx, UnitKind::Struct, s.ident.to_string(), public(&s.vis), LineCol::of(s.ident.span())),
        SynItem::Enum(e) => one(cx, UnitKind::Enum, e.ident.to_string(), public(&e.vis), LineCol::of(e.ident.span())),
        SynItem::Type(t) => one(cx, UnitKind::Alias, t.ident.to_string(), public(&t.vis), LineCol::of(t.ident.span())),
        SynItem::Impl(imp) if imp.trait_.is_none() => imp
            .items
            .iter()
            .map(|member| {
                let mut single = imp.clone();
                single.items = vec![member.clone()];
                let owner = self_name(&imp.self_ty);
                let (name, is_pub, at) = match member {
                    syn::ImplItem::Fn(f) => (f.sig.ident.to_string(), public(&f.vis), LineCol::of(f.sig.ident.span())),
                    other => ("?".to_string(), false, LineCol::of(other.span())),
                };
                let lowered = lower_one(cx, SynItem::Impl(single), all_causes, "method");
                unit(UnitKind::Method { owner }, name, is_pub, at, lowered)
            })
            .collect(),
        // `impl TryFrom<T> for X` is the method `X::try_from`, which
        // `#[serde(try_from = "T")]` on `X` needs. Not counted as a public
        // function, as the trait impl is not one in Rust.
        SynItem::Impl(imp) if is_try_from_impl(imp) => {
            let lowered = lower_one(cx, item.clone(), all_causes, "method");
            vec![unit(
                UnitKind::Method { owner: self_name(&imp.self_ty) },
                "try_from".to_string(),
                false,
                LineCol::of(imp.span()),
                lowered,
            )]
        }
        other => {
            let what = match other {
                SynItem::Impl(_) => "trait impl",
                SynItem::Const(_) => "const",
                SynItem::Static(_) => "static",
                SynItem::Trait(_) => "trait",
                SynItem::Macro(_) => "macro",
                _ => "other",
            };
            let name = match other {
                SynItem::Impl(imp) => format!(
                    "{} for {}",
                    imp.trait_.as_ref().map(|(_, p, _)| path_text(p)).unwrap_or_default(),
                    self_name(&imp.self_ty)
                ),
                SynItem::Const(c) => c.ident.to_string(),
                SynItem::Static(s) => s.ident.to_string(),
                SynItem::Trait(t) => t.ident.to_string(),
                _ => String::new(),
            };
            let lowered = (
                item::lower_item(cx, other.clone())
                    .and_then(|_| Err(ParseError::new(Reason::UnsupportedItem, format!("{what} is not in v0")))),
                Vec::new(),
            );
            vec![unit(UnitKind::Other { what }, name, false, LineCol::of(other.span()), lowered)]
        }
    }
}

fn is_try_from_impl(imp: &syn::ItemImpl) -> bool {
    imp.trait_
        .as_ref()
        .and_then(|(_, p, _)| p.segments.last())
        .is_some_and(|s| s.ident == "TryFrom")
}

fn self_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => path_text(&p.path),
        other => other.span().source_text().unwrap_or_default(),
    }
}

fn path_text(p: &syn::Path) -> String {
    p.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
