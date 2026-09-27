//! Lowering for measurement: every item of a module tree is lowered on its
//! own, so one unsupported item does not hide the rest.

use purecrate_ir::{Item, Reason};
use syn::spanned::Spanned;
use syn::Item as SynItem;

use crate::item::{self, is_test_only, Cx, LineCol, ParseError};

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
}

/// Out-of-line `mod x;` declarations, as paths relative to the declaring
/// file's module directory: `mod a { mod b; }` gives `["a", "b"]`.
pub fn module_decls(source: &str) -> Result<Vec<Vec<String>>, ParseError> {
    let file = parse(source)?;
    let mut out = Vec::new();
    collect_mods(&file.items, &mut Vec::new(), &mut out);
    Ok(out)
}

fn collect_mods(items: &[SynItem], prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    for item in items.iter().filter(|i| !is_test_only(i)) {
        if let SynItem::Mod(m) = item {
            prefix.push(m.ident.to_string());
            match &m.content {
                Some((_, inner)) => collect_mods(inner, prefix, out),
                None => out.push(prefix.clone()),
            }
            prefix.pop();
        }
    }
}

/// A file `syn` cannot parse fails the whole survey; everything else is
/// reported per unit.
pub fn survey_files(sources: &[&str]) -> Result<Vec<Unit>, (usize, ParseError)> {
    let files = sources
        .iter()
        .enumerate()
        .map(|(i, s)| parse(s).map_err(|e| (i, e)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut all = Vec::new();
    for f in &files {
        flatten(&f.items, &mut all);
    }
    let mut cx = Cx::scan_items(all.iter().copied());
    let mut units = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let mut items = Vec::new();
        flatten(&f.items, &mut items);
        for item in items {
            units.extend(units_of(&mut cx, i, item));
        }
    }
    Ok(units)
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

fn units_of(cx: &mut Cx, file: usize, item: &SynItem) -> Vec<Unit> {
    let unit = |kind, name: String, public, at, lowered| Unit {
        file,
        at,
        kind,
        name,
        public,
        lowered,
    };
    let one = |cx: &mut Cx, kind, name: String, public, at| {
        let lowered = item::lower_item(cx, item.clone()).and_then(|mut items| match items.len() {
            1 => Ok(items.remove(0).0),
            _ => Err(ParseError::new(Reason::UnsupportedItem, "expected one item")),
        });
        vec![unit(kind, name, public, at, lowered)]
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
                let lowered = item::lower_item(cx, SynItem::Impl(single)).and_then(|mut items| match items.len() {
                    1 => Ok(items.remove(0).0),
                    _ => Err(ParseError::new(Reason::UnsupportedItem, "expected one method")),
                });
                unit(UnitKind::Method { owner }, name, is_pub, at, lowered)
            })
            .collect(),
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
            let lowered = item::lower_item(cx, other.clone())
                .and_then(|_| Err(ParseError::new(Reason::UnsupportedItem, format!("{what} is not in v0"))));
            vec![unit(UnitKind::Other { what }, name, false, LineCol::of(other.span()), lowered)]
        }
    }
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
