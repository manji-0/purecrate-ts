//! The derived `PartialEq` of the crate's types (design/01 §5.1): each
//! struct, and each enum with a variant that holds fields, gets the method
//! `eq`, written here as the derive's code reads (fields in declaration
//! order joined by `&&`, so a field is compared only while those before it
//! are equal; an enum's variant first, then that variant's fields the same
//! way), before anything else is checked, so it is typed and printed as any
//! method is, each field's `==` by its own type. `==` on the type then
//! calls it (`X.eq(a, b)`). One the crate's `==` never reaches is dropped
//! once the crate is typed (`drop_unread`), so a type compared nowhere
//! prints as it did.
//!
//! rustc refuses `==` on a type without `PartialEq`, and `check` refuses a
//! hand-written one (§7.25), so a type `==` reaches derives it. An enum
//! whose variants are all unit has none: its `==` is `a.kind === b.kind`.
//! Nor has a type with its own method `eq` (the companion holds one member
//! of a name), nor one holding a value `Eq.deep` cannot compare either:
//! `==` on those stays `Eq.deep`.

use std::collections::BTreeSet;

use purecrate_ir::{
    Arm, BinOp, Callee, Crate, Enum, Expr, Fn, Item, Lit, Name, Param, Pattern, Prim, Ty, VariantBind, VariantFields,
    Vis,
};

use crate::defs::{Defs, Peel};

const SELF: &str = "self";
const OTHER: &str = "other";

/// `krate` with an `eq` for each struct or enum that can have one, and the
/// index of the item each added one was made for, appended to `origin`.
pub fn derive(krate: Crate, origin: &mut Vec<usize>) -> Crate {
    let defs = Defs::of(&krate);
    let mut made = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        let (name, generics, body) = match item {
            Item::Struct(s) => {
                if !wanted(&defs, s.name.as_str()) {
                    continue;
                }
                let eqs = s
                    .fields
                    .iter()
                    // `()` is always equal to itself.
                    .filter(|f| !unit(&defs, &f.ty))
                    .map(|f| same(field(SELF, &f.name), field(OTHER, &f.name)))
                    .collect();
                (s.name.clone(), &s.generics, all(eqs))
            }
            Item::Enum(e) if !e.std && e.variants.iter().any(|v| v.fields != VariantFields::Unit) => {
                if !wanted(&defs, e.name.as_str()) {
                    continue;
                }
                (e.name.clone(), &e.generics, enum_eq(&defs, e))
            }
            _ => continue,
        };
        let ty = if generics.is_empty() {
            Ty::Named(name.clone())
        } else {
            Ty::App { name: name.clone(), args: generics.iter().cloned().map(Ty::Param).collect() }
        };
        made.push((
            i,
            Item::Fn(Fn {
                vis: Vis::Pub,
                name: Name::new("eq"),
                owner: Some(name),
                generics: generics.clone(),
                params: vec![Param { name: Name::new(SELF), ty: ty.clone() }, Param { name: Name::new(OTHER), ty }],
                ret: Ty::bool(),
                body,
                doc: None,
                display: false,
                derived_eq: true,
            }),
        ));
    }
    let name = krate.name.clone();
    let mut items = krate.items;
    for (i, item) in made {
        items.push(item);
        origin.push(i);
    }
    Crate::new(name.as_str(), items)
}

/// Whether the crate type `name` gets an `eq`: it has no method of that
/// name, and every value it holds compares as Rust's `==` does.
fn wanted(defs: &Defs<'_>, name: &str) -> bool {
    !defs.methods.contains_key(&(name, "eq")) && comparable(defs, &Ty::named(name), &mut Vec::new())
}

/// `types::deep_comparable` before typing: what `Eq.deep` (or a field's
/// own `==`) compares as Rust does. `ParseIntError` and `uuid::Error` are
/// opaque here, and a function is no value to compare.
fn comparable(defs: &Defs<'_>, ty: &Ty, seen: &mut Vec<String>) -> bool {
    match defs.expand(ty, Peel::Every) {
        Ty::Prim(Prim::ParseIntError | Prim::UuidError) => false,
        Ty::Prim(_) | Ty::Param(_) | Ty::Never => true,
        Ty::Option(x) | Ty::Vec(x) | Ty::Set(x) | Ty::Array { elem: x, .. } => comparable(defs, x, seen),
        Ty::Map { key, value } => comparable(defs, key, seen) && comparable(defs, value, seen),
        Ty::Result { ok, err } => comparable(defs, ok, seen) && comparable(defs, err, seen),
        Ty::Tuple(xs) => xs.iter().all(|x| comparable(defs, x, seen)),
        Ty::App { name, args } => {
            args.iter().all(|a| comparable(defs, a, seen)) && comparable(defs, &Ty::Named(name.clone()), seen)
        }
        Ty::Named(n) if seen.iter().any(|s| s == n.as_str()) => true,
        Ty::Named(n) => {
            seen.push(n.as_str().to_string());
            let fields: Vec<&Ty> = if let Some(s) = defs.structs.get(n.as_str()) {
                s.fields.iter().map(|f| &f.ty).collect()
            } else if let Some(e) = defs.enums.get(n.as_str()) {
                e.variants
                    .iter()
                    .flat_map(|v| match &v.fields {
                        VariantFields::Unit => Vec::new(),
                        VariantFields::Tuple(ts) => ts.iter().collect(),
                        VariantFields::Struct(fs) => fs.iter().map(|f| &f.ty).collect(),
                    })
                    .collect()
            } else {
                return false;
            };
            fields.into_iter().all(|f| comparable(defs, f, seen))
        }
        Ty::Ignored { inner, .. } => comparable(defs, inner, seen),
        Ty::Fn { .. } => false,
    }
}

