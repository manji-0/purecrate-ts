//! Traits (design/01 §7.25): a trait is the type of a record of functions
//! over `Self`, an `impl` adds members to the implementing type's companion,
//! and a bound `T: Tr` is a parameter carrying the record for `T`.

use purecrate_ir::{
    dict_ty, to_camel, Item, Name, Param, Reason, Trait, TraitConst, TraitImpl, TraitMethod, Ty, DICT_SELF, SELF,
};
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::Visibility;

use crate::item::{
    doc, lower_const_parts, lower_fn, lower_param, lower_vis, reject_attrs, snippet, type_params, Bounded, ConstParts,
    Cx, Owner, ParseError, SelfIsOwner,
};
use crate::ty::{lower_type, ParamScope};

/// The type an `impl` is on, when it is a plain name (`X`, `Page<T>`).
pub(crate) fn impl_owner(imp: &syn::ItemImpl) -> Option<String> {
    match &*imp.self_ty {
        syn::Type::Path(p) if p.qself.is_none() && p.path.segments.len() == 1 => {
            Some(p.path.segments[0].ident.to_string())
        }
        _ => None,
    }
}

/// A trait's associated consts.
pub(crate) fn consts_of(t: &syn::ItemTrait) -> impl Iterator<Item = &syn::TraitItemConst> {
    t.items.iter().filter_map(|i| match i {
        syn::TraitItem::Const(c) => Some(c),
        _ => None,
    })
}

/// The names a trait puts on an implementing type's companion: its methods
/// and consts, as TS spells them.
fn members_of(t: &syn::ItemTrait) -> Vec<String> {
    t.items
        .iter()
        .filter_map(|i| match i {
            syn::TraitItem::Fn(f) => Some(to_camel(&f.sig.ident.to_string())),
            syn::TraitItem::Const(c) => Some(to_camel(&c.ident.to_string())),
            _ => None,
        })
        .collect()
}

fn refuse(reason: Reason, message: impl Into<String>, span: proc_macro2::Span) -> ParseError {
    ParseError::new(reason, message).or_at(span)
}

