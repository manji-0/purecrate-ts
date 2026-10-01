use std::collections::{HashMap, HashSet};

use purecrate_ir::{
    Alias, Const, Enum, Field, Fn, IntTy, Item, Name, Param, Reason, Serde, Struct, Variant, VariantFields, Vis, NEWTYPE_FIELD,
};
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::{Fields as SynFields, Item as SynItem, Visibility};

use crate::expr::{lower_block, lower_expr};
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
    /// Names made up while lowering (`$g1`, ... for guarded scrutinees).
    fresh: std::cell::Cell<u32>,
    /// When set (`survey --all-causes`), an expression or block that cannot
    /// be lowered is recorded here and stands in as `unreachable`, so the
    /// rest of the item is still lowered and every cause is found.
    recovered: std::cell::RefCell<Option<Vec<ParseError>>>,
    /// `const` items of the blocks being lowered, innermost last: a name
    /// here compares in a pattern, which the IR cannot express.
    local_consts: std::cell::RefCell<Vec<String>>,
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
            fresh: std::cell::Cell::new(0),
            recovered: std::cell::RefCell::new(None),
            local_consts: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Makes `Ordering` name std's enum (`std_ordering`). Its variants are
    /// found only through `Ordering::`: a bare `Less` stays the crate's.
    pub fn add_std_ordering(&mut self) {
        let e = purecrate_ir::Enum::std_ordering(Vis::Pub);
        for v in &e.variants {
            self.variants.insert((e.name.as_str().to_string(), v.name.as_str().to_string()));
        }
        self.enums.insert(e.name.as_str().to_string());
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

    /// Runs `f` recording the expressions it cannot lower instead of
    /// failing on the first; returns what `f` returned and those errors.
    pub fn recovering<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> (T, Vec<ParseError>) {
        *self.recovered.borrow_mut() = Some(Vec::new());
        let out = f(self);
        let errors = self.recovered.borrow_mut().take().unwrap_or_default();
        (out, errors)
    }

    /// In recovery, records `e` and returns `true`: the caller stands in
    /// a placeholder. Otherwise `false`, and the caller fails with `e`.
    pub fn recover(&self, e: &ParseError) -> bool {
        match self.recovered.borrow_mut().as_mut() {
            Some(errors) => {
                errors.push(e.clone());
                true
            }
            None => false,
        }
    }

    /// Runs `f` with `names` in scope as a block's `const` items.
    pub fn with_local_consts<T>(&self, names: Vec<String>, f: impl FnOnce() -> T) -> T {
        let n = names.len();
        self.local_consts.borrow_mut().extend(names);
        let out = f();
        let mut scope = self.local_consts.borrow_mut();
        let keep = scope.len() - n;
        scope.truncate(keep);
        out
    }

    pub fn is_local_const(&self, name: &str) -> bool {
        self.local_consts.borrow().iter().any(|c| c == name)
    }

    /// A name no Rust identifier can have (`$` is not in one).
    pub fn fresh(&self, prefix: &str) -> Name {
        self.fresh.set(self.fresh.get() + 1);
        Name::new(format!("${prefix}{}", self.fresh.get()))
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
                "`#[serde(...)]` changes the JSON shape, which v0 does not model; \
                 the one exception is `#[serde(try_from = \"T\")]` on a struct",
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
        SynItem::Const(c) => &c.attrs,
        _ => &[],
    }
}

/// The text of `///` and `/** */` comments (`#[doc = "..."]`), one line
/// each, less the one space after `///`; `None` without any.
fn doc(attrs: &[syn::Attribute]) -> Option<String> {
    let mut lines = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        let syn::Meta::NameValue(nv) = &attr.meta else { continue };
        let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) = &nv.value else { continue };
        for line in text.value().split('\n') {
            lines.push(line.strip_prefix(' ').unwrap_or(line).trim_end().to_string());
        }
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    let first = lines.iter().position(|l| !l.is_empty())?;
    Some(lines[first..].join("\n"))
}

trait WithDoc {
    fn with_doc(self, attrs: &[syn::Attribute]) -> Self;
}

impl WithDoc for Fn {
    fn with_doc(self, attrs: &[syn::Attribute]) -> Self {
        Fn { doc: doc(attrs), ..self }
    }
}

