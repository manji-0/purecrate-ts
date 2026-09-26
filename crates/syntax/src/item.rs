use std::collections::{HashMap, HashSet};

use purecrate_ir::{
    Alias, Enum, Field, Fn, Item, Name, Param, Struct, Variant, VariantFields, Vis,
};
use syn::spanned::Spanned;
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
    pub message: String,
    pub at: Option<LineCol>,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            at: None,
        }
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
            Some(LineCol { line, col }) => write!(f, "{line}:{col}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ParseError {}

pub struct Cx {
    enums: HashSet<String>,
    structs: HashSet<String>,
    variant_owner: HashMap<String, String>,
}

impl Cx {
    pub fn scan(file: &syn::File) -> Self {
        let mut enums = HashSet::new();
        let mut structs = HashSet::new();
        let mut variant_owner = HashMap::new();
        for item in &file.items {
            match item {
                SynItem::Enum(e) => {
                    enums.insert(e.ident.to_string());
                    for v in &e.variants {
                        variant_owner.insert(v.ident.to_string(), e.ident.to_string());
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
        }
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
    let items = lower_item_node(cx, item).map_err(|e| e.or_at(span))?;
    Ok(items.into_iter().zip(names).collect())
}

fn lower_item_node(cx: &mut Cx, item: SynItem) -> Result<Vec<Item>, ParseError> {
    match item {
        SynItem::Enum(e) => Ok(vec![Item::Enum(lower_enum(&e)?)]),
        SynItem::Struct(s) => Ok(vec![Item::Struct(lower_struct(&s)?)]),
        SynItem::Fn(f) => Ok(vec![Item::Fn(lower_fn(cx, None, &f.sig, &f.vis, &f.block)?)]),
        SynItem::Type(t) => {
            if t.generics.params.is_empty() {
                Ok(vec![Item::Alias(Alias {
                    vis: lower_vis(&t.vis),
                    name: Name::new(t.ident.to_string()),
                    ty: lower_type(&t.ty)?,
                })])
            } else {
                Err(ParseError::new("generic type aliases are not in v0"))
            }
        }
        SynItem::Impl(imp) => lower_impl(cx, imp),
        SynItem::Use(_) => Ok(vec![]),
        SynItem::Mod(_) => Err(ParseError::new(
            "modules are flattened at the call site; a single file is required in v0 parse",
        )),
        other => Err(ParseError::new(format!("unsupported item {}", snippet(&other)))),
    }
}

fn lower_enum(e: &syn::ItemEnum) -> Result<Enum, ParseError> {
    if !e.generics.params.is_empty() {
        return Err(ParseError::new("generic enums are not in v0"));
    }
    let mut variants = Vec::new();
    for v in &e.variants {
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
    if !s.generics.params.is_empty() {
        return Err(ParseError::new("generic structs are not in v0"));
    }
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
        SynFields::Unnamed(_) => {
            return Err(ParseError::new("tuple structs are not in v0"))
        }
    };
    Ok(Struct {
        vis: lower_vis(&s.vis),
        name: Name::new(s.ident.to_string()),
        fields,
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
        Err(ParseError::new(format!("{what} is not allowed in v0")).or_at(span))
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
    if !sig.generics.params.is_empty() {
        return reject("a generic function", sig.generics.span());
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
            Some(_) if r.reference.is_some() => Err(ParseError::new(
                "reference receivers are not allowed; take self by value",
            )),
            Some(owner) => Ok(Param {
                name: Name::new("self"),
                ty: purecrate_ir::Ty::Named(owner.clone()),
            }),
            None => Err(ParseError::new("`self` outside an impl block")),
        },
        syn::FnArg::Typed(p) => match &*p.pat {
            syn::Pat::Ident(id) => Ok(Param {
                name: Name::new(id.ident.to_string()),
                ty: lower_type(&p.ty)?,
            }),
            _ => Err(ParseError::new("only named parameters")),
        },
    }
}

fn lower_impl(cx: &mut Cx, imp: syn::ItemImpl) -> Result<Vec<Item>, ParseError> {
    if imp.trait_.is_some() {
        return Err(ParseError::new("trait impls are not in v0"));
    }
    if !imp.generics.params.is_empty() {
        return Err(ParseError::new("generic impls are not in v0"));
    }
    let owner = match &*imp.self_ty {
        syn::Type::Path(p) if p.path.segments.len() == 1 => {
            Name::new(p.path.segments[0].ident.to_string())
        }
        _ => {
            return Err(ParseError::new("impl type must be a simple name").or_at(imp.self_ty.span()))
        }
    };
    let mut out = Vec::new();
    for item in &imp.items {
        let lowered = match item {
            syn::ImplItem::Fn(f) => lower_fn(cx, Some(owner.clone()), &f.sig, &f.vis, &f.block),
            _ => Err(ParseError::new("only methods in impl blocks in v0")),
        };
        out.push(Item::Fn(lowered.map_err(|e| e.or_at(item.span()))?));
    }
    Ok(out)
}

fn lower_vis(vis: &Visibility) -> Vis {
    match vis {
        Visibility::Public(_) => Vis::Pub,
        _ => Vis::Internal,
    }
}
