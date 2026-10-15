use std::collections::{HashMap, HashSet};

use purecrate_ir::{
    Alias, Const, Enum, Field, Fn, IntTy, Item, Name, Order, Param, Reason, Serde, Struct, Tagging, Variant,
    VariantFields, Vis, NEWTYPE_FIELD,
};
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::{Fields as SynFields, Item as SynItem, Visibility};

use crate::expr::{lower_block, lower_expr};
use crate::serde_attrs::{serde_attrs, wire_name, Place, RenameRule};
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
        Self { line: start.line, col: start.column + 1 }
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
        Self { reason, detail: None, message: message.into(), at: None }
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

/// A source file, or `syn`'s error as `[syntax/invalid]`.
pub fn parse(source: &str) -> Result<syn::File, ParseError> {
    syn::parse_file(source).map_err(|e| ParseError::new(Reason::InvalidSyntax, e.to_string()).or_at(e.span()))
}

/// The names of `path`'s segments, without their arguments.
pub fn path_idents(path: &syn::Path) -> Vec<String> {
    path.segments.iter().map(|s| s.ident.to_string()).collect()
}

/// `path` as written, without its arguments: `std::cmp::max`.
pub fn path_text(path: &syn::Path) -> String {
    path_idents(path).join("::")
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
    /// The consts of inherent `impl` blocks, by type and name: `T::N` reads
    /// one.
    assoc_consts: HashSet<(String, String)>,
    /// The structs and enums that derive `PartialOrd`: a hand-written `impl
    /// Ord` beside one would order `<` and `sort` differently.
    partial_ord: HashSet<String>,
    /// Names made up while lowering (`$g1`, ... for guarded scrutinees).
    fresh: std::cell::Cell<u32>,
    /// When set (`survey --all-causes`), an expression or block that cannot
    /// be lowered is recorded here and stands in as `unreachable`, so the
    /// rest of the item is still lowered and every cause is found.
    recovered: std::cell::RefCell<Option<Vec<ParseError>>>,
    /// `const` items of the blocks being lowered, innermost last: a name
    /// here compares in a pattern, which the IR cannot express.
    local_consts: std::cell::RefCell<Vec<String>>,
    /// Struct patterns of the pattern being lowered: each stands in as a
    /// fresh name, with the patterns of its fields (`StructPats`).
    struct_pats: std::cell::RefCell<Vec<StructPat>>,
    /// The lines of the file whose items are being lowered, to find the
    /// `//` lines above an item.
    source: std::cell::RefCell<Vec<String>>,
    /// The crate's traits, by name (design/01 §7.25).
    pub(crate) traits: HashMap<String, syn::ItemTrait>,
    /// Each `impl Tr for X` of a crate trait, in source order: the trait's
    /// name and the impl.
    pub(crate) trait_impls: Vec<(String, syn::ItemImpl)>,
    /// The methods and consts of inherent `impl` blocks, by type and name.
    pub(crate) inherent: HashSet<(String, String)>,
    /// The companion members a std trait gives a type, by type and TS
    /// name, with what gives each: `cmp` (`Ord`, `PartialOrd`), `default`,
    /// `toString` (`Display`), `tryFrom`, and each `impl From`'s `fromS`.
    pub(crate) std_members: HashMap<(String, String), String>,
}