/// `trait Tr: Base { .. }` as its record type, then a function per default
/// body, owned by the trait, its dictionary first (`dict: Tr<Self>`).
pub(crate) fn lower_trait(cx: &mut Cx, t: &syn::ItemTrait) -> Result<Vec<Item>, ParseError> {
    let name = Name::new(t.ident.to_string());
    if let Some(u) = &t.unsafety {
        return Err(refuse(Reason::UnsupportedItem, "an `unsafe trait` is not in v0", u.span).detail("trait"));
    }
    if let Some(a) = &t.auto_token {
        return Err(refuse(Reason::UnsupportedItem, "an `auto trait` is not in v0", a.span).detail("trait"));
    }
    if !t.generics.params.is_empty() {
        return Err(refuse(
            Reason::Generics,
            format!(
                "the trait `{}` has type parameters, which are not in v0 (design/01 §7.25): a trait is over `Self` alone; write one trait per type it converts from, or a function",
                name.as_str()
            ),
            t.generics.span(),
        ));
    }
    if let Some(w) = &t.generics.where_clause {
        return Err(refuse(
            Reason::Generics,
            format!("the `where` clause {} on a trait is not in v0 (design/01 §7.25)", snippet(w)),
            w.span(),
        ));
    }
    let mut supertraits = Vec::new();
    for b in &t.supertraits {
        match b {
            syn::TypeParamBound::Trait(p)
                if matches!(p.modifier, syn::TraitBoundModifier::None)
                    && p.path.get_ident().is_some_and(|i| cx.is_trait(&i.to_string())) =>
            {
                supertraits.push(Name::new(p.path.get_ident().expect("checked by the guard").to_string()));
            }
            other => {
                return Err(refuse(
                    Reason::Generics,
                    format!(
                        "the supertrait {} is not in v0: a trait may build only on the crate's own traits (design/01 §7.25)",
                        snippet(other)
                    ),
                    other.span(),
                ))
            }
        }
    }
    if t.items.is_empty() && supertraits.is_empty() {
        return Err(refuse(
            Reason::UnsupportedItem,
            format!(
                "the trait `{}` has no methods or consts, which is not in v0: its dictionary would hold nothing (design/01 §7.25)",
                name.as_str()
            ),
            t.ident.span(),
        )
        .detail("trait"));
    }
    let self_name = Name::new(SELF);
    let _scope = ParamScope::enter(std::slice::from_ref(&self_name));
    let self_ty = Ty::Param(self_name.clone());
    let mut methods = Vec::new();
    let mut consts = Vec::new();
    let mut defaults = Vec::new();
    for item in &t.items {
        match item {
            syn::TraitItem::Fn(f) => {
                reject_attrs(&f.attrs)?;
                let sig = &f.sig;
                let qualifier = [
                    sig.asyncness.map(|t| ("async", t.span)),
                    sig.unsafety.map(|t| ("unsafe", t.span)),
                    sig.abi.as_ref().map(|a| ("extern abi", a.span())),
                ];
                if let Some((what, span)) = qualifier.into_iter().flatten().next() {
                    return Err(
                        refuse(Reason::FnQualifier, format!("{what} is not allowed in v0"), span).detail(what)
                    );
                }
                if sig.generics.params.iter().any(|p| !matches!(p, syn::GenericParam::Lifetime(_))) {
                    return Err(refuse(
                        Reason::Generics,
                        format!(
                            "the trait method `{}` has type parameters, which are not in v0 (design/01 §7.25): a dictionary holds one function per member",
                            sig.ident
                        ),
                        sig.generics.span(),
                    ));
                }
                if let Some(w) = &sig.generics.where_clause {
                    return Err(refuse(
                        Reason::Generics,
                        format!("the `where` clause {} on a trait method is not in v0 (design/01 §7.25)", snippet(w)),
                        w.span(),
                    ));
                }
                if let Some(syn::FnArg::Receiver(r)) = sig.inputs.first() {
                    if r.colon_token.is_some() {
                        return Err(refuse(
                            Reason::RefReceiver,
                            "a typed receiver (`self: Box<Self>`) is not in v0: take `self` or `&self`",
                            r.span(),
                        ));
                    }
                }
                let mut params = Vec::new();
                for input in &sig.inputs {
                    params.push(lower_param(Some(&self_ty), input).map_err(|e| e.or_at(input.span()))?);
                }
                let ret = match &sig.output {
                    syn::ReturnType::Default => Ty::Prim(purecrate_ir::Prim::Unit),
                    syn::ReturnType::Type(_, ty) => lower_type(ty)?,
                };
                if let Some(block) = &f.default {
                    let owner = Owner::plain(name.clone(), self_ty.clone(), vec![self_name.clone()]);
                    let public = Visibility::Public(Default::default());
                    let mut lowered = lower_fn(cx, Some(owner), sig, &public, block, &f.attrs)
                        .map_err(|e| e.or_at(item.span()))?;
                    lowered.params.insert(0, Param { name: Name::new(DICT_SELF), ty: dict_ty(&name, self_ty.clone()) });
                    defaults.push(Item::Fn(lowered));
                }
                methods.push(TraitMethod {
                    name: Name::new(sig.ident.to_string()),
                    params,
                    ret,
                    default: f.default.is_some(),
                    doc: doc(&f.attrs),
                });
            }
            syn::TraitItem::Const(c) => {
                reject_attrs(&c.attrs)?;
                if !c.generics.params.is_empty() {
                    return Err(refuse(Reason::Generics, "generic consts are not in v0", c.generics.span()));
                }
                consts.push(TraitConst {
                    name: Name::new(c.ident.to_string()),
                    ty: lower_type(&c.ty)?,
                    doc: doc(&c.attrs),
                });
            }
            syn::TraitItem::Type(ty) => {
                return Err(refuse(
                    Reason::UnsupportedItem,
                    format!(
                        "the associated type `{}` of trait `{}` is not in v0 (design/01 §7.25): make it a type parameter of the functions that use it",
                        ty.ident,
                        name.as_str()
                    ),
                    ty.span(),
                )
                .detail("trait"))
            }
            other => {
                return Err(refuse(
                    Reason::UnsupportedItem,
                    format!("only methods and consts in a trait in v0, not {}", snippet(other)),
                    other.span(),
                )
                .detail("trait"))
            }
        }
    }
    let impls = cx
        .trait_impls
        .iter()
        .filter(|(of, _)| of == name.as_str())
        .filter_map(|(_, imp)| {
            let (generics, _) = type_params(cx, &imp.generics, Bounded::Fn).ok()?;
            let _scope = ParamScope::enter(&generics);
            let ty = lower_type(&imp.self_ty).ok()?;
            ty.applied().is_some().then_some(TraitImpl { ty, generics })
        })
        .collect();
    let trait_ = Trait { vis: lower_vis(&t.vis), name, supertraits, methods, consts, impls, doc: doc(&t.attrs) };
    Ok(std::iter::once(Item::Trait(trait_)).chain(defaults).collect())
}

