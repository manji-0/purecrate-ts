//! Lowering for measurement: every item of a module tree is lowered on its
//! own, so one unsupported item does not hide the rest.

use std::collections::HashSet;

use purecrate_ir::{Enum, Item, Reason, ORDERING};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::Item as SynItem;

use crate::item::{self, is_test_only, parse, path_text, Cx, LineCol, ParseError};
use crate::std_ordering::StdOrdering;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitKind {
    Fn,
    Method {
        owner: String,
    },
    Struct,
    Enum,
    Alias,
    /// A trait of the crate (design/01 §7.25): referenced as a type, by a
    /// bound's dictionary. Not judged on its own.
    Trait,
    /// A `const`, or an associated one of `owner`'s `impl`. Lowered, as the
    /// functions that read it need it, but not judged on its own.
    Const {
        owner: Option<String>,
    },
    /// `impl Display for X`: the method `X::to_string` where its `fmt` is
    /// translated (design/01 §7.25), which `x.to_string()` and `format!`
    /// reach; tallied as a trait impl, not judged on its own.
    Display {
        owner: String,
    },
    /// Anything else that may be referenced or may hold code, e.g. `const`,
    /// `trait`, a trait impl. `what` names it for tallying.
    Other {
        what: &'static str,
    },
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
    Ok(module_decls_vis(source)?.into_iter().map(|(path, _)| path).collect())
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

/// A file `syn` cannot parse fails the whole survey; everything else is
/// reported per unit. With `all_causes`, an item is lowered past the
/// expressions it cannot take, and each of them is a cause (`Unit::causes`).
pub fn survey_files(sources: &[&str], all_causes: bool) -> Result<Vec<Unit>, (usize, ParseError)> {
    let mut files =
        sources.iter().enumerate().map(|(i, s)| parse(s).map_err(|e| (i, e))).collect::<Result<Vec<_>, _>>()?;
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
    // As `parse_files_spanned` does: `crate::m::f` and `m::f` name `f`.
    for f in &mut files {
        for item in &mut f.items {
            crate::modules::strip_module_paths(item, &modules);
        }
    }
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
        SynItem::Fn(f) => {
            one(cx, UnitKind::Fn, f.sig.ident.to_string(), public(&f.vis), LineCol::of(f.sig.ident.span()))
        }
        SynItem::Struct(s) if derives_default(&s.attrs) => {
            with_default(cx, item, all_causes, UnitKind::Struct, s.ident.to_string(), public(&s.vis), file)
        }
        SynItem::Enum(e) if derives_default(&e.attrs) => {
            with_default(cx, item, all_causes, UnitKind::Enum, e.ident.to_string(), public(&e.vis), file)
        }
        SynItem::Struct(s) => {
            one(cx, UnitKind::Struct, s.ident.to_string(), public(&s.vis), LineCol::of(s.ident.span()))
        }
        SynItem::Enum(e) => one(cx, UnitKind::Enum, e.ident.to_string(), public(&e.vis), LineCol::of(e.ident.span())),
        SynItem::Type(t) => one(cx, UnitKind::Alias, t.ident.to_string(), public(&t.vis), LineCol::of(t.ident.span())),
        SynItem::Const(c) => {
            one(cx, UnitKind::Const { owner: None }, c.ident.to_string(), public(&c.vis), LineCol::of(c.ident.span()))
        }
        SynItem::Impl(imp) if imp.trait_.is_none() => imp
            .items
            .iter()
            .map(|member| {
                let mut single = imp.clone();
                single.items = vec![member.clone()];
                let owner = self_name(&imp.self_ty);
                let (name, is_pub, at) = match member {
                    syn::ImplItem::Fn(f) => (f.sig.ident.to_string(), public(&f.vis), LineCol::of(f.sig.ident.span())),
                    syn::ImplItem::Const(c) => (c.ident.to_string(), public(&c.vis), LineCol::of(c.ident.span())),
                    other => ("?".to_string(), false, LineCol::of(other.span())),
                };
                let lowered = lower_one(cx, SynItem::Impl(single), all_causes, "method");
                let kind = match member {
                    syn::ImplItem::Const(_) => UnitKind::Const { owner: Some(owner) },
                    _ => UnitKind::Method { owner },
                };
                unit(kind, name, is_pub, at, lowered)
            })
            .collect(),
        // A crate trait (design/01 §7.25): its record, then each default
        // body as a method of the trait.
        SynItem::Trait(t) => {
            let at = LineCol::of(t.ident.span());
            let name = t.ident.to_string();
            match item::lower_item(cx, item.clone()) {
                Ok(lowered) => lowered
                    .into_iter()
                    .map(|(lowered, at)| {
                        let (kind, member) = match &lowered {
                            Item::Fn(f) => (UnitKind::Method { owner: name.clone() }, f.name.as_str().to_string()),
                            _ => (UnitKind::Trait, name.clone()),
                        };
                        unit(kind, member, false, at, (Ok(lowered), Vec::new()))
                    })
                    .collect(),
                Err(e) => vec![unit(UnitKind::Trait, name, false, at, (Err(e), Vec::new()))],
            }
        }
        // `impl Tr for X` of a crate trait: each member a method (or const)
        // of `X`, as an inherent one is.
        SynItem::Impl(imp)
            if imp.trait_.as_ref().and_then(|(_, p, _)| p.get_ident()).is_some_and(|i| cx.is_trait(&i.to_string())) =>
        {
            imp.items
                .iter()
                .map(|member| {
                    let mut single = imp.clone();
                    single.items = vec![member.clone()];
                    let owner = self_name(&imp.self_ty);
                    let (name, at) = match member {
                        syn::ImplItem::Fn(f) => (f.sig.ident.to_string(), LineCol::of(f.sig.ident.span())),
                        syn::ImplItem::Const(c) => (c.ident.to_string(), LineCol::of(c.ident.span())),
                        other => ("?".to_string(), LineCol::of(other.span())),
                    };
                    // The consts the impl takes from the trait's defaults
                    // follow the member: only the member is this unit.
                    let lowered = item::lower_item(cx, SynItem::Impl(single))
                        .map(|items| items.into_iter().next().map(|(i, _)| i))
                        .and_then(|i| i.ok_or_else(|| ParseError::new(Reason::UnsupportedItem, "expected one method")));
                    let kind = match member {
                        syn::ImplItem::Const(_) => UnitKind::Const { owner: Some(owner) },
                        _ => UnitKind::Method { owner },
                    };
                    unit(kind, name, false, at, (lowered, Vec::new()))
                })
                .collect()
        }
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
        // `impl From<S>` and `impl Default` are a member of the type's
        // companion (`fromS`, `default`), as `try_from` is.
        SynItem::Impl(imp) if is_member_impl(imp) => {
            let lowered = lower_one(cx, item.clone(), all_causes, "method");
            let name = match &lowered.0 {
                Ok(it) => it.name().as_str().to_string(),
                Err(_) => imp.trait_.as_ref().map(|(_, p, _)| path_text(p)).unwrap_or_default().to_lowercase(),
            };
            vec![unit(
                UnitKind::Method { owner: self_name(&imp.self_ty) },
                name,
                false,
                LineCol::of(imp.span()),
                lowered,
            )]
        }
        // A `fmt` of another shape is skipped: no item, so nothing to reach.
        SynItem::Impl(imp) if is_display_impl(imp) => {
            let lowered = lower_one(cx, item.clone(), all_causes, "method");
            let name = format!("{} for {}", trait_text(imp), self_name(&imp.self_ty));
            vec![unit(
                UnitKind::Display { owner: self_name(&imp.self_ty) },
                name,
                false,
                LineCol::of(imp.span()),
                lowered,
            )]
        }
        // `impl Ord for X` is the method `X::cmp`, which ordering an `X`
        // calls (design/01 §7.25). Not a public function either.
        SynItem::Impl(imp) if is_ord_impl(imp) => {
            let lowered = lower_one(cx, item.clone(), all_causes, "method");
            vec![unit(
                UnitKind::Method { owner: self_name(&imp.self_ty) },
                "cmp".to_string(),
                false,
                LineCol::of(imp.span()),
                lowered,
            )]
        }
        other => {
            let what = match other {
                SynItem::Impl(_) => "trait impl",
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

fn is_display_impl(imp: &syn::ItemImpl) -> bool {
    imp.trait_.as_ref().and_then(|(_, p, _)| p.segments.last()).is_some_and(|s| s.ident == "Display")
}

fn trait_text(imp: &syn::ItemImpl) -> String {
    imp.trait_.as_ref().map(|(_, p, _)| path_text(p)).unwrap_or_default()
}

fn is_try_from_impl(imp: &syn::ItemImpl) -> bool {
    imp.trait_.as_ref().and_then(|(_, p, _)| p.segments.last()).is_some_and(|s| s.ident == "TryFrom")
}

fn is_member_impl(imp: &syn::ItemImpl) -> bool {
    imp.trait_
        .as_ref()
        .and_then(|(_, p, _)| p.segments.last())
        .is_some_and(|s| s.ident == "From" || s.ident == "Default")
}

fn derives_default(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().filter(|a| a.path().is_ident("derive")).any(|a| {
        a.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
            .is_ok_and(|paths| paths.iter().any(|p| p.segments.last().is_some_and(|s| s.ident == "Default")))
    })
}

/// A struct or enum with `#[derive(Default)]`: the type's unit, and one
/// for the method `default` the derive writes, not counted as public, as
/// the trait impl is not a function in Rust.
fn with_default(
    cx: &mut Cx,
    item: &SynItem,
    all_causes: bool,
    kind: UnitKind,
    name: String,
    public: bool,
    file: usize,
) -> Vec<Unit> {
    let ident = item::item_ident(item).expect("a struct or an enum");
    let at = LineCol::of(ident.span());
    let run = |cx: &mut Cx| item::lower_item(cx, item.clone());
    let (lowered, causes) = if all_causes { cx.recovering(run) } else { (run(cx), Vec::new()) };
    let (ty, default) = match lowered {
        Ok(mut items) if items.len() == 2 => {
            let (default, default_at) = items.pop().expect("two items");
            (Ok(items.remove(0).0), Some((Ok(default), default_at)))
        }
        Ok(mut items) if items.len() == 1 => (Ok(items.remove(0).0), None),
        Ok(_) => (Err(ParseError::new(Reason::UnsupportedItem, "expected one item")), None),
        Err(e) => (Err(e.clone()), Some((Err(e), at))),
    };
    let mut units = vec![Unit { file, at, kind, name: name.clone(), public, lowered: ty, causes: causes.clone() }];
    if let Some((lowered, at)) = default {
        units.push(Unit {
            file,
            at,
            kind: UnitKind::Method { owner: name },
            name: "default".to_string(),
            public: false,
            lowered,
            causes,
        });
    }
    units
}

fn is_ord_impl(imp: &syn::ItemImpl) -> bool {
    imp.trait_.as_ref().and_then(|(_, p, _)| p.segments.last()).is_some_and(|s| s.ident == "Ord")
}

fn self_name(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(p) => path_text(&p.path),
        other => other.span().source_text().unwrap_or_default(),
    }
}