/// A struct pattern `S { f: p, .. }` lowered as a binding of `name`, its
/// fields tested in the arm's guard and bound in its body.
pub struct StructPat {
    pub name: purecrate_ir::Name,
    pub fields: Vec<(purecrate_ir::Name, purecrate_ir::Pattern)>,
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
        let mut assoc_consts = HashSet::new();
        let mut partial_ord = HashSet::new();
        let mut traits = HashMap::new();
        let mut trait_impls = Vec::new();
        let mut inherent = HashSet::new();
        let mut std_members = HashMap::new();
        let items: Vec<&SynItem> = items.into_iter().collect();
        crate::ty::scan_consts(items.iter().copied());
        for item in items {
            std_members_of(item, &mut std_members);
            match item {
                SynItem::Impl(imp) if imp.trait_.is_none() => {
                    if let syn::Type::Path(p) = &*imp.self_ty {
                        if let Some(ty) = p.path.segments.last() {
                            for member in &imp.items {
                                if let syn::ImplItem::Const(c) = member {
                                    assoc_consts.insert((ty.ident.to_string(), c.ident.to_string()));
                                    inherent.insert((ty.ident.to_string(), c.ident.to_string()));
                                }
                                if let syn::ImplItem::Fn(f) = member {
                                    inherent.insert((ty.ident.to_string(), f.sig.ident.to_string()));
                                }
                            }
                        }
                    }
                }
                SynItem::Impl(imp) => {
                    if let Some(name) = imp.trait_.as_ref().and_then(|(_, path, _)| path.get_ident()) {
                        trait_impls.push((name.to_string(), imp.clone()));
                    }
                }
                SynItem::Trait(t) => {
                    traits.insert(t.ident.to_string(), t.clone());
                }
                SynItem::Enum(e) => {
                    if derives(&e.attrs).iter().any(|d| d == "PartialOrd") {
                        partial_ord.insert(e.ident.to_string());
                    }
                    enums.insert(e.ident.to_string());
                    for v in &e.variants {
                        variant_owner.insert(v.ident.to_string(), e.ident.to_string());
                        variants.insert((e.ident.to_string(), v.ident.to_string()));
                    }
                }
                SynItem::Struct(s) => {
                    if derives(&s.attrs).iter().any(|d| d == "PartialOrd") {
                        partial_ord.insert(s.ident.to_string());
                    }
                    structs.insert(s.ident.to_string());
                }
                _ => {}
            }
        }
        // A trait's consts are read through each implementing type
        // (`Item::N`), declared in the impl or taken from the default.
        trait_impls.retain(|(name, _)| traits.contains_key(name));
        for (name, imp) in &trait_impls {
            if let Some(ty) = crate::traits::impl_owner(imp) {
                for c in crate::traits::consts_of(&traits[name]) {
                    assoc_consts.insert((ty.to_string(), c.ident.to_string()));
                }
            }
        }
        Self {
            enums,
            structs,
            variant_owner,
            variants,
            assoc_consts,
            partial_ord,
            traits,
            trait_impls,
            inherent,
            std_members,
            fresh: std::cell::Cell::new(0),
            recovered: std::cell::RefCell::new(None),
            local_consts: std::cell::RefCell::new(Vec::new()),
            struct_pats: std::cell::RefCell::new(Vec::new()),
            source: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// The file whose items are lowered next.
    pub fn set_source(&self, text: &str) {
        *self.source.borrow_mut() = text.lines().map(String::from).collect();
    }

    /// The `//` lines directly above line `line` (1-based) of the file.
    pub fn comment_above(&self, line: usize) -> Vec<String> {
        let lines = self.source.borrow();
        let mut out = Vec::new();
        for i in (0..line.saturating_sub(1)).rev() {
            match lines.get(i).and_then(|l| crate::expr::comment_text(l)) {
                Some(text) => out.push(text),
                None => break,
            }
        }
        out.reverse();
        out
    }

    pub fn push_struct_pat(&self, pat: StructPat) {
        self.struct_pats.borrow_mut().push(pat);
    }

    /// The struct patterns lowered since the last call, outermost first.
    pub fn take_struct_pats(&self) -> Vec<StructPat> {
        let mut pats = std::mem::take(&mut *self.struct_pats.borrow_mut());
        pats.reverse();
        pats
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

    /// `ty` has the associated const `name`.
    pub fn is_assoc_const(&self, ty: &str, name: &str) -> bool {
        self.assoc_consts.contains(&(ty.to_string(), name.to_string()))
    }

    pub fn is_enum(&self, name: &str) -> bool {
        self.enums.contains(name)
    }

    pub fn is_struct(&self, name: &str) -> bool {
        self.structs.contains(name)
    }

    /// `name` is a trait of the crate.
    pub fn is_trait(&self, name: &str) -> bool {
        self.traits.contains_key(name)
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

    /// Runs `f` failing on the first expression it cannot lower, even in
    /// recovery: for a body that is skipped, not refused, when it fails.
    fn strict<T>(&self, f: impl FnOnce() -> T) -> T {
        let saved = self.recovered.borrow_mut().take();
        let out = f();
        *self.recovered.borrow_mut() = saved;
        out
    }

    /// `recover`, as a result: `Err(e)` when not in recovery.
    pub fn recover_or_fail(&self, e: ParseError) -> Result<(), ParseError> {
        if self.recover(&e) {
            Ok(())
        } else {
            Err(e)
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
pub(crate) fn reject_attrs<'a>(attrs: impl IntoIterator<Item = &'a syn::Attribute>) -> Result<(), ParseError> {
    for attr in attrs {
        let path = attr.path();
        let (reason, message) = if path.is_ident("serde") {
            (
                Reason::SerdeAttr,
                "`#[serde(...)]` is read on a struct, an enum, a variant, or a field (design/04 §3.6), not here",
            )
        } else if path.is_ident("cfg") || path.is_ident("cfg_attr") {
            (Reason::Cfg, "conditional compilation is not in v0: the generated TS cannot follow `#[cfg]`")
        } else {
            continue;
        };
        return Err(ParseError::new(reason, message).or_at(attr.span()));
    }
    Ok(())
}

/// `reject_attrs` but for `#[serde(...)]`, which the caller reads where it
/// is written (`serde_attrs`).
fn reject_other_attrs(attrs: &[syn::Attribute]) -> Result<(), ParseError> {
    reject_attrs(attrs.iter().filter(|a| !a.path().is_ident("serde")))
}

/// The fields of a struct or a struct variant, each with its JSON name:
/// its own `rename`, else `rule` applied.
fn named_fields(named: &syn::FieldsNamed, rule: Option<RenameRule>) -> Result<Vec<Field>, ParseError> {
    named
        .named
        .iter()
        .map(|f| {
            reject_other_attrs(&f.attrs)?;
            let own = serde_attrs(&f.attrs, Place::Field)?.rename;
            let ident = f.ident.as_ref().unwrap();
            Ok(Field {
                name: Name::new(ident.to_string()),
                ty: lower_type(&f.ty)?,
                doc: doc(&f.attrs),
                wire: wire_name(ident, own, rule, false),
            })
        })
        .collect()
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
        SynItem::Trait(t) => &t.attrs,
        _ => &[],
    }
}

/// The name of an item lowered as one named item: a struct, an enum, a
/// function, a type alias, or a `const`.
pub(crate) fn item_ident(item: &SynItem) -> Option<&syn::Ident> {
    match item {
        SynItem::Enum(e) => Some(&e.ident),
        SynItem::Struct(s) => Some(&s.ident),
        SynItem::Fn(f) => Some(&f.sig.ident),
        SynItem::Type(t) => Some(&t.ident),
        SynItem::Const(c) => Some(&c.ident),
        SynItem::Trait(t) => Some(&t.ident),
        _ => None,
    }
}

/// The text of `///` and `/** */` comments (`#[doc = "..."]`), one line
/// each, less the one space after `///`; `None` without any.
pub(crate) fn doc(attrs: &[syn::Attribute]) -> Option<String> {
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

/// The serde derives in `#[derive(...)]`, by the last path segment, so
/// `serde::Serialize` counts as `Serialize`.
fn serde_derives(attrs: &[syn::Attribute]) -> Serde {
    let mut out = Serde::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("derive")) {
        let Ok(paths) =
            attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
        else {
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

/// The last segments of the paths in `#[derive(...)]`: `Ord`, `serde::Serialize`.
fn derives(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("derive")) {
        let Ok(paths) =
            attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
        else {
            continue;
        };
        out.extend(paths.iter().filter_map(|p| p.segments.last().map(|s| s.ident.to_string())));
    }
    out
}

/// What `#[derive(PartialOrd, Ord)]` says of a type's order (design/01
/// §7.25): `Ord` gives it a `cmp`, written by `check::accept`.
fn order_derive(attrs: &[syn::Attribute]) -> Order {
    let derived = derives(attrs);
    if derived.iter().any(|d| d == "Ord") {
        Order::Total
    } else if derived.iter().any(|d| d == "PartialOrd") {
        Order::Partial
    } else {
        Order::None
    }
}

/// `#[cfg(test)]` items are absent from the build the TS mirrors.
pub fn is_test_only(item: &SynItem) -> bool {
    item_attrs(item)
        .iter()
        .any(|attr| attr.path().is_ident("cfg") && attr.parse_args::<syn::Ident>().is_ok_and(|id| id == "test"))
}

/// Each lowered item paired with the location of its name.
pub fn lower_item(cx: &mut Cx, item: SynItem) -> Result<Vec<(Item, LineCol)>, ParseError> {
    let span = item.span();
    let names: Vec<LineCol> = match &item {
        // One per lowered method; `type Error` of a `TryFrom` impl is not one.
        SynItem::Impl(imp) => imp
            .items
            .iter()
            .filter_map(|i| match i {
                syn::ImplItem::Fn(f) => Some(LineCol::of(f.sig.ident.span())),
                syn::ImplItem::Const(c) => Some(LineCol::of(c.ident.span())),
                _ => None,
            })
            .collect(),
        // A derived `default()` follows its type, at the derive.
        other => item_ident(other)
            .map(|id| LineCol::of(id.span()))
            .into_iter()
            .chain(match other {
                SynItem::Struct(_) | SynItem::Enum(_) => derive_span(item_attrs(other), "Default").map(LineCol::of),
                _ => None,
            })
            .collect(),
    };
    reject_attrs(
        item_attrs(&item)
            .iter()
            // A struct's and an enum's are read where they are lowered.
            .filter(|a| !(matches!(item, SynItem::Struct(_) | SynItem::Enum(_)) && a.path().is_ident("serde"))),
    )?;
    let items = lower_item_node(cx, item).map_err(|e| e.or_at(span))?;
    // A trait's default bodies, and the consts an impl takes from the
    // trait's defaults, are at the item.
    let mut names = names;
    let at = names.first().copied().unwrap_or(LineCol::of(span));
    names.resize(items.len().max(names.len()), at);
    Ok(items.into_iter().zip(names).collect())
}

fn lower_item_node(cx: &mut Cx, item: SynItem) -> Result<Vec<Item>, ParseError> {
    match item {
        SynItem::Enum(e) => {
            let lowered = lower_enum(cx, &e)?;
            let default = derived_default(&e.attrs, &lowered.name, &lowered.generics, || enum_default(&e))?;
            Ok(std::iter::once(Item::Enum(lowered)).chain(default.map(Item::Fn)).collect())
        }
        SynItem::Struct(s) => {
            let lowered = lower_struct(cx, &s)?;
            let default = derived_default(&s.attrs, &lowered.name, &lowered.generics, || Ok(struct_default(&lowered)))?;
            Ok(std::iter::once(Item::Struct(lowered)).chain(default.map(Item::Fn)).collect())
        }
        SynItem::Fn(f) => Ok(vec![Item::Fn(lower_fn(cx, None, &f.sig, &f.vis, &f.block, &f.attrs)?)]),
        SynItem::Trait(t) => crate::traits::lower_trait(cx, &t),
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
    let parts = ConstParts { vis: &c.vis, ident: &c.ident, generics: &c.generics, ty: &c.ty, expr: &c.expr };
    lower_const_parts(cx, None, parts, &c.attrs, c.span())
}

/// What an item `const` and an associated one have in common.
pub(crate) struct ConstParts<'a> {
    pub vis: &'a Visibility,
    pub ident: &'a syn::Ident,
    pub generics: &'a syn::Generics,
    pub ty: &'a syn::Type,
    pub expr: &'a syn::Expr,
}

pub(crate) fn lower_const_parts(
    cx: &Cx,
    owner: Option<Name>,
    c: ConstParts<'_>,
    attrs: &[syn::Attribute],
    span: proc_macro2::Span,
) -> Result<Const, ParseError> {
    if !c.generics.params.is_empty() {
        return Err(ParseError::new(Reason::Generics, "generic consts are not in v0"));
    }
    if c.ident == "_" {
        return Err(ParseError::new(Reason::UnsupportedItem, "`const _` is not in v0").detail("const"));
    }
    Ok(Const {
        vis: lower_vis(c.vis),
        name: Name::new(c.ident.to_string()),
        owner,
        ty: lower_type(c.ty)?,
        value: lower_expr(cx, c.expr)?,
        doc: doc(attrs),
        comment: cx.comment_above(span.start().line),
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
        let ty = IntTy::of_suffix(&id.to_string()).ok_or_else(|| {
            ParseError::new(Reason::UnsupportedItem, format!("`#[repr({id})]` is not in v0; use an integer type"))
                .detail("repr")
                .or_at(attr.span())
        })?;
        repr = Some(ty);
    }
    Ok(repr)
}

fn lower_enum(cx: &Cx, e: &syn::ItemEnum) -> Result<Enum, ParseError> {
    let (generics, _) = type_params(cx, &e.generics, Bounded::Type)?;
    let _scope = crate::ty::ParamScope::enter(&generics);
    if e.variants.is_empty() {
        return Err(ParseError::new(
            Reason::UnsupportedItem,
            format!("enum `{}` has no variants; an uninhabited type has no TS union or wire form", e.ident),
        )
        .detail("empty-enum"));
    }
    let fieldless = e.variants.iter().all(|v| matches!(v.fields, SynFields::Unit));
    let container = serde_attrs(&e.attrs, Place::Enum)?;
    let mut variants = Vec::new();
    for v in &e.variants {
        reject_other_attrs(&v.attrs)?;
        let own = serde_attrs(&v.attrs, Place::Variant)?;
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
            fields: lower_fields(&v.fields, own.rename_all)?,
            discriminant,
            doc: doc(&v.attrs),
            wire: wire_name(&v.ident, own.rename, container.rename_all, true),
        });
    }
    Ok(Enum {
        generics,
        vis: lower_vis(&e.vis),
        name: Name::new(e.ident.to_string()),
        variants,
        repr: enum_repr(&e.attrs)?,
        std: false,
        serde: serde_derives(&e.attrs),
        order: order_derive(&e.attrs),
        tagging: match (container.tag, container.content) {
            (None, _) => Tagging::External,
            (Some(tag), None) => Tagging::Internal { tag },
            (Some(tag), Some(content)) => Tagging::Adjacent { tag, content },
        },
        doc: doc(&e.attrs),
    })
}

fn lower_struct(cx: &Cx, s: &syn::ItemStruct) -> Result<Struct, ParseError> {
    let (generics, _) = type_params(cx, &s.generics, Bounded::Type)?;
    let _scope = crate::ty::ParamScope::enter(&generics);
    let container = serde_attrs(&s.attrs, Place::Struct)?;
    let closed = s.fields.iter().any(|f| lower_vis(&f.vis) != Vis::Pub);
    let fields = match &s.fields {
        SynFields::Named(n) => named_fields(n, container.rename_all)?,
        // serde writes `struct S;` as `null` and `struct S {}` as `{}`; the IR
        // keeps only the fields, so one spelling is taken (design/04 §3).
        SynFields::Unit => {
            return Err(ParseError::new(
                Reason::UnitStruct,
                "unit structs are not in v0; write `struct S {}` or an enum",
            ))
        }
        // serde writes a newtype as its content: a `rename` on the one field
        // names nothing.
        SynFields::Unnamed(u) if u.unnamed.len() == 1 => {
            reject_other_attrs(&u.unnamed[0].attrs)?;
            serde_attrs(&u.unnamed[0].attrs, Place::Field)?;
            vec![Field { name: Name::new(NEWTYPE_FIELD), ty: lower_type(&u.unnamed[0].ty)?, doc: None, wire: None }]
        }
        SynFields::Unnamed(_) => {
            return Err(ParseError::new(
                Reason::TupleStruct,
                "tuple structs with more than one field are not in v0; name the fields",
            ))
        }
    };
    let wire_from = container.try_from.as_ref().map(lower_type).transpose()?;
    Ok(Struct {
        generics,
        vis: lower_vis(&s.vis),
        name: Name::new(s.ident.to_string()),
        fields,
        closed,
        wire_from,
        serde: serde_derives(&s.attrs),
        order: order_derive(&s.attrs),
        doc: doc(&s.attrs),
    })
}

/// A variant's fields; `rule` is the variant's own `rename_all`, which
/// names a struct variant's fields.
fn lower_fields(fields: &SynFields, rule: Option<RenameRule>) -> Result<VariantFields, ParseError> {
    match fields {
        SynFields::Unit => Ok(VariantFields::Unit),
        SynFields::Unnamed(u) => {
            let tys = u
                .unnamed
                .iter()
                .map(|f| {
                    reject_other_attrs(&f.attrs)?;
                    serde_attrs(&f.attrs, Place::Field)?;
                    lower_type(&f.ty)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(VariantFields::Tuple(tys))
        }
        SynFields::Named(n) => Ok(VariantFields::Struct(named_fields(n, rule)?)),
    }
}

/// The type a method is folded onto: its name, its type as `self` has it,
/// and the `impl`'s type parameters with the bounds that carry a
/// dictionary (design/01 §7.25).
pub(crate) struct Owner {
    pub name: Name,
    pub ty: purecrate_ir::Ty,
    pub generics: Vec<Name>,
    pub bounds: Vec<(Name, Name)>,
}

impl Owner {
    pub fn plain(name: Name, ty: purecrate_ir::Ty, generics: Vec<Name>) -> Self {
        Owner { name, ty, generics, bounds: Vec::new() }
    }
}

/// Free function (`owner: None`) or method folded onto `owner`'s companion.
/// Each bound that carries a dictionary, the owner's then the function's,
/// adds one parameter after the others (`pricedT: Priced<T>`).
pub(crate) fn lower_fn(
    cx: &Cx,
    owner: Option<Owner>,
    sig: &syn::Signature,
    vis: &Visibility,
    block: &syn::Block,
    attrs: &[syn::Attribute],
) -> Result<Fn, ParseError> {
    let reject = |what: &str, span: proc_macro2::Span| {
        Err(ParseError::new(Reason::FnQualifier, format!("{what} is not allowed in v0")).detail(what).or_at(span))
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
    crate::traits::refuse_impl_return(&sig.output)?;
    // `impl Tr` as a parameter's type: an anonymous type parameter.
    let sig = &crate::traits::impl_params(cx, sig);
    let (own, own_bounds) = type_params(cx, &sig.generics, Bounded::Fn).map_err(|e| e.or_at(sig.generics.span()))?;
    let _scope = crate::ty::ParamScope::enter(&own);
    let (owner, owner_ty, outer, outer_bounds) = match owner {
        Some(o) => (Some(o.name), Some(o.ty), o.generics, o.bounds),
        None => (None, None, Vec::new(), Vec::new()),
    };
    let mut params = Vec::new();
    for input in &sig.inputs {
        params.push(lower_param(owner_ty.as_ref(), input).map_err(|e| e.or_at(input.span()))?);
    }
    for (param, trait_) in outer_bounds.iter().chain(&own_bounds) {
        params.push(Param {
            name: purecrate_ir::dict_param(trait_.as_str(), param.as_str()),
            ty: purecrate_ir::dict_ty(trait_, purecrate_ir::Ty::Param(param.clone())),
        });
    }
    let ret = match &sig.output {
        syn::ReturnType::Default => purecrate_ir::Ty::Prim(purecrate_ir::Prim::Unit),
        syn::ReturnType::Type(_, t) => lower_type(t)?,
    };
    Ok(Fn {
        generics: outer.into_iter().chain(own).collect(),
        vis: lower_vis(vis),
        name: Name::new(sig.ident.to_string()),
        owner,
        params,
        ret,
        body: lower_block(cx, block)?,
        doc: doc(attrs),
        display: false,
    })
}

pub(crate) fn lower_param(owner: Option<&purecrate_ir::Ty>, input: &syn::FnArg) -> Result<Param, ParseError> {
    match input {
        syn::FnArg::Receiver(r) => match owner {
            Some(_) if r.reference.is_some() && r.mutability.is_some() => Err(ParseError::new(
                Reason::RefReceiver,
                "`&mut self` is not in v0: take `self` and return the new value",
            )),
            Some(owner) => Ok(Param { name: Name::new("self"), ty: owner.clone() }),
            None => Err(ParseError::new(Reason::UnsupportedItem, "`self` outside an impl block")),
        },
        syn::FnArg::Typed(p) => match &*p.pat {
            syn::Pat::Ident(id) if id.mutability.is_some() => Err(ParseError::new(
                Reason::MutParam,
                format!("`mut` parameters are not in v0; write `let mut {0} = {0};` in the body", id.ident),
            )),
            syn::Pat::Ident(id) => Ok(Param { name: Name::new(id.ident.to_string()), ty: lower_type(&p.ty)? }),
            _ => Err(ParseError::new(Reason::ParamPattern, "only named parameters")),
        },
    }
}

/// The trait impls v0 takes: `Display` becomes the method `to_string` when
/// its text is fixed per value (`lower_display`), else it is skipped, as
/// `Error` is (a server needs them, for serde's `try_from` among others);
/// `TryFrom<T>` becomes the method `try_from`; `From<S>` the member
/// `fromS` (`lower_from`), `Default` the method `default`; `Ord` the method
/// `cmp`, and `PartialOrd`, written as `cmp` gives, nothing (design/01 §7.25).
enum TraitImpl {
    Display,
    Skipped,
    TryFrom,
    From,
    Default,
    Ord,
    PartialOrd,
}

/// The companion members `item` gives a type through a std trait, derived
/// or written, into `out` (`Cx::std_members`).
fn std_members_of(item: &SynItem, out: &mut HashMap<(String, String), String>) {
    let mut add = |ty: &str, member: &str, what: String| {
        out.entry((ty.to_string(), member.to_string())).or_insert(what);
    };
    let derived = |attrs: &[syn::Attribute], ty: &syn::Ident, add: &mut dyn FnMut(&str, &str, String)| {
        for d in derives(attrs) {
            match d.as_str() {
                "Ord" | "PartialOrd" => add(&ty.to_string(), "cmp", format!("the `cmp` that `#[derive({d})]` gives")),
                "Default" => add(&ty.to_string(), "default", "the `default` that `#[derive(Default)]` gives".into()),
                _ => {}
            }
        }
    };
    match item {
        SynItem::Struct(s) => derived(&s.attrs, &s.ident, &mut add),
        SynItem::Enum(e) => derived(&e.attrs, &e.ident, &mut add),
        SynItem::Impl(imp) => {
            let (Some((_, path, _)), Some(ty)) = (&imp.trait_, crate::traits::impl_owner(imp)) else { return };
            let name = path_text(path);
            let member = match trait_impl(path) {
                Some(TraitImpl::Display) => "toString".to_string(),
                Some(TraitImpl::TryFrom) => "tryFrom".to_string(),
                Some(TraitImpl::Default) => "default".to_string(),
                Some(TraitImpl::Ord | TraitImpl::PartialOrd) => "cmp".to_string(),
                Some(TraitImpl::From) => {
                    let source = match &path.segments.last().map(|s| &s.arguments) {
                        Some(syn::PathArguments::AngleBracketed(a)) => match a.args.first() {
                            Some(syn::GenericArgument::Type(t)) => lower_type(t).ok(),
                            _ => None,
                        },
                        _ => None,
                    };
                    match source.as_ref().and_then(purecrate_ir::from_member) {
                        Some(m) => m,
                        None => return,
                    }
                }
                Some(TraitImpl::Skipped) | None => return,
            };
            add(&ty, &member, format!("the `{member}` that `impl {name}` gives"));
        }
        _ => {}
    }
}

fn trait_impl(path: &syn::Path) -> Option<TraitImpl> {
    let idents = path_idents(path);
    let idents: Vec<&str> = idents.iter().map(String::as_str).collect();
    match idents.as_slice() {
        ["Display"] | ["fmt", "Display"] | ["std" | "core", "fmt", "Display"] => Some(TraitImpl::Display),
        ["Error"] | ["error", "Error"] | ["std" | "core", "error", "Error"] => Some(TraitImpl::Skipped),
        ["Default"] | ["default", "Default"] | ["std" | "core", "default", "Default"] => Some(TraitImpl::Default),
        ["From"] | ["convert", "From"] | ["std" | "core", "convert", "From"] => {
            match &path.segments.last()?.arguments {
                syn::PathArguments::AngleBracketed(a) if a.args.len() == 1 => Some(TraitImpl::From),
                _ => None,
            }
        }
        ["Ord"] | ["cmp", "Ord"] | ["std" | "core", "cmp", "Ord"] => Some(TraitImpl::Ord),
        ["PartialOrd"] | ["cmp", "PartialOrd"] | ["std" | "core", "cmp", "PartialOrd"] => Some(TraitImpl::PartialOrd),
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

/// `impl Display for X` as the method `X::to_string(&self) -> String`
/// (design/01 §7.25). A `fmt` that writes a text fixed per value keeps
/// §7.11's form: its body `f.write_str(t)`, `write!(f, "..")` with no `{}`,
/// or a `match self` whose arms are those or string literals, or `let x =
/// <such a match>; f.write_str(x)`. Otherwise the body is made of writes
/// (`written`): `write!(f, "..", args)`, read as `format!` reads it,
/// `f.write_str(x)` of any `&str`, a `match self` whose arms are writes, and several joined with `?` and
/// ending in `Ok(())`, which the text puts end to end. Any other `fmt` is
/// skipped as before, so a crate that compiled keeps compiling; so is one
/// `check` cannot type (it prints a float), as `Fn::display` marks.
fn lower_display(cx: &mut Cx, imp: &syn::ItemImpl, owner_ident: &syn::Ident) -> Option<Item> {
    let f = imp.items.iter().find_map(|i| match i {
        syn::ImplItem::Fn(f) if f.sig.ident == "fmt" => Some(f),
        _ => None,
    })?;
    let formatter = match f.sig.inputs.iter().nth(1)? {
        syn::FnArg::Typed(p) => match &*p.pat {
            syn::Pat::Ident(id) => id.ident.clone(),
            _ => return None,
        },
        syn::FnArg::Receiver(_) => return None,
    };
    let text = match f.block.stmts.as_slice() {
        [syn::Stmt::Expr(tail, None)] => written_text(tail, &formatter),
        [syn::Stmt::Local(local), syn::Stmt::Expr(tail, None)] => {
            let (syn::Pat::Ident(bound), Some(init)) = (&local.pat, &local.init) else { return None };
            match written_arg(tail, &formatter) {
                Some(syn::Expr::Path(p)) if p.path.is_ident(&bound.ident) => fixed_text(&init.expr),
                _ => None,
            }
        }
        _ => None,
    };
    let body = match text {
        Some(mut text) => {
            SelfIsOwner(owner_ident, None).visit_expr_mut(&mut text);
            let body = lower_expr(cx, &text).ok()?;
            purecrate_ir::Expr::Call { callee: purecrate_ir::Callee::StringFrom, args: vec![body] }
        }
        None => {
            let mut block = f.block.clone();
            SelfIsOwner(owner_ident, None).visit_block_mut(&mut block);
            cx.strict(|| written(cx, &block.stmts, &formatter, &mut 0))?
        }
    };
    let owner = Name::new(owner_ident.to_string());
    Some(Item::Fn(Fn {
        generics: Vec::new(),
        vis: Vis::Pub,
        name: Name::new("to_string"),
        owner: Some(owner.clone()),
        params: vec![Param { name: Name::new("self"), ty: purecrate_ir::Ty::Named(owner) }],
        ret: purecrate_ir::Ty::Prim(purecrate_ir::Prim::String),
        body,
        doc: doc(&imp.attrs),
        display: true,
    }))
}

/// The `String` that the statements of a `fmt` body write to `formatter`:
/// `let`s, then writes, each but the last followed by `?`, the last bare or
/// followed by `Ok(())`. The texts of several writes are put end to end
/// (`a + b`), each its own template, so each `fmt` an argument calls runs
/// after that write's arguments and before the next write's, as in Rust.
/// `next` numbers the placeholders: the `let`s are lowered as a block whose
/// value is one, then replaced by the text.
fn written(cx: &Cx, stmts: &[syn::Stmt], formatter: &syn::Ident, next: &mut usize) -> Option<purecrate_ir::Expr> {
    let lets = stmts.iter().take_while(|s| matches!(s, syn::Stmt::Local(_))).count();
    if lets > 0 {
        let text = written(cx, &stmts[lets..], formatter, next)?;
        let name = format!("__display_text_{next}");
        *next += 1;
        let placeholder = syn::Ident::new(&name, proc_macro2::Span::call_site());
        let mut block: syn::Block = syn::parse_quote!({ #placeholder });
        block.stmts.splice(0..0, stmts[..lets].iter().cloned());
        let mut lowered = lower_block(cx, &block).ok()?;
        fill_placeholder(&mut lowered, &name, &text);
        return Some(lowered);
    }
    let (last, init) = stmts.split_last()?;
    let mut parts = Vec::new();
    for s in init {
        let syn::Stmt::Expr(syn::Expr::Try(t), Some(_)) = s else { return None };
        parts.push(written_part(cx, &t.expr, formatter, next)?);
    }
    let tail = match last {
        syn::Stmt::Expr(e, None) => e.clone(),
        syn::Stmt::Macro(m) if m.semi_token.is_none() => {
            syn::Expr::Macro(syn::ExprMacro { attrs: m.attrs.clone(), mac: m.mac.clone() })
        }
        _ => return None,
    };
    if !is_ok_unit(&tail) {
        parts.push(written_part(cx, &tail, formatter, next)?);
    }
    let empty = || purecrate_ir::Expr::Call {
        callee: purecrate_ir::Callee::StringFrom,
        args: vec![purecrate_ir::Expr::Lit(purecrate_ir::Lit::Str(String::new()))],
    };
    let concat = |a, b| purecrate_ir::Expr::Call { callee: purecrate_ir::Callee::StrConcat, args: vec![a, b] };
    Some(parts.into_iter().reduce(concat).unwrap_or_else(empty))
}

/// One write: `write!(formatter, "..", args)`, `formatter.write_str(x)`
/// (a computed `x` as `write!(formatter, "{}", x)`), `Ok(())` (nothing),
/// a block of writes, or a `match self` whose arms are writes. The `match`
/// is lowered with a placeholder for each arm's body, then each placeholder
/// is replaced by that body's text.
fn written_part(cx: &Cx, e: &syn::Expr, formatter: &syn::Ident, next: &mut usize) -> Option<purecrate_ir::Expr> {
    match e {
        syn::Expr::Macro(m) if m.mac.path.is_ident("write") => crate::expr::lower_write(cx, &m.mac, formatter),
        syn::Expr::MethodCall(_) => {
            let text = written_arg(e, formatter)?;
            if literal(&text).is_some() {
                let text = lower_expr(cx, &text).ok()?;
                return Some(purecrate_ir::Expr::Call { callee: purecrate_ir::Callee::StringFrom, args: vec![text] });
            }
            // A computed text writes as `write!(f, "{}", x)` does.
            let mac: syn::Macro = syn::parse_quote!(write!(#formatter, "{}", #text));
            crate::expr::lower_write(cx, &mac, formatter)
        }
        syn::Expr::Block(b) if b.label.is_none() => written(cx, &b.block.stmts, formatter, next),
        // An arm that writes nothing.
        _ if is_ok_unit(e) => written(cx, &[syn::Stmt::Expr(e.clone(), None)], formatter, next),
        syn::Expr::Match(m) if is_self(&m.expr) => {
            let mut m = m.clone();
            let mut bodies = Vec::new();
            for arm in &mut m.arms {
                let name = format!("__display_arm_{next}");
                *next += 1;
                bodies.push((name.clone(), written_part(cx, &arm.body, formatter, next)?));
                let placeholder = syn::Ident::new(&name, arm.body.span());
                *arm.body = syn::parse_quote!(#placeholder);
            }
            let mut lowered = lower_expr(cx, &syn::Expr::Match(m)).ok()?;
            for (name, body) in &bodies {
                fill_placeholder(&mut lowered, name, body);
            }
            Some(lowered)
        }
        _ => None,
    }
}

/// Replaces the variable `name` in `e` with `body`.
fn fill_placeholder(e: &mut purecrate_ir::Expr, name: &str, body: &purecrate_ir::Expr) {
    if matches!(e, purecrate_ir::Expr::Var(n) if n.as_str() == name) {
        *e = body.clone();
        return;
    }
    for child in e.children_mut() {
        fill_placeholder(child, name, body);
    }
}

/// `Ok(())`.
fn is_ok_unit(e: &syn::Expr) -> bool {
    match e {
        syn::Expr::Call(c) if c.args.len() == 1 => {
            matches!(&*c.func, syn::Expr::Path(p) if p.path.is_ident("Ok"))
                && matches!(&c.args[0], syn::Expr::Tuple(t) if t.elems.is_empty())
        }
        _ => false,
    }
}

/// The `&str` that `e` writes to `formatter`, when it is fixed per value.
fn written_text(e: &syn::Expr, formatter: &syn::Ident) -> Option<syn::Expr> {
    match e {
        syn::Expr::Match(m) if is_self(&m.expr) => {
            let mut m = m.clone();
            for arm in &mut m.arms {
                if arm.guard.is_some() {
                    return None;
                }
                *arm.body = written_text(&arm.body, formatter).or_else(|| literal(&arm.body))?;
            }
            Some(syn::Expr::Match(m))
        }
        other => fixed_text(&written_arg(other, formatter)?),
    }
}

/// The argument of `formatter.write_str(x)`, or the literal of
/// `write!(formatter, "..")` with no formatting arguments or `{}`.
fn written_arg(e: &syn::Expr, formatter: &syn::Ident) -> Option<syn::Expr> {
    match e {
        syn::Expr::MethodCall(c) if c.method == "write_str" && c.args.len() == 1 => match &*c.receiver {
            syn::Expr::Path(p) if p.path.is_ident(formatter) => Some(c.args[0].clone()),
            _ => None,
        },
        syn::Expr::Macro(m) if m.mac.path.is_ident("write") => {
            let args = m
                .mac
                .parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
                .ok()?;
            let [syn::Expr::Path(target), syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. })] =
                args.iter().collect::<Vec<_>>()[..]
            else {
                return None;
            };
            if !target.path.is_ident(formatter) {
                return None;
            }
            let value = text.value();
            let plain = value.replace("{{", "").replace("}}", "");
            if plain.contains('{') || plain.contains('}') {
                return None;
            }
            let unescaped = value.replace("{{", "{").replace("}}", "}");
            Some(syn::Expr::Lit(syn::ExprLit {
                attrs: Vec::new(),
                lit: syn::Lit::Str(syn::LitStr::new(&unescaped, text.span())),
            }))
        }
        _ => None,
    }
}

/// `e` when it is a string literal or a `match self` whose arms are.
fn fixed_text(e: &syn::Expr) -> Option<syn::Expr> {
    match e {
        syn::Expr::Match(m)
            if is_self(&m.expr) && m.arms.iter().all(|a| a.guard.is_none() && literal(&a.body).is_some()) =>
        {
            Some(e.clone())
        }
        other => literal(other),
    }
}

fn literal(e: &syn::Expr) -> Option<syn::Expr> {
    matches!(e, syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(_), .. })).then(|| e.clone())
}

fn is_self(e: &syn::Expr) -> bool {
    matches!(e, syn::Expr::Path(p) if p.path.is_ident("self"))
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
                SelfIsOwner(owner_ident, None).visit_impl_item_fn_mut(&mut f);
                let public = Visibility::Public(Default::default());
                let owned = Owner::plain(owner.clone(), purecrate_ir::Ty::Named(owner.clone()), Vec::new());
                let lowered =
                    lower_fn(cx, Some(owned), &f.sig, &public, &f.block, &f.attrs).map_err(|e| e.or_at(item.span()))?;
                out.push(Item::Fn(lowered));
            }
            other => {
                return Err(ParseError::new(
                    Reason::ImplShape,
                    "`impl TryFrom` has only `type Error` and `fn try_from`",
                )
                .or_at(other.span()))
            }
        }
    }
    Ok(out)
}

/// `impl From<S> for X { fn from(s: S) -> Self }` as `X`'s public member
/// named after `S` (`purecrate_ir::from_member`: `fromU32`,
/// `fromParseIntError`), as a type may implement `From` for several sources.
/// A source that is a reference, a tuple, an `Option`, or a type with
/// arguments has no such name and is refused.
fn lower_from(cx: &mut Cx, imp: &syn::ItemImpl, owner_ident: &syn::Ident) -> Result<Vec<Item>, ParseError> {
    let owner = Name::new(owner_ident.to_string());
    let shape = || ParseError::new(Reason::ImplShape, "`impl From` has only `fn from(s: S) -> Self`");
    let [syn::ImplItem::Fn(f)] = imp.items.as_slice() else {
        return Err(shape().or_at(imp.span()));
    };
    if f.sig.ident != "from" || f.sig.inputs.len() != 1 {
        return Err(shape().or_at(f.span()));
    }
    reject_attrs(&f.attrs)?;
    let Some(syn::FnArg::Typed(param)) = f.sig.inputs.first() else {
        return Err(shape().or_at(f.span()));
    };
    let source = lower_type(&param.ty).map_err(|e| e.or_at(param.ty.span()))?;
    let refused = |why: &str| {
        Err(ParseError::new(
            Reason::TraitImpl,
            format!(
                "`impl From<{}> for {}`: {why} is not in v0, as the member is named after its source type (`fromU32`, `fromMyError`); convert from a named type",
                snippet(&*param.ty).trim_matches('`'),
                owner.as_str()
            ),
        )
        .or_at(param.ty.span()))
    };
    let member = match (&*param.ty, &source) {
        (syn::Type::Reference(_), _) => return refused("a reference source"),
        (syn::Type::Tuple(_), _) => return refused("a tuple source"),
        (_, purecrate_ir::Ty::Option(_)) => return refused("an `Option` source"),
        _ => match purecrate_ir::from_member(&source) {
            Some(m) => m,
            None => return refused("a source with type arguments"),
        },
    };
    let mut f = f.clone();
    f.sig.ident = syn::Ident::new(&member, f.sig.ident.span());
    SelfIsOwner(owner_ident, None).visit_impl_item_fn_mut(&mut f);
    let public = Visibility::Public(Default::default());
    let owned = Owner::plain(owner.clone(), purecrate_ir::Ty::Named(owner), Vec::new());
    Ok(vec![Item::Fn(lower_fn(cx, Some(owned), &f.sig, &public, &f.block, &f.attrs).map_err(|e| e.or_at(f.span()))?)])
}

/// `impl Default for X { fn default() -> Self }` as `X`'s public method
/// `default`, which `#[derive(Default)]` writes too (`derived_default`).
fn lower_default(cx: &mut Cx, imp: &syn::ItemImpl, owner_ident: &syn::Ident) -> Result<Vec<Item>, ParseError> {
    let shape = || ParseError::new(Reason::ImplShape, "`impl Default` has only `fn default() -> Self`");
    let [syn::ImplItem::Fn(f)] = imp.items.as_slice() else {
        return Err(shape().or_at(imp.span()));
    };
    if f.sig.ident != "default" || !f.sig.inputs.is_empty() {
        return Err(shape().or_at(f.span()));
    }
    reject_attrs(&f.attrs)?;
    let mut f = f.clone();
    SelfIsOwner(owner_ident, None).visit_impl_item_fn_mut(&mut f);
    let owner = Name::new(owner_ident.to_string());
    let public = Visibility::Public(Default::default());
    let owned = Owner::plain(owner.clone(), purecrate_ir::Ty::Named(owner), Vec::new());
    Ok(vec![Item::Fn(lower_fn(cx, Some(owned), &f.sig, &public, &f.block, &f.attrs).map_err(|e| e.or_at(f.span()))?)])
}

/// `#[derive(Default)]` as the method `X::default()`, public like the
/// trait: on a struct each field's `Default::default()`, which the check
/// reads at the field's type (`Callee::Default`); on an enum its
/// `#[default]` unit variant. `None` without the derive. On a type with
/// type parameters it is generic as the derive's `impl<T: Default>` is,
/// taking a `Default` dictionary per parameter (design/01 §7.25).
fn derived_default(
    attrs: &[syn::Attribute],
    name: &Name,
    generics: &[Name],
    body: impl FnOnce() -> Result<purecrate_ir::Expr, ParseError>,
) -> Result<Option<Fn>, ParseError> {
    let Some(at) = derive_span(attrs, "Default") else { return Ok(None) };
    let params = generics
        .iter()
        .map(|g| Param {
            name: purecrate_ir::dict_param("Default", g.as_str()),
            ty: purecrate_ir::dict_ty(&Name::new("Default"), purecrate_ir::Ty::Param(g.clone())),
        })
        .collect();
    let ret = if generics.is_empty() {
        purecrate_ir::Ty::Named(name.clone())
    } else {
        purecrate_ir::Ty::App {
            name: name.clone(),
            args: generics.iter().cloned().map(purecrate_ir::Ty::Param).collect(),
        }
    };
    Ok(Some(Fn {
        generics: generics.to_vec(),
        vis: Vis::Pub,
        name: Name::new("default"),
        owner: Some(name.clone()),
        params,
        ret,
        body: body().map_err(|e| e.or_at(at))?,
        doc: None,
        display: false,
    }))
}

/// Where `#[derive(..)]` names `which`, by the last path segment.
fn derive_span(attrs: &[syn::Attribute], which: &str) -> Option<proc_macro2::Span> {
    attrs.iter().filter(|a| a.path().is_ident("derive")).find_map(|attr| {
        let paths =
            attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated).ok()?;
        paths.iter().find(|p| p.segments.last().is_some_and(|s| s.ident == which)).map(|p| p.span())
    })
}

/// A struct's derived `default()`: `X { f: Default::default(), .. }`, or
/// `X(Default::default())` for a newtype.
fn struct_default(s: &Struct) -> purecrate_ir::Expr {
    use purecrate_ir::{Callee, Expr, Fields};
    let default = |f: &Field| Expr::Call { callee: Callee::Default { field: Some(f.name.clone()) }, args: Vec::new() };
    match s.fields.as_slice() {
        [f] if f.name.as_str() == NEWTYPE_FIELD => {
            Expr::Call { callee: Callee::StructNew(s.name.clone()), args: vec![default(f)] }
        }
        fields => Expr::Construct {
            ty: s.name.clone(),
            variant: None,
            fields: Fields::Named(fields.iter().map(|f| (f.name.clone(), default(f))).collect()),
            base: None,
        },
    }
}

/// An enum's derived `default()`: its `#[default]` variant, which has no
/// fields.
fn enum_default(e: &syn::ItemEnum) -> Result<purecrate_ir::Expr, ParseError> {
    let marked: Vec<&syn::Variant> =
        e.variants.iter().filter(|v| v.attrs.iter().any(|a| a.path().is_ident("default"))).collect();
    match marked.as_slice() {
        [v] if matches!(v.fields, SynFields::Unit) => Ok(purecrate_ir::Expr::Construct {
            ty: Name::new(e.ident.to_string()),
            variant: Some(Name::new(v.ident.to_string())),
            fields: purecrate_ir::Fields::Unit,
            base: None,
        }),
        _ => Err(ParseError::new(
            Reason::UnsupportedItem,
            format!("`#[derive(Default)]` on the enum `{}` needs one `#[default]` variant without fields", e.ident),
        )
        .detail("default")),
    }
}

/// `impl Ord for X` as the method `X::cmp`: `fn cmp` is its one item, as
/// std's other methods (`max`, `min`, `clamp`) follow from it. Refused
/// beside a derived `PartialOrd`, which orders `<` field by field whatever
/// `cmp` says.
fn lower_ord(cx: &mut Cx, imp: &syn::ItemImpl, owner_ident: &syn::Ident) -> Result<Vec<Item>, ParseError> {
    if cx.partial_ord.contains(&owner_ident.to_string()) {
        return Err(ParseError::new(
            Reason::ImplShape,
            format!("`{owner_ident}` derives `PartialOrd` and writes `impl Ord`: `<` would compare field by field and `cmp` by hand; write `impl PartialOrd` as `Some(self.cmp(other))`"),
        ));
    }
    let owner = Name::new(owner_ident.to_string());
    let mut out = Vec::new();
    for item in &imp.items {
        match item {
            syn::ImplItem::Fn(f) if f.sig.ident == "cmp" => {
                reject_attrs(&f.attrs)?;
                let mut f = f.clone();
                SelfIsOwner(owner_ident, None).visit_impl_item_fn_mut(&mut f);
                let public = Visibility::Public(Default::default());
                let owned = Owner::plain(owner.clone(), purecrate_ir::Ty::Named(owner.clone()), Vec::new());
                let lowered =
                    lower_fn(cx, Some(owned), &f.sig, &public, &f.block, &f.attrs).map_err(|e| e.or_at(item.span()))?;
                out.push(Item::Fn(lowered));
            }
            other => {
                return Err(ParseError::new(
                    Reason::ImplShape,
                    "`impl Ord` has only `fn cmp` in v0: std's `max`, `min`, and `clamp` follow from it",
                )
                .or_at(other.span()))
            }
        }
    }
    if out.is_empty() {
        return Err(ParseError::new(Reason::ImplShape, "`impl Ord` needs `fn cmp(&self, other: &Self) -> Ordering`"));
    }
    Ok(out)
}

/// `impl PartialOrd for X` whose `partial_cmp` is `Some(self.cmp(other))`,
/// as clippy asks beside `Ord`: `<` is then `cmp`'s, and the impl adds no
/// member. Any other body may disagree with `cmp` (one giving `None`
/// makes every `<` false), so it is refused.
fn partial_cmp_is_cmp(imp: &syn::ItemImpl) -> Result<(), ParseError> {
    let refused = || {
        ParseError::new(
            Reason::ImplShape,
            "`impl PartialOrd` is taken in v0 only as `fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }`, beside `Ord`: any other `partial_cmp` may order `<` otherwise than `cmp`",
        )
    };
    let [syn::ImplItem::Fn(f)] = imp.items.as_slice() else { return Err(refused()) };
    if f.sig.ident != "partial_cmp" {
        return Err(refused().or_at(f.sig.ident.span()));
    }
    let other = match f.sig.inputs.iter().collect::<Vec<_>>().as_slice() {
        [syn::FnArg::Receiver(r), syn::FnArg::Typed(p)] if r.reference.is_some() && r.mutability.is_none() => {
            match &*p.pat {
                syn::Pat::Ident(id) => id.ident.clone(),
                _ => return Err(refused().or_at(p.span())),
            }
        }
        _ => return Err(refused().or_at(f.sig.span())),
    };
    let is_cmp = match f.block.stmts.as_slice() {
        [syn::Stmt::Expr(syn::Expr::Call(some), None)] => {
            match (&*some.func, some.args.iter().collect::<Vec<_>>().as_slice()) {
                (syn::Expr::Path(p), [syn::Expr::MethodCall(c)]) => {
                    p.path.is_ident("Some")
                        && c.method == "cmp"
                        && is_self(&c.receiver)
                        && matches!(c.args.iter().collect::<Vec<_>>().as_slice(), [syn::Expr::Path(a)] if a.path.is_ident(&other))
                }
                _ => false,
            }
        }
        _ => false,
    };
    if is_cmp {
        Ok(())
    } else {
        Err(refused().or_at(f.block.span()))
    }
}

/// Rewrites the type `Self::Error` to the impl's `type Error`.
struct SelfError<'a>(&'a syn::Type);

impl VisitMut for SelfError<'_> {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let syn::Type::Path(p) = ty {
            let idents = path_idents(&p.path);
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
    if let Some(name) =
        imp.trait_.as_ref().and_then(|(_, path, _)| path.get_ident()).filter(|n| cx.is_trait(&n.to_string()))
    {
        let name = name.clone();
        return crate::traits::lower_trait_impl(cx, &imp, &name);
    }
    let kind = match &imp.trait_ {
        None => None,
        Some((_, path, _)) if path.segments.last().is_some_and(|s| s.ident == "Into") => {
            return Err(ParseError::new(
                Reason::TraitImpl,
                "`impl Into` is not in v0: write `impl From<S> for X`, which gives `s.into()` too (design/01 §7.25)",
            ))
        }
        Some((_, path, _)) => Some(trait_impl(path).ok_or_else(|| {
            ParseError::new(
                Reason::TraitImpl,
                "trait impls are not in v0, except `Display`, `Error` (skipped), `TryFrom<T>`, `From<S>`, `Default`, `Ord`, and `PartialOrd`, and of the crate's own traits",
            )
        })?),
    };
    if matches!(kind, Some(TraitImpl::Skipped)) {
        return Ok(Vec::new());
    }
    if matches!(kind, Some(TraitImpl::From)) && has_type_generics(&imp.generics) {
        return Err(ParseError::new(
            Reason::Generics,
            "a generic `impl From` (`impl<T> From<T> for X`) is not in v0: each source is a member named after its type; write one `impl From<S>` per source type",
        ));
    }
    if kind.is_some() && has_type_generics(&imp.generics) {
        return Err(ParseError::new(
            Reason::Generics,
            "a generic trait impl is not in v0; write an inherent `impl<T> S<T>`",
        ));
    }
    let (outer, outer_bounds) = type_params(cx, &imp.generics, Bounded::Fn)?;
    let _scope = crate::ty::ParamScope::enter(&outer);
    let owner_ty = crate::ty::lower_type(&imp.self_ty)?;
    let owner_ident = match &*imp.self_ty {
        syn::Type::Path(p) if p.path.segments.len() == 1 => p.path.segments[0].ident.clone(),
        _ => {
            return Err(ParseError::new(Reason::ImplShape, "impl type must be a simple name").or_at(imp.self_ty.span()))
        }
    };
    if matches!(kind, Some(TraitImpl::From | TraitImpl::Default)) {
        let owner = owner_ident.to_string();
        if !(cx.is_struct(&owner) || cx.is_enum(&owner)) {
            let tr = if matches!(kind, Some(TraitImpl::From)) { "From" } else { "Default" };
            return Err(ParseError::new(
                Reason::TraitImpl,
                format!(
                    "`impl {tr}` for `{owner}`, a type outside the crate, is not in v0: there is no companion to put it in; implement it on a struct or enum of the crate, or write a method that returns a `{owner}`"
                ),
            )
            .or_at(imp.self_ty.span()));
        }
        // `impl Default for Page<u32>`: one instantiation of a generic type,
        // whose companion member is shared by all of them.
        if matches!(owner_ty, purecrate_ir::Ty::App { .. }) {
            let tr = if matches!(kind, Some(TraitImpl::From)) { "From" } else { "Default" };
            return Err(ParseError::new(
                Reason::Generics,
                format!(
                    "`impl {tr}` for `{}`, a type with type parameters, is not in v0: its companion member would serve every instantiation; write a function that returns this one",
                    snippet(&*imp.self_ty).trim_matches('`')
                ),
            )
            .or_at(imp.self_ty.span()));
        }
    }
    if matches!(kind, Some(TraitImpl::TryFrom)) {
        return lower_try_from(cx, &imp, &owner_ident);
    }
    if matches!(kind, Some(TraitImpl::From)) {
        return lower_from(cx, &imp, &owner_ident);
    }
    if matches!(kind, Some(TraitImpl::Default)) {
        return lower_default(cx, &imp, &owner_ident);
    }
    if matches!(kind, Some(TraitImpl::Display)) {
        return Ok(lower_display(cx, &imp, &owner_ident).into_iter().collect());
    }
    if matches!(kind, Some(TraitImpl::Ord)) {
        return lower_ord(cx, &imp, &owner_ident);
    }
    if matches!(kind, Some(TraitImpl::PartialOrd)) {
        return partial_cmp_is_cmp(&imp).map(|()| Vec::new());
    }
    let owner = Name::new(owner_ident.to_string());
    let mut out = Vec::new();
    for item in &imp.items {
        let lowered = match item {
            syn::ImplItem::Fn(f) => reject_attrs(&f.attrs).and_then(|()| {
                let mut f = f.clone();
                SelfIsOwner(&owner_ident, Some(&imp.self_ty)).visit_impl_item_fn_mut(&mut f);
                let owned = Owner {
                    name: owner.clone(),
                    ty: owner_ty.clone(),
                    generics: outer.clone(),
                    bounds: outer_bounds.clone(),
                };
                lower_fn(cx, Some(owned), &f.sig, &f.vis, &f.block, &f.attrs).map(Item::Fn)
            }),
            // A member of the companion, read as `T::N` (`Self::N` here).
            syn::ImplItem::Const(c) => reject_attrs(&c.attrs).and_then(|()| {
                let mut c = c.clone();
                SelfIsOwner(&owner_ident, Some(&imp.self_ty)).visit_impl_item_const_mut(&mut c);
                let parts =
                    ConstParts { vis: &c.vis, ident: &c.ident, generics: &c.generics, ty: &c.ty, expr: &c.expr };
                lower_const_parts(cx, Some(owner.clone()), parts, &c.attrs, c.span()).map(Item::Const)
            }),
            _ => Err(ParseError::new(Reason::ImplShape, "only methods and consts in impl blocks in v0")),
        };
        out.push(lowered.map_err(|e| e.or_at(item.span()))?);
    }
    Ok(out)
}

/// Rewrites `Self` to the impl's type name in types, paths and patterns;
/// as a type, to the impl's whole type (`Page<T>`) where it is given.
/// Nested items keep their own `Self`.
pub(crate) struct SelfIsOwner<'a>(pub &'a syn::Ident, pub Option<&'a syn::Type>);

impl VisitMut for SelfIsOwner<'_> {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if let (Some(full), syn::Type::Path(p)) = (self.1, &*ty) {
            if p.qself.is_none() && p.path.is_ident("Self") {
                *ty = full.clone();
                return;
            }
        }
        visit_mut::visit_type_mut(self, ty);
    }

    fn visit_path_mut(&mut self, path: &mut syn::Path) {
        if let Some(first) = path.segments.first_mut() {
            if first.ident == "Self" {
                first.ident = syn::Ident::new(&self.0.to_string(), first.ident.span());
            }
        }
        visit_mut::visit_path_mut(self, path);
    }

    /// A macro's body is parsed only when it is lowered (`matches!(x,
    /// Self::A)`, `assert!(n < Self::MAX)`), so `Self` is the type's name
    /// there as a token.
    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        fn rename(tokens: proc_macro2::TokenStream, owner: &syn::Ident) -> proc_macro2::TokenStream {
            use proc_macro2::{Group, TokenTree};
            tokens
                .into_iter()
                .map(|t| match t {
                    TokenTree::Ident(id) if id == "Self" => {
                        TokenTree::Ident(syn::Ident::new(&owner.to_string(), id.span()))
                    }
                    TokenTree::Group(g) => {
                        let mut inner = Group::new(g.delimiter(), rename(g.stream(), owner));
                        inner.set_span(g.span());
                        TokenTree::Group(inner)
                    }
                    other => other,
                })
                .collect()
        }
        mac.tokens = rename(std::mem::take(&mut mac.tokens), self.0);
        visit_mut::visit_macro_mut(self, mac);
    }

    fn visit_item_mut(&mut self, _: &mut SynItem) {}
}

/// The traits a type parameter may be bounded by without a dictionary: what
/// the structural output honors (`Clone` is the value, `==` is `Eq.deep`),
/// and `Debug`, accepted but not usable on a `T` (design/01 §7.20).
const BOUNDS: &[&str] = &["Clone", "Copy", "PartialEq", "Eq", "Debug"];

/// Where type parameters are declared: a struct's or enum's may not have
/// a bound that carries a dictionary (design/01 §7.25); an `impl`'s and a
/// function's may.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bounded {
    Type,
    Fn,
}