/// `impl Tr for X` of a crate trait: its methods and consts are members of
/// `X`'s companion, public as the trait's are; a const it does not write
/// takes the trait's default. A method it does not write is the trait's
/// default, which the companion prints as a call of it (`emit`).
pub(crate) fn lower_trait_impl(cx: &mut Cx, imp: &syn::ItemImpl, trait_: &syn::Ident) -> Result<Vec<Item>, ParseError> {
    let def = cx.traits[&trait_.to_string()].clone();
    if let Some((Some(bang), _, _)) = &imp.trait_ {
        return Err(refuse(Reason::TraitImpl, "a negative impl `impl !Tr for X` is not in v0", bang.span));
    }
    let (generics, dicts) = type_params(cx, &imp.generics, Bounded::Fn)?;
    let owner = impl_owner(imp);
    let self_ty = &*imp.self_ty;
    match owner.as_deref() {
        Some(o) if generics.iter().any(|g| g.as_str() == o) => {
            return Err(refuse(
                Reason::TraitImpl,
                format!(
                    "the blanket impl `impl<{o}> {trait_} for {o}` is not in v0 (design/01 §7.25): implement `{trait_}` on each type of the crate that needs it"
                ),
                self_ty.span(),
            ))
        }
        Some(o) if cx.is_struct(o) || cx.is_enum(o) => {}
        _ => {
            return Err(refuse(
                Reason::TraitImpl,
                format!(
                    "`impl {trait_} for {}` is not in v0: the type is not a struct or enum of the crate, so there is no companion to put the methods in (design/01 §7.25); wrap it in a struct of the crate",
                    snippet(self_ty).trim_matches('`')
                ),
                self_ty.span(),
            ))
        }
    }
    if let Some((param, bound)) = dicts.first() {
        return Err(refuse(
            Reason::Generics,
            format!(
                "`impl<{p}: {b}> {trait_} for ..` is not in v0 (design/01 §7.25): its dictionary would be a function of `{p}`'s; bound `{p}` on the functions that call `{trait_}`'s methods instead",
                p = param.as_str(),
                b = bound.as_str()
            ),
            imp.generics.span(),
        ));
    }
    let owner_ident = match self_ty {
        syn::Type::Path(p) => p.path.segments[0].ident.clone(),
        _ => unreachable!("an owner is a plain name"),
    };
    let owner = Name::new(owner_ident.to_string());
    collisions(cx, &def, imp, &owner)?;
    let _scope = ParamScope::enter(&generics);
    let owner_ty = lower_type(self_ty)?;
    let public = Visibility::Public(Default::default());
    let mut out = Vec::new();
    let mut written = Vec::new();
    for item in &imp.items {
        let lowered = match item {
            syn::ImplItem::Fn(f) => reject_attrs(&f.attrs).and_then(|()| {
                let mut f = f.clone();
                SelfIsOwner(&owner_ident, Some(self_ty)).visit_impl_item_fn_mut(&mut f);
                let owned = Owner::plain(owner.clone(), owner_ty.clone(), generics.clone());
                lower_fn(cx, Some(owned), &f.sig, &public, &f.block, &f.attrs).map(Item::Fn)
            }),
            syn::ImplItem::Const(c) => reject_attrs(&c.attrs).and_then(|()| {
                written.push(c.ident.to_string());
                let mut c = c.clone();
                SelfIsOwner(&owner_ident, Some(self_ty)).visit_impl_item_const_mut(&mut c);
                let parts =
                    ConstParts { vis: &public, ident: &c.ident, generics: &c.generics, ty: &c.ty, expr: &c.expr };
                lower_const_parts(cx, Some(owner.clone()), parts, &c.attrs, c.span()).map(Item::Const)
            }),
            _ => Err(ParseError::new(Reason::ImplShape, "only methods and consts in impl blocks in v0")),
        };
        out.push(lowered.map_err(|e| e.or_at(item.span()))?);
    }
    // The trait's default for each const the impl does not write, `Self`
    // the implementing type.
    for c in consts_of(&def) {
        let Some((_, default)) = &c.default else { continue };
        if written.contains(&c.ident.to_string()) {
            continue;
        }
        let mut expr = default.clone();
        SelfIsOwner(&owner_ident, Some(self_ty)).visit_expr_mut(&mut expr);
        let mut ty = c.ty.clone();
        SelfIsOwner(&owner_ident, Some(self_ty)).visit_type_mut(&mut ty);
        let parts = ConstParts { vis: &public, ident: &c.ident, generics: &c.generics, ty: &ty, expr: &expr };
        out.push(Item::Const(lower_const_parts(cx, Some(owner.clone()), parts, &c.attrs, imp.span())?));
    }
    Ok(out)
}