/// Whether `ty` is `()`, which has one value.
fn unit(defs: &Defs<'_>, ty: &Ty) -> bool {
    matches!(defs.expand(ty, Peel::Every), Ty::Prim(Prim::Unit))
}

/// `self.f` / `other.f`.
fn field(base: &str, name: &Name) -> Expr {
    Expr::Field { base: Box::new(Expr::var(base)), name: name.clone() }
}

/// `a == b`, typed by the field's own type.
fn same(a: Expr, b: Expr) -> Expr {
    Expr::Binary { op: BinOp::Eq, left: Box::new(a), right: Box::new(b) }
}

/// `e0 && e1 && ..`: each runs only where those before it hold, as the
/// derive's code stops at the first field that differs; `true` with none.
fn all(eqs: Vec<Expr>) -> Expr {
    eqs.into_iter()
        .reduce(|a, b| Expr::Binary { op: BinOp::And, left: Box::new(a), right: Box::new(b) })
        .unwrap_or(Expr::Lit(Lit::Bool(true)))
}

/// An enum's `eq`: a `match` on `self`, each arm testing that `other` is
/// the same variant (`other == E::V` for a unit one, which prints as its
/// `kind`), then its fields in order.
fn enum_eq(defs: &Defs<'_>, e: &Enum) -> Expr {
    let bind = |fields: &VariantFields, side: &str| -> (VariantBind, Vec<Expr>) {
        let name = |i: usize| Name::new(format!("{side}{i}"));
        match fields {
            VariantFields::Unit => (VariantBind::Unit, Vec::new()),
            VariantFields::Tuple(tys) => (
                VariantBind::Tuple((0..tys.len()).map(|i| Pattern::Var(name(i))).collect()),
                (0..tys.len()).map(|i| Expr::Var(name(i))).collect(),
            ),
            VariantFields::Struct(fs) => (
                VariantBind::Struct(
                    fs.iter().enumerate().map(|(i, f)| (f.name.clone(), Pattern::Var(name(i)))).collect(),
                ),
                (0..fs.len()).map(|i| Expr::Var(name(i))).collect(),
            ),
        }
    };
    let variant = |v: &Name, bind: VariantBind| Pattern::Variant { ty: e.name.clone(), variant: v.clone(), bind };
    let arms = e
        .variants
        .iter()
        .map(|v| {
            let (mine, a) = bind(&v.fields, "a");
            let body = if a.is_empty() {
                let unit = Expr::Construct {
                    ty: e.name.clone(),
                    variant: Some(v.name.clone()),
                    fields: purecrate_ir::Fields::Unit,
                    base: None,
                };
                same(Expr::var(OTHER), unit)
            } else {
                let (theirs, b) = bind(&v.fields, "b");
                let tys: Vec<&Ty> = match &v.fields {
                    VariantFields::Unit => Vec::new(),
                    VariantFields::Tuple(ts) => ts.iter().collect(),
                    VariantFields::Struct(fs) => fs.iter().map(|f| &f.ty).collect(),
                };
                let fields = all(a
                    .into_iter()
                    .zip(b)
                    .zip(tys)
                    .filter(|(_, t)| !unit(defs, t))
                    .map(|((a, b), _)| same(a, b))
                    .collect());
                Expr::Match {
                    scrutinee: Box::new(Expr::var(OTHER)),
                    arms: vec![
                        Arm::new(variant(&v.name, theirs), fields),
                        Arm::new(Pattern::Wildcard, Expr::Lit(Lit::Bool(false))),
                    ],
                }
            };
            Arm::new(variant(&v.name, mine), body)
        })
        .collect();
    Expr::Match { scrutinee: Box::new(Expr::var(SELF)), arms }
}

/// `krate` without the derived `eq`s that nothing reaches: from every other
/// function, a call `X.eq(..)` or a comparer calling it, and from those, the
/// calls they make in turn.
pub fn drop_unread(mut krate: Crate) -> Crate {
    let derived = |item: &Item| matches!(item, Item::Fn(f) if f.derived_eq);
    let calls = |body: &Expr, out: &mut BTreeSet<String>| {
        body.walk(|e| {
            match e {
                Expr::Call { callee: Callee::Method { ty, name }, .. } if name.as_str() == "eq" => {
                    out.insert(ty.as_str().to_string());
                }
                // `Eq.option(X.eq)(a, b)`: each `eq` the comparer calls.
                Expr::Call { callee: Callee::EqBy(by), .. } => {
                    out.extend(by.types().into_iter().map(|t| t.as_str().to_string()));
                }
                _ => {}
            }
        });
    };
    let mut read = BTreeSet::new();
    for item in &krate.items {
        if let Item::Fn(f) = item {
            if !f.derived_eq {
                calls(&f.body, &mut read);
            }
        }
    }
    loop {
        let mut more = read.clone();
        for item in &krate.items {
            if let Item::Fn(f) = item {
                if f.derived_eq && f.owner.as_ref().is_some_and(|o| read.contains(o.as_str())) {
                    calls(&f.body, &mut more);
                }
            }
        }
        if more == read {
            break;
        }
        read = more;
    }
    krate.items.retain(|item| {
        !derived(item) || matches!(item, Item::Fn(f) if f.owner.as_ref().is_some_and(|o| read.contains(o.as_str())))
    });
    krate
}