/// The serde derives in `#[derive(...)]`, by the last path segment, so
/// `serde::Serialize` counts as `Serialize`.
fn serde_derives(attrs: &[syn::Attribute]) -> Serde {
    let mut out = Serde::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("derive")) {
        let Ok(paths) = attr.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        ) else {
            continue;
        };
        for path in paths {
            match path.segments.last().map(|s| s.ident.to_string()).as_deref() {
                Some("Serialize") => out.ser = true,
                Some("Deserialize") => out.de = true,
                _ => {}
            }
        }
    }
    out
}

/// `#[serde(try_from = "T")]`, the one serde attribute v0 models.
fn is_try_from_attr(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("serde") && wire_from_attr(attr).is_ok()
}

/// The type named by `#[serde(try_from = "T")]`, with nothing else in it.
fn wire_from_attr(attr: &syn::Attribute) -> Result<syn::Type, ParseError> {
    let not_try_from = || {
        ParseError::new(
            Reason::SerdeAttr,
            "the only `#[serde(...)]` in v0 is `#[serde(try_from = \"T\")]` on a struct; other attributes change the JSON shape",
        )
        .or_at(attr.span())
    };
    let nv: syn::MetaNameValue = attr.parse_args().map_err(|_| not_try_from())?;
    if !nv.path.is_ident("try_from") {
        return Err(not_try_from());
    }
    match &nv.value {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => s.parse::<syn::Type>().map_err(|_| not_try_from()),
        _ => Err(not_try_from()),
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
        SynItem::Const(c) => vec![LineCol::of(c.ident.span())],
        // One per lowered method; `type Error` of a `TryFrom` impl is not one.
        SynItem::Impl(imp) => imp
            .items
            .iter()
            .filter_map(|i| match i {
                syn::ImplItem::Fn(f) => Some(LineCol::of(f.sig.ident.span())),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let attrs: Vec<syn::Attribute> = item_attrs(&item)
        .iter()
        .filter(|a| !(matches!(item, SynItem::Struct(_)) && is_try_from_attr(a)))
        .cloned()
        .collect();
    reject_attrs(&attrs)?;
    let items = lower_item_node(cx, item).map_err(|e| e.or_at(span))?;
    Ok(items.into_iter().zip(names).collect())
}

fn lower_item_node(cx: &mut Cx, item: SynItem) -> Result<Vec<Item>, ParseError> {
    match item {
        SynItem::Enum(e) => Ok(vec![Item::Enum(lower_enum(cx, &e)?)]),
        SynItem::Struct(s) => Ok(vec![Item::Struct(lower_struct(&s)?)]),
        SynItem::Fn(f) => Ok(vec![Item::Fn(lower_fn(cx, None, &f.sig, &f.vis, &f.block)?.with_doc(&f.attrs))]),
        SynItem::Type(t) => {
            if !has_type_generics(&t.generics) {
                Ok(vec![Item::Alias(Alias {
                    vis: lower_vis(&t.vis),
                    name: Name::new(t.ident.to_string()),
                    ty: lower_type(&t.ty)?,
                    doc: doc(&t.attrs),
                })])
            } else {
                Err(ParseError::new(Reason::Generics, "generic type aliases are not in v0"))
            }
        }
        SynItem::Impl(imp) => lower_impl(cx, imp),
        SynItem::Const(c) => Ok(vec![Item::Const(lower_const(cx, &c)?)]),
        SynItem::Use(_) => Ok(vec![]),
        SynItem::Mod(_) => Err(ParseError::new(
            Reason::Module,
            "modules are flattened at the call site; a single file is required in v0 parse",
        )),
        other => Err(ParseError::new(Reason::UnsupportedItem, format!("unsupported item {}", snippet(&other)))
            .detail(item_kind(&other))),
    }
}

/// `const NAME: T = expr;`, folded by `check::accept`. `const _` and
/// generic consts are not in v0.
fn lower_const(cx: &Cx, c: &syn::ItemConst) -> Result<Const, ParseError> {
    if has_type_generics(&c.generics) || !c.generics.params.is_empty() {
        return Err(ParseError::new(Reason::Generics, "generic consts are not in v0"));
    }
    if c.ident == "_" {
        return Err(ParseError::new(Reason::UnsupportedItem, "`const _` is not in v0").detail("const"));
    }
    Ok(Const {
        vis: lower_vis(&c.vis),
        name: Name::new(c.ident.to_string()),
        ty: lower_type(&c.ty)?,
        value: lower_expr(cx, &c.expr)?,
        doc: doc(&c.attrs),
    })
}

/// `#[repr(u8)]` and the other integer reprs; any other `repr` changes
/// nothing TS can follow and is refused.
fn enum_repr(attrs: &[syn::Attribute]) -> Result<Option<IntTy>, ParseError> {
    let mut repr = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("repr")) {
        let id: syn::Ident = attr.parse_args().map_err(|_| {
            ParseError::new(Reason::UnsupportedItem, "only `#[repr(<integer>)]` is in v0 on an enum")
                .detail("repr")
                .or_at(attr.span())
        })?;
        let ty = IntTy::ALL.into_iter().find(|t| id == t.as_str()).ok_or_else(|| {
            ParseError::new(Reason::UnsupportedItem, format!("`#[repr({id})]` is not in v0; use an integer type"))
                .detail("repr")
                .or_at(attr.span())
        })?;
        repr = Some(ty);
    }
    Ok(repr)
}

fn lower_enum(cx: &Cx, e: &syn::ItemEnum) -> Result<Enum, ParseError> {
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
    let fieldless = e.variants.iter().all(|v| matches!(v.fields, SynFields::Unit));
    let mut variants = Vec::new();
    for v in &e.variants {
        reject_attrs(&v.attrs)?;
        reject_field_attrs(&v.fields)?;
        let discriminant = match &v.discriminant {
            Some((_, expr)) if fieldless => Some(lower_expr(cx, expr)?),
            Some((_, expr)) => {
                return Err(ParseError::new(
                    Reason::UnsupportedItem,
                    "discriminants are in v0 only on an enum whose variants have no fields",
                )
                .detail("discriminant")
                .or_at(expr.span()))
            }
            None => None,
        };
        variants.push(Variant {
            name: Name::new(v.ident.to_string()),
            fields: lower_fields(&v.fields)?,
            discriminant,
            doc: doc(&v.attrs),
        });
    }
    Ok(Enum {
        vis: lower_vis(&e.vis),
        name: Name::new(e.ident.to_string()),
        variants,
        repr: enum_repr(&e.attrs)?,
        std: false,
        serde: serde_derives(&e.attrs),
        doc: doc(&e.attrs),
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
                    doc: doc(&f.attrs),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        // serde writes `struct S;` as `null` and `struct S {}` as `{}`; the IR
        // keeps only the fields, so one spelling is taken (design/04 §3).
        SynFields::Unit => {
            return Err(ParseError::new(
                Reason::UnitStruct,
                "unit structs are not in v0; write `struct S {}` or an enum",
            ))
        }
        SynFields::Unnamed(u) if u.unnamed.len() == 1 => vec![Field {
            name: Name::new(NEWTYPE_FIELD),
            ty: lower_type(&u.unnamed[0].ty)?,
            doc: None,
        }],
        SynFields::Unnamed(_) => {
            return Err(ParseError::new(
                Reason::TupleStruct,
                "tuple structs with more than one field are not in v0; name the fields",
            ))
        }
    };
    let mut wire_from = None;
    for attr in s.attrs.iter().filter(|a| a.path().is_ident("serde")) {
        if wire_from.is_some() {
            return Err(ParseError::new(Reason::SerdeAttr, "`#[serde(try_from)]` is given twice").or_at(attr.span()));
        }
        wire_from = Some(lower_type(&wire_from_attr(attr)?)?);
    }
    Ok(Struct {
        vis: lower_vis(&s.vis),
        name: Name::new(s.ident.to_string()),
        fields,
        closed,
        wire_from,
        serde: serde_derives(&s.attrs),
        doc: doc(&s.attrs),
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
                        doc: doc(&f.attrs),
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
        doc: None,
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

/// The trait impls v0 takes: `Display` and `Error` are skipped (a server
/// needs them, for serde's `try_from` among others, and nothing translated
/// can call them), and `TryFrom<T>` becomes the method `try_from`.
enum TraitImpl {
    Skipped,
    TryFrom,
}

fn trait_impl(path: &syn::Path) -> Option<TraitImpl> {
    let idents: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    let idents: Vec<&str> = idents.iter().map(String::as_str).collect();
    match idents.as_slice() {
        ["Display"] | ["fmt", "Display"] | ["std" | "core", "fmt", "Display"] => Some(TraitImpl::Skipped),
        ["Error"] | ["error", "Error"] | ["std" | "core", "error", "Error"] => Some(TraitImpl::Skipped),
        ["TryFrom"] | ["convert", "TryFrom"] | ["std" | "core", "convert", "TryFrom"] => {
            match &path.segments.last()?.arguments {
                syn::PathArguments::AngleBracketed(a) => match a.args.first() {
                    Some(syn::GenericArgument::Type(_)) if a.args.len() == 1 => Some(TraitImpl::TryFrom),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

/// `impl TryFrom<T> for X { type Error = E; fn try_from(value: T) -> Result<Self, Self::Error> }`
/// as `X::try_from`, public like the trait, with `Self::Error` read as `E`.
fn lower_try_from(cx: &mut Cx, imp: &syn::ItemImpl, owner_ident: &syn::Ident) -> Result<Vec<Item>, ParseError> {
    let error = imp
        .items
        .iter()
        .find_map(|i| match i {
            syn::ImplItem::Type(t) if t.ident == "Error" => Some(t.ty.clone()),
            _ => None,
        })
        .ok_or_else(|| ParseError::new(Reason::ImplShape, "`impl TryFrom` needs `type Error = ..`"))?;
    let owner = Name::new(owner_ident.to_string());
    let mut out = Vec::new();
    for item in &imp.items {
        match item {
            syn::ImplItem::Type(t) if t.ident == "Error" => {}
            syn::ImplItem::Fn(f) if f.sig.ident == "try_from" => {
                reject_attrs(&f.attrs)?;
                let mut f = f.clone();
                SelfError(&error).visit_impl_item_fn_mut(&mut f);
                SelfIsOwner(owner_ident).visit_impl_item_fn_mut(&mut f);
                let public = Visibility::Public(Default::default());
                let lowered = lower_fn(cx, Some(owner.clone()), &f.sig, &public, &f.block).map(|l| l.with_doc(&f.attrs))
                    .map_err(|e| e.or_at(item.span()))?;
                out.push(Item::Fn(lowered));
            }
            other => {
                return Err(ParseError::new(Reason::ImplShape, "`impl TryFrom` has only `type Error` and `fn try_from`")
                    .or_at(other.span()))
            }
        }
    }
    Ok(out)
}

/// Rewrites the type `Self::Error` to the impl's `type Error`.
struct SelfError<'a>(&'a syn::Type);

impl VisitMut for SelfError<'_> {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let syn::Type::Path(p) = ty {
            let idents: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
            if p.qself.is_none() && idents == ["Self", "Error"] {
                *ty = self.0.clone();
                return;
            }
        }
        visit_mut::visit_type_mut(self, ty);
    }

    fn visit_item_mut(&mut self, _: &mut SynItem) {}
}

fn lower_impl(cx: &mut Cx, imp: syn::ItemImpl) -> Result<Vec<Item>, ParseError> {
    let kind = match &imp.trait_ {
        None => None,
        Some((_, path, _)) => Some(trait_impl(path).ok_or_else(|| {
            ParseError::new(Reason::TraitImpl, "trait impls are not in v0, except `Display`, `Error` (skipped) and `TryFrom<T>`")
        })?),
    };
    if matches!(kind, Some(TraitImpl::Skipped)) {
        return Ok(Vec::new());
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
    if matches!(kind, Some(TraitImpl::TryFrom)) {
        return lower_try_from(cx, &imp, &owner_ident);
    }
    let owner = Name::new(owner_ident.to_string());
    let mut out = Vec::new();
    for item in &imp.items {
        let lowered = match item {
            syn::ImplItem::Fn(f) => reject_attrs(&f.attrs).and_then(|()| {
                let mut f = f.clone();
                SelfIsOwner(&owner_ident).visit_impl_item_fn_mut(&mut f);
                lower_fn(cx, Some(owner.clone()), &f.sig, &f.vis, &f.block).map(|l| l.with_doc(&f.attrs))
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