/// A bound that carries a dictionary: a crate trait, or one of
/// `STD_DICT_TRAITS`.
fn carries_dict(cx: &Cx, name: &str) -> bool {
    cx.is_trait(name) || purecrate_ir::STD_DICT_TRAITS.contains(&name)
}

/// The std trait of `STD_DICT_TRAITS` a bound names (design/01 §7.25):
/// `Ord`, `PartialOrd`, `Display`, or `Default`, alone where the crate has
/// no trait of that name, or by its path (`std::cmp::Ord`, `fmt::Display`,
/// `std::default::Default`).
fn std_dict_bound(cx: &Cx, path: &syn::Path) -> Option<String> {
    let last = path.segments.last()?;
    let name = last.ident.to_string();
    if !purecrate_ir::STD_DICT_TRAITS.contains(&name.as_str()) || !last.arguments.is_none() {
        return None;
    }
    let module = match name.as_str() {
        "Display" => "fmt",
        "Default" => "default",
        _ => "cmp",
    };
    let prefix: Vec<String> = path.segments.iter().rev().skip(1).map(|s| s.ident.to_string()).collect();
    let std_path = match prefix.as_slice() {
        [] => path.leading_colon.is_none() && !cx.is_trait(&name),
        [m] => m == module,
        [m, root] => m == module && (root == "std" || root == "core"),
        _ => false,
    };
    std_path.then_some(name)
}

