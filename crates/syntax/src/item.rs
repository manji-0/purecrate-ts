use std::collections::{HashMap, HashSet};

use purecrate_ir::{
    Alias, Enum, Field, Fn, Item, Name, Param, Reason, Struct, Variant, VariantFields, Vis, NEWTYPE_FIELD,
};
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::{Fields as SynFields, Item as SynItem, Visibility};

use crate::expr::lower_block;
use crate::ty::lower_type;

/// 1-based line and column in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    pub col: usize,
}

impl LineCol {
    pub fn of(span: proc_macro2::Span) -> Self {
        let start = span.start();
        Self {
            line: start.line,
            col: start.column + 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub reason: Reason,
    /// What the reason is about when that varies, e.g. the macro or method
    /// name, for tallying.
    pub detail: Option<String>,
    pub message: String,
    pub at: Option<LineCol>,
}

impl ParseError {
    pub fn new(reason: Reason, message: impl Into<String>) -> Self {
        Self {
            reason,
            detail: None,
            message: message.into(),
            at: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Keep the innermost location: only fills `at` when still unset.
    pub fn or_at(mut self, span: proc_macro2::Span) -> Self {
        if self.at.is_none() {
            self.at = Some(LineCol::of(span));
        }
        self
    }
}

/// Source text of `node` for diagnostics: first line, at most 60 chars.
pub fn snippet(node: &impl Spanned) -> String {
    let text = node.span().source_text().unwrap_or_default();
    let first = text.lines().next().unwrap_or("");
    let short: String = first.chars().take(60).collect();
    if short.len() < text.len() {
        format!("`{short}…`")
    } else {
        format!("`{short}`")
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.at {
            Some(LineCol { line, col }) => write!(f, "{line}:{col}: [{}] {}", self.reason, self.message),
            None => write!(f, "[{}] {}", self.reason, self.message),
        }
    }
}

impl std::error::Error for ParseError {}

pub struct Cx {
    enums: HashSet<String>,
    structs: HashSet<String>,
    variant_owner: HashMap<String, String>,
    variants: HashSet<(String, String)>,
}

impl Cx {
    pub fn scan(file: &syn::File) -> Self {
        Self::scan_items(file.items.iter().filter(|i| !is_test_only(i)))
    }

    pub fn scan_items<'a>(items: impl IntoIterator<Item = &'a SynItem>) -> Self {
        let mut enums = HashSet::new();
        let mut structs = HashSet::new();
        let mut variant_owner = HashMap::new();
        let mut variants = HashSet::new();
        for item in items {
            match item {
                SynItem::Enum(e) => {
                    enums.insert(e.ident.to_string());
                    for v in &e.variants {
                        variant_owner.insert(v.ident.to_string(), e.ident.to_string());
                        variants.insert((e.ident.to_string(), v.ident.to_string()));
                    }
                }
                SynItem::Struct(s) => {
                    structs.insert(s.ident.to_string());
                }
                _ => {}
            }
        }
        Self {
            enums,
            structs,
            variant_owner,
            variants,
        }
    }

    pub fn is_variant(&self, ty: &str, variant: &str) -> bool {
        self.variants.contains(&(ty.to_string(), variant.to_string()))
    }

    pub fn is_enum(&self, name: &str) -> bool {
        self.enums.contains(name)
    }

    pub fn is_struct(&self, name: &str) -> bool {
        self.structs.contains(name)
    }

    pub fn enum_for_variant(&self, variant: &str) -> Option<String> {
        self.variant_owner.get(variant).cloned()
    }
}

/// Attributes that would make the emitted TS disagree with the Rust build if
/// they were dropped. Everything else (`derive`, `doc`, lints) is inert here.
fn reject_attrs(attrs: &[syn::Attribute]) -> Result<(), ParseError> {
    for attr in attrs {
        let path = attr.path();
        let (reason, message) = if path.is_ident("serde") {
            (
                Reason::SerdeAttr,
                "`#[serde(...)]` changes the JSON shape, which v0 does not model yet; \
                 remove it or keep this type out of the crate",
            )
        } else if path.is_ident("cfg") || path.is_ident("cfg_attr") {
            (
                Reason::Cfg,
                "conditional compilation is not in v0: the generated TS cannot follow `#[cfg]`",
            )
        } else {
            continue;
        };
        return Err(ParseError::new(reason, message).or_at(attr.span()));
    }
    Ok(())
}

fn reject_field_attrs(fields: &SynFields) -> Result<(), ParseError> {
    fields.iter().try_for_each(|f| reject_attrs(&f.attrs))
}

fn item_attrs(item: &SynItem) -> &[syn::Attribute] {
    match item {
        SynItem::Enum(e) => &e.attrs,
        SynItem::Struct(s) => &s.attrs,
        SynItem::Fn(f) => &f.attrs,
        SynItem::Type(t) => &t.attrs,
        SynItem::Impl(i) => &i.attrs,
        SynItem::Use(u) => &u.attrs,
        SynItem::Mod(m) => &m.attrs,
        _ => &[],
    }
}

/// `#[cfg(test)]` items are absent from the build the TS mirrors.
pub fn is_test_only(item: &SynItem) -> bool {
    item_attrs(item).iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<syn::Ident>()
                .is_ok_and(|id| id == "test")
    })
}

/// Each lowered item paired with the location of its name.
pub fn lower_item(cx: &mut Cx, item: SynItem) -> Result<Vec<(Item, LineCol)>, ParseError> {
    let span = item.span();
    let names: Vec<LineCol> = match &item {
        SynItem::Enum(e) => vec![LineCol::of(e.ident.span())],
        SynItem::Struct(s) => vec![LineCol::of(s.ident.span())],
        SynItem::Fn(f) => vec![LineCol::of(f.sig.ident.span())],
        SynItem::Type(t) => vec![LineCol::of(t.ident.span())],
        SynItem::Impl(imp) => imp
            .items
            .iter()
            .map(|i| match i {
                syn::ImplItem::Fn(f) => LineCol::of(f.sig.ident.span()),
                other => LineCol::of(other.span()),
            })
            .collect(),
        _ => Vec::new(),
    };
    reject_attrs(item_attrs(&item))?;
    let items = lower_item_node(cx, item).map_err(|e| e.or_at(span))?;
    Ok(items.into_iter().zip(names).collect())
}

fn lower_item_node(cx: &mut Cx, item: SynItem) -> Result<Vec<Item>, ParseError> {
    match item {
        SynItem::Enum(e) => Ok(vec![Item::Enum(lower_enum(&e)?)]),
        SynItem::Struct(s) => Ok(vec![Item::Struct(lower_struct(&s)?)]),
        SynItem::Fn(f) => Ok(vec![Item::Fn(lower_fn(cx, None, &f.sig, &f.vis, &f.block)?)]),
        SynItem::Type(t) => {
            if !has_type_generics(&t.generics) {
                Ok(vec![Item::Alias(Alias {
                    vis: lower_vis(&t.vis),
                    name: Name::new(t.ident.to_string()),
                    ty: lower_type(&t.ty)?,
                })])
            } else {
                Err(ParseError::new(Reason::Generics, "generic type aliases are not in v0"))
            }
        }
        SynItem::Impl(imp) => lower_impl(cx, imp),
        SynItem::Use(_) => Ok(vec![]),
        SynItem::Mod(_) => Err(ParseError::new(
            Reason::Module,
            "modules are flattened at the call site; a single file is required in v0 parse",
        )),
        other => Err(ParseError::new(Reason::UnsupportedItem, format!("unsupported item {}", snippet(&other)))
            .detail(item_kind(&other))),
    }
}

fn lower_enum(e: &syn::ItemEnum) -> Result<Enum, ParseError> {
    if has_type_generics(&e.generics) {
        return Err(ParseError::new(Reason::Generics, "generic enums are not in v0"));
    }
    if e.variants.is_empty() {
        return Err(ParseError::new(
            Reason::UnsupportedItem,
            format!("enum `{}` has no variants; an uninhabited type has no TS union or wire form", e.ident),
        )
        .detail("empty-enum"));
    }
    let mut variants = Vec::new();
    for v in &e.variants {
        reject_attrs(&v.attrs)?;
        reject_field_attrs(&v.fields)?;
        variants.push(Variant {
            name: Name::new(v.ident.to_string()),
            fields: lower_fields(&v.fields)?,
        });
    }
    Ok(Enum {
        vis: lower_vis(&e.vis),
        name: Name::new(e.ident.to_string()),
        variants,
    })
}

fn lower_struct(s: &syn::ItemStruct) -> Result<Struct, ParseError> {
    if has_type_generics(&s.generics) {
        return Err(ParseError::new(Reason::Generics, "generic structs are not in v0"));
    }
    reject_field_attrs(&s.fields)?;
    let closed = s.fields.iter().any(|f| lower_vis(&f.vis) != Vis::Pub);
    let fields = match &s.fields {
        SynFields::Named(n) => n
            .named
            .iter()
            .map(|f| {
                Ok(Field {
                    name: Name::new(f.ident.as_ref().unwrap().to_string()),
                    ty: lower_type(&f.ty)?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        SynFields::Unit => Vec::new(),
        SynFields::Unnamed(u) if u.unnamed.len() == 1 => vec![Field {
            name: Name::new(NEWTYPE_FIELD),
            ty: lower_type(&u.unnamed[0].ty)?,
        }],
        SynFields::Unnamed(_) => {
            return Err(ParseError::new(
                Reason::TupleStruct,
                "tuple structs with more than one field are not in v0; name the fields",
            ))
        }
    };
    Ok(Struct {
        vis: lower_vis(&s.vis),
        name: Name::new(s.ident.to_string()),
        fields,
        closed,
    })
}

fn lower_fields(fields: &SynFields) -> Result<VariantFields, ParseError> {
    match fields {
        SynFields::Unit => Ok(VariantFields::Unit),
        SynFields::Unnamed(u) => {
            let tys = u
                .unnamed
                .iter()
                .map(|f| lower_type(&f.ty))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VariantFields::Tuple(tys))
        }
        SynFields::Named(n) => {
            let fs = n
                .named
                .iter()
                .map(|f| {
                    Ok(Field {
                        name: Name::new(f.ident.as_ref().unwrap().to_string()),
                        ty: lower_type(&f.ty)?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VariantFields::Struct(fs))
        }
    }
}

/// Free function (`owner: None`) or method folded onto `owner`'s companion.
fn lower_fn(
    cx: &Cx,
    owner: Option<Name>,
    sig: &syn::Signature,
    vis: &Visibility,
    block: &syn::Block,
) -> Result<Fn, ParseError> {
    let reject = |what: &str, span: proc_macro2::Span| {
        Err(ParseError::new(Reason::FnQualifier, format!("{what} is not allowed in v0"))
            .detail(what)
            .or_at(span))
    };
    if let Some(t) = &sig.asyncness {
        return reject("async", t.span);
    }
    if let Some(t) = &sig.unsafety {
        return reject("unsafe", t.span);
    }
    if let Some(abi) = &sig.abi {
        return reject("extern abi", abi.span());
    }
    if has_type_generics(&sig.generics) {
        return Err(ParseError::new(Reason::Generics, "generic functions are not in v0").or_at(sig.generics.span()));
    }
    let mut params = Vec::new();
    for input in &sig.inputs {
        params.push(lower_param(owner.as_ref(), input).map_err(|e| e.or_at(input.span()))?);
    }
    let ret = match &sig.output {
        syn::ReturnType::Default => purecrate_ir::Ty::Prim(purecrate_ir::Prim::Unit),
        syn::ReturnType::Type(_, t) => lower_type(t)?,
    };
    Ok(Fn {
        vis: lower_vis(vis),
        name: Name::new(sig.ident.to_string()),
        owner,
        params,
        ret,
        body: lower_block(cx, block)?,
    })
}

fn lower_param(owner: Option<&Name>, input: &syn::FnArg) -> Result<Param, ParseError> {
    match input {
        syn::FnArg::Receiver(r) => match owner {
            Some(_) if r.reference.is_some() && r.mutability.is_some() => Err(ParseError::new(
                Reason::RefReceiver,
                "`&mut self` is not in v0: take `self` and return the new value",
            )),
            Some(owner) => Ok(Param {
                name: Name::new("self"),
                ty: purecrate_ir::Ty::Named(owner.clone()),
            }),
            None => Err(ParseError::new(Reason::UnsupportedItem, "`self` outside an impl block")),
        },
        syn::FnArg::Typed(p) => match &*p.pat {
            syn::Pat::Ident(id) if id.mutability.is_some() => Err(ParseError::new(Reason::MutParam, format!(
                "`mut` parameters are not in v0; write `let mut {0} = {0};` in the body",
                id.ident
            ))),
            syn::Pat::Ident(id) => Ok(Param {
                name: Name::new(id.ident.to_string()),
                ty: lower_type(&p.ty)?,
            }),
            _ => Err(ParseError::new(Reason::ParamPattern, "only named parameters")),
        },
    }
}

fn lower_impl(cx: &mut Cx, imp: syn::ItemImpl) -> Result<Vec<Item>, ParseError> {
    if imp.trait_.is_some() {
        return Err(ParseError::new(Reason::TraitImpl, "trait impls are not in v0"));
    }
    if has_type_generics(&imp.generics) {
        return Err(ParseError::new(Reason::Generics, "generic impls are not in v0"));
    }
    let owner_ident = match &*imp.self_ty {
        syn::Type::Path(p) if p.path.segments.len() == 1 => p.path.segments[0].ident.clone(),
        _ => {
            return Err(ParseError::new(Reason::ImplShape, "impl type must be a simple name").or_at(imp.self_ty.span()))
        }
    };
    let owner = Name::new(owner_ident.to_string());
    let mut out = Vec::new();
    for item in &imp.items {
        let lowered = match item {
            syn::ImplItem::Fn(f) => reject_attrs(&f.attrs).and_then(|()| {
                let mut f = f.clone();
                SelfIsOwner(&owner_ident).visit_impl_item_fn_mut(&mut f);
                lower_fn(cx, Some(owner.clone()), &f.sig, &f.vis, &f.block)
            }),
            _ => Err(ParseError::new(Reason::ImplShape, "only methods in impl blocks in v0")),
        };
        out.push(Item::Fn(lowered.map_err(|e| e.or_at(item.span()))?));
    }
    Ok(out)
}

/// Rewrites `Self` to the impl's type name in types, paths and patterns.
/// Nested items keep their own `Self`.
struct SelfIsOwner<'a>(&'a syn::Ident);

impl VisitMut for SelfIsOwner<'_> {
    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        if let Some(first) = path.segments.first_mut() {
            if first.ident == "Self" {
                first.ident = syn::Ident::new(&self.0.to_string(), first.ident.span());
            }
        }
        visit_mut::visit_path_mut(self, path);
    }

    fn visit_item_mut(&mut self, _: &mut SynItem) {}
}

/// Lifetimes only relate references, which lower to plain values.
fn has_type_generics(generics: &syn::Generics) -> bool {
    generics.params.iter().any(|p| !matches!(p, syn::GenericParam::Lifetime(_)))
}

fn lower_vis(vis: &Visibility) -> Vis {
    match vis {
        Visibility::Public(_) => Vis::Pub,
        _ => Vis::Internal,
    }
}

fn item_kind(item: &SynItem) -> &'static str {
    match item {
        SynItem::Const(_) => "const",
        SynItem::Static(_) => "static",
        SynItem::Trait(_) | SynItem::TraitAlias(_) => "trait",
        SynItem::Union(_) => "union",
        SynItem::Macro(_) => "macro",
        SynItem::ExternCrate(_) | SynItem::ForeignMod(_) => "extern",
        _ => "other",
    }
}