/// The companion has one namespace, where Rust resolves a trait's member
/// beside an inherent one, a variant, or another trait's by its own rules:
/// a name the companion already holds is refused rather than emulated.
fn collisions(cx: &Cx, def: &syn::ItemTrait, imp: &syn::ItemImpl, owner: &Name) -> Result<(), ParseError> {
    let o = owner.as_str();
    let trait_ = def.ident.to_string();
    for member in members_of(def) {
        let other = if cx.is_struct(o) && member == "of" {
            Some("the constructor `of` the companion already has".to_string())
        } else if cx.is_variant(o, &member) {
            Some(format!("the variant `{o}::{member}`"))
        } else if cx.inherent.iter().any(|(t, m)| t == o && to_camel(m) == member) {
            Some(format!("a method or const of an inherent `impl {o}`"))
        } else if let Some(what) = cx.std_members.get(&(o.to_string(), member.clone())) {
            Some(what.clone())
        } else {
            cx.trait_impls
                .iter()
                .filter(|(name, other)| *name != trait_ && impl_owner(other).as_deref() == Some(o))
                .find(|(name, _)| members_of(&cx.traits[name]).contains(&member))
                .map(|(name, _)| format!("a member of `impl {name} for {o}`"))
        };
        if let Some(other) = other {
            return Err(refuse(
                Reason::NameCollision,
                format!(
                    "`impl {trait_} for {o}` puts `{member}` on `{o}`'s companion, which already holds {other}; the companion has one member of each name, so rename one (design/01 §7.25)"
                ),
                imp.self_ty.span(),
            ));
        }
    }
    Ok(())
}

/// `impl Tr` in return position: refused, as a value has no tag that the
/// caller could dispatch on.
pub(crate) fn refuse_impl_return(output: &syn::ReturnType) -> Result<(), ParseError> {
    struct Find(Option<proc_macro2::Span>);
    impl VisitMut for Find {
        fn visit_type_mut(&mut self, ty: &mut syn::Type) {
            if let syn::Type::ImplTrait(t) = ty {
                if plain_bounds(&t.bounds) {
                    self.0.get_or_insert(t.span());
                }
            }
            visit_mut::visit_type_mut(self, ty);
        }
    }
    let syn::ReturnType::Type(_, ty) = output else { return Ok(()) };
    let mut find = Find(None);
    find.visit_type_mut(&mut ty.as_ref().clone());
    match find.0 {
        Some(span) => Err(refuse(
            Reason::FnType,
            "`impl Tr` in return position is not in v0: a value has no tag to dispatch on, so the caller could not call the trait's methods on it; return the type itself (design/01 §7.25)",
            span,
        )),
        None => Ok(()),
    }
}

/// Bounds that are each a trait named by one identifier, as a crate
/// trait's, or a lifetime: not `Fn(u32)`.
fn plain_bounds(bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>) -> bool {
    bounds.iter().all(|b| match b {
        syn::TypeParamBound::Trait(t) => t.path.get_ident().is_some(),
        syn::TypeParamBound::Lifetime(_) => true,
        _ => false,
    }) && bounds.iter().any(|b| matches!(b, syn::TypeParamBound::Trait(_)))
}

/// `sig` with each `impl Tr` in a parameter's type made a type parameter of
/// its own, `Impl1`, `Impl2`, .., bounded as written: `fn f(x: impl Tr)` is
/// `fn f<Impl1: Tr>(x: Impl1)`, which Rust's is but for the name.
pub(crate) fn impl_params(cx: &Cx, sig: &syn::Signature) -> syn::Signature {
    struct Lift<'a> {
        cx: &'a Cx,
        taken: Vec<String>,
        found: Vec<(syn::Ident, syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>)>,
    }
    impl VisitMut for Lift<'_> {
        fn visit_type_mut(&mut self, ty: &mut syn::Type) {
            if let syn::Type::ImplTrait(t) = ty {
                if plain_bounds(&t.bounds) {
                    let name = (1..)
                        .map(|n| format!("Impl{n}"))
                        .find(|n| !self.taken.contains(n) && !self.cx.is_struct(n) && !self.cx.is_enum(n))
                        .expect("a free name");
                    self.taken.push(name.clone());
                    let ident = syn::Ident::new(&name, t.span());
                    self.found.push((ident.clone(), t.bounds.clone()));
                    *ty = syn::Type::Path(syn::TypePath { qself: None, path: ident.into() });
                    return;
                }
            }
            visit_mut::visit_type_mut(self, ty);
        }
    }
    let mut sig = sig.clone();
    let taken = sig
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(t) => Some(t.ident.to_string()),
            _ => None,
        })
        .collect();
    let mut lift = Lift { cx, taken, found: Vec::new() };
    for input in sig.inputs.iter_mut() {
        if let syn::FnArg::Typed(p) = input {
            lift.visit_type_mut(&mut p.ty);
        }
    }
    for (ident, bounds) in lift.found {
        sig.generics.params.push(syn::GenericParam::Type(syn::TypeParam {
            attrs: Vec::new(),
            ident,
            colon_token: Some(Default::default()),
            bounds,
            eq_token: None,
            default: None,
        }));
    }
    sig
}