/// Type parameters, and each (parameter, trait) whose bound carries a
/// dictionary.
pub(crate) type TypeParams = (Vec<Name>, Vec<(Name, Name)>);

/// The type parameters of an item, their bounds checked; lifetimes skipped.
/// Also each (parameter, trait) whose bound carries a dictionary, each
/// parameter's in the order written (on the parameter, then in `where`).
pub(crate) fn type_params(cx: &Cx, generics: &syn::Generics, at: Bounded) -> Result<TypeParams, ParseError> {
    let mut dicts: Vec<(Name, Name)> = Vec::new();
    let mut bounded = |param: &Name, bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>| {
        for b in bounds {
            match b {
                syn::TypeParamBound::Trait(t)
                    if matches!(t.modifier, syn::TraitBoundModifier::None)
                        && t.path.get_ident().is_some_and(|i| BOUNDS.contains(&i.to_string().as_str())) => {}
                // A std bound is the parameter's: a struct's or enum's
                // carries nothing, as in 0.20.0.
                syn::TypeParamBound::Trait(t)
                    if matches!(t.modifier, syn::TraitBoundModifier::None)
                        && at == Bounded::Type
                        && std_dict_bound(cx, &t.path).is_some() => {}
                syn::TypeParamBound::Trait(t)
                    if matches!(t.modifier, syn::TraitBoundModifier::None) && std_dict_bound(cx, &t.path).is_some() =>
                {
                    dicts.push((param.clone(), Name::new(std_dict_bound(cx, &t.path).expect("checked by the guard"))));
                }
                syn::TypeParamBound::Trait(t)
                    if matches!(t.modifier, syn::TraitBoundModifier::None)
                        && t.path.get_ident().is_some_and(|i| carries_dict(cx, &i.to_string())) =>
                {
                    let name = t.path.get_ident().expect("checked by the guard").to_string();
                    if at == Bounded::Type {
                        return Err(ParseError::new(
                            Reason::Generics,
                            format!(
                                "the bound `{name}` on a struct's or enum's type parameter is not in v0 (design/01 §7.25): put it on the `impl` or the function that calls the trait's methods"
                            ),
                        ));
                    }
                    dicts.push((param.clone(), Name::new(name)));
                }
                syn::TypeParamBound::Lifetime(_) => {}
                syn::TypeParamBound::Trait(t)
                    if t.path.segments.last().is_some_and(|s| s.ident == "Into" || s.ident == "From") =>
                {
                    return Err(ParseError::new(
                        Reason::Generics,
                        format!(
                            "the bound {} is not in v0: a conversion is resolved where both types are known; take the target type and convert at the call (`X::from(s)`, `s.into()`)",
                            snippet(t)
                        ),
                    ))
                }
                other => {
                    return Err(ParseError::new(
                        Reason::Generics,
                        format!(
                            "the bound {} is not in v0: a type parameter may be bounded by a trait of the crate, `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Display`, `Default`, and `Debug`",
                            snippet(other)
                        ),
                    ))
                }
            }
        }
        Ok(())
    };
    let mut names = Vec::new();
    for p in &generics.params {
        match p {
            syn::GenericParam::Lifetime(_) => {}
            syn::GenericParam::Const(c) => {
                return Err(ParseError::new(Reason::Generics, "const generics are not in v0").or_at(c.span()))
            }
            syn::GenericParam::Type(t) => {
                if t.default.is_some() {
                    return Err(
                        ParseError::new(Reason::Generics, "a type parameter's default is not in v0").or_at(t.span())
                    );
                }
                let name = Name::new(t.ident.to_string());
                bounded(&name, &t.bounds)?;
                names.push(name);
            }
        }
    }
    if let Some(w) = &generics.where_clause {
        for pred in &w.predicates {
            match pred {
                syn::WherePredicate::Type(t) if matches!(&t.bounded_ty, syn::Type::Path(p) if p.path.get_ident().is_some_and(|i| names.iter().any(|n| i == n.as_str()))) =>
                {
                    let syn::Type::Path(p) = &t.bounded_ty else { unreachable!("checked by the guard") };
                    let name = Name::new(p.path.get_ident().expect("checked by the guard").to_string());
                    bounded(&name, &t.bounds)?
                }
                syn::WherePredicate::Lifetime(_) => {}
                other => {
                    return Err(ParseError::new(
                        Reason::Generics,
                        format!(
                            "the `where` clause {} is not in v0: bound a type parameter by the traits listed",
                            snippet(other)
                        ),
                    ))
                }
            }
        }
    }
    // Each parameter's dictionaries in the order the parameters are written.
    let dicts =
        names.iter().flat_map(|n| dicts.iter().filter(move |(p, _)| p == n).cloned()).collect::<Vec<(Name, Name)>>();
    Ok((names, dicts))
}

/// Lifetimes only relate references, which lower to plain values.
fn has_type_generics(generics: &syn::Generics) -> bool {
    generics.params.iter().any(|p| !matches!(p, syn::GenericParam::Lifetime(_)))
}

pub(crate) fn lower_vis(vis: &Visibility) -> Vis {
    match vis {
        Visibility::Public(_) => Vis::Pub,
        _ => Vis::Internal,
    }
}

fn item_kind(item: &SynItem) -> &'static str {
    match item {
        SynItem::Static(_) => "static",
        SynItem::Trait(_) | SynItem::TraitAlias(_) => "trait",
        SynItem::Union(_) => "union",
        SynItem::Macro(_) => "macro",
        SynItem::ExternCrate(_) | SynItem::ForeignMod(_) => "extern",
        _ => "other",
    }
}
