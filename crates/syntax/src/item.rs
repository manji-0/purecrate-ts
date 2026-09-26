use std::collections::{HashMap, HashSet};

use purecrate_ir::{
    Alias, Enum, Field, Fn, Item, Name, Param, Struct, Variant, VariantFields, Vis,
};
use syn::{Fields as SynFields, Item as SynItem, Visibility};

use crate::expr::lower_block;
use crate::ty::lower_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
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

pub fn lower_item(cx: &mut Cx, item: SynItem) -> Result<Vec<Item>, ParseError> {
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
        other => Err(ParseError::new(format!("unsupported item: {other:?}"))),
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

fn lower_fn(
    cx: &Cx,
    owner: Option<Name>,
    sig: &syn::Signature,
    vis: &Visibility,
    block: &syn::Block,
) -> Result<Fn, ParseError> {
    if sig.asyncness.is_some() {
        return Err(ParseError::new("async is not allowed"));
    }
    if sig.unsafety.is_some() {
        return Err(ParseError::new("unsafe is not allowed"));
    }
    if !sig.generics.params.is_empty() {
        return Err(ParseError::new("generic functions are not in v0"));
    }
    if sig.abi.is_some() {
        return Err(ParseError::new("extern abi is not allowed"));
    }
    let mut params = Vec::new();
    for input in &sig.inputs {
        match input {
            syn::FnArg::Receiver(_) => {
                return Err(ParseError::new(
                    "reference receivers are not allowed; take self by value",
                ))
            }
            syn::FnArg::Typed(p) => {
                let name = match &*p.pat {
                    syn::Pat::Ident(id) => Name::new(id.ident.to_string()),
                    _ => return Err(ParseError::new("only named parameters")),
                };
                params.push(Param {
                    name,
                    ty: lower_type(&p.ty)?,
                });
            }
        }
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
        _ => return Err(ParseError::new("impl type must be a simple name")),
    };
    let mut out = Vec::new();
    for item in imp.items {
        match item {
            syn::ImplItem::Fn(f) => {
                let mut params = Vec::new();
                for input in &f.sig.inputs {
                    match input {
                        syn::FnArg::Receiver(r) => {
                            if r.reference.is_some() {
                                return Err(ParseError::new(
                                    "reference receivers are not allowed; take self by value",
                                ));
                            }
                            params.push(Param {
                                name: Name::new("self"),
                                ty: purecrate_ir::Ty::Named(owner.clone()),
                            });
                        }
                        syn::FnArg::Typed(p) => {
                            let name = match &*p.pat {
                                syn::Pat::Ident(id) => Name::new(id.ident.to_string()),
                                _ => return Err(ParseError::new("only named parameters")),
                            };
                            params.push(Param {
                                name,
                                ty: lower_type(&p.ty)?,
                            });
                        }
                    }
                }
                let ret = match &f.sig.output {
                    syn::ReturnType::Default => {
                        purecrate_ir::Ty::Prim(purecrate_ir::Prim::Unit)
                    }
                    syn::ReturnType::Type(_, t) => lower_type(t)?,
                };
                out.push(Item::Fn(Fn {
                    vis: lower_vis(&f.vis),
                    name: Name::new(f.sig.ident.to_string()),
                    owner: Some(owner.clone()),
                    params,
                    ret,
                    body: lower_block(cx, &f.block)?,
                }));
            }
            _ => return Err(ParseError::new("only methods in impl blocks in v0")),
        }
    }
    Ok(out)
}

fn lower_vis(vis: &Visibility) -> Vis {
    match vis {
        Visibility::Public(_) => Vis::Pub,
        _ => Vis::Internal,
    }
}
