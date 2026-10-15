//! `#[derive(PartialOrd, Ord)]` on the crate's types (design/01 §7.25):
//! each such struct or enum gets the method `cmp`, written here as the
//! derive's code reads (fields in declaration order, compared until one
//! differs; an enum's variants by discriminant first), before anything else
//! is checked, so it is typed and printed as a hand-written `impl Ord`'s
//! `cmp` is. A type whose fields are not all ordered gets none: ordering it
//! is refused where it is asked for, naming the field.

use std::collections::BTreeSet;

use purecrate_ir::{
    Arm, Callee, ClosureParam, Cmp, Crate, Enum, Expr, Fields, Fn, IntTy, Item, Name, Order, Param, Pattern, Prim, Ty,
    VariantBind, VariantFields, Vis, ORDERING,
};

use crate::defs::{Defs, Peel};
use crate::types::show;
use crate::Diagnostic;

/// `krate` with a `cmp` for each type that derives `Ord` and can have one,
/// and std's `Ordering` (exported) where one is written and the crate does
/// not have it yet. Each added item's index past the input's maps to the
/// type it was made for, in `origin`, so a diagnostic about it points at
/// the derive.
pub fn derive(krate: &Crate) -> (Crate, Vec<usize>, Vec<Diagnostic>) {
    let defs = Defs::of(krate);
    let mut found = Vec::new();
    let mut candidates: BTreeSet<&str> = BTreeSet::new();
    for (i, item) in krate.items.iter().enumerate() {
        let (name, order, generic) = match item {
            Item::Struct(s) => (&s.name, s.order, !s.generics.is_empty()),
            Item::Enum(e) => (&e.name, e.order, !e.generics.is_empty()),
            _ => continue,
        };
        if order != Order::Total || generic {
            continue;
        }
        if defs.methods.contains_key(&(name.as_str(), "cmp")) {
            found.push(
                Diagnostic::at(
                    i,
                    purecrate_ir::Reason::NameCollision,
                    format!(
                        "`{0}` derives `Ord` and has its own method `cmp`: the derived one is `{0}.cmp` in TS, and a companion holds one member of a name; rename the method",
                        name.as_str()
                    ),
                )
                .about(name.as_str()),
            );
            continue;
        }
        candidates.insert(name.as_str());
    }
    if !std_ordering_free(&defs) {
        candidates.clear();
    }
    // A type is ordered while every field is: drop the rest until none
    // changes, so two types may hold each other. Every type counts as seen,
    // so no reason is spelled out here.
    let mut all: Vec<String> = defs.structs.keys().chain(defs.enums.keys()).map(|n| n.to_string()).collect();
    loop {
        let unordered: Vec<&str> = candidates
            .iter()
            .copied()
            .filter(|n| field_unordered(&defs, n, &|m| candidates.contains(m) || written(&defs, m), &mut all).is_some())
            .collect();
        if unordered.is_empty() {
            break;
        }
        for n in unordered {
            candidates.remove(n);
        }
    }
    let mut items = krate.items.clone();
    let mut origin = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        let made = match item {
            Item::Struct(s) if candidates.contains(s.name.as_str()) => {
                let cmps = s.fields.iter().map(|f| field_cmp(field(SELF, &f.name), field(OTHER, &f.name))).collect();
                Some((s.name.clone(), s.vis, chain(cmps)))
            }
            // None where a discriminant does not fold, reported on the enum.
            Item::Enum(e) if candidates.contains(e.name.as_str()) => {
                enum_cmp(&defs, e).map(|body| (e.name.clone(), e.vis, body))
            }
            _ => None,
        };
        let Some((name, vis, body)) = made else { continue };
        let ty = Ty::Named(name.clone());
        items.push(Item::Fn(Fn {
            vis: Vis::Pub,
            name: Name::new("cmp"),
            owner: Some(name),
            generics: Vec::new(),
            params: vec![Param { name: Name::new(SELF), ty: ty.clone() }, Param { name: Name::new(OTHER), ty }],
            ret: Ty::named(ORDERING),
            body,
            doc: None,
            display: false,
        }));
        origin.push(i);
        // A public type's `cmp` gives std's `Ordering` to the crate's callers.
        let public = vis == Vis::Pub;
        match items.iter_mut().find_map(|item| match item {
            Item::Enum(e) if e.std => Some(e),
            _ => None,
        }) {
            Some(e) if public => e.vis = Vis::Pub,
            Some(_) => {}
            None => {
                items.push(Item::Enum(Enum::std_ordering(if public { Vis::Pub } else { Vis::Internal })));
                origin.push(i);
            }
        }
    }
    (Crate::new(krate.name.as_str(), items), origin, found)
}

const SELF: &str = "self";
const OTHER: &str = "other";

/// Whether `Ordering` names std's, or nothing yet: the crate's own enum,
/// struct, or alias of that name would hide it.
fn std_ordering_free(defs: &Defs<'_>) -> bool {
    match defs.enums.get(ORDERING) {
        Some(e) => e.std,
        None => !defs.structs.contains_key(ORDERING) && !defs.aliases.contains_key(ORDERING),
    }
}

/// A `cmp` the crate writes: `impl Ord`'s, or an inherent one beside a
/// hand-written `PartialOrd` (rustc refuses `<` without either). Not beside
/// a derived `PartialOrd`, whose `<` compares field by field, nor a derived
/// `Ord`, which the companion's one `cmp` is.
fn written(defs: &Defs<'_>, name: &str) -> bool {
    let order = defs.structs.get(name).map(|s| s.order).or_else(|| defs.enums.get(name).map(|e| e.order));
    matches!(order, Some(Order::None)) && defs.methods.contains_key(&(name, "cmp"))
}

/// Whether the crate type `name` has a `cmp` that orders it, once `derive`
/// has run: a derived one, or one the crate writes.
pub fn has_cmp(defs: &Defs<'_>, name: &str) -> bool {
    let order = match (defs.structs.get(name), defs.enums.get(name)) {
        (Some(s), _) if s.generics.is_empty() => s.order,
        (_, Some(e)) if e.generics.is_empty() && !e.std => e.order,
        _ => return false,
    };
    order != Order::Partial && defs.methods.contains_key(&(name, "cmp"))
}

/// Why the crate type `name` has no `cmp` (`has_cmp` is false), as the end
/// of a sentence that names it.
pub fn why_unordered(defs: &Defs<'_>, name: &str) -> String {
    why_in(defs, name, &mut Vec::new())
}

/// `why_unordered`, not spelled out again for a type in `seen`, which a
/// recursive type would reach again.
fn why_in(defs: &Defs<'_>, name: &str, seen: &mut Vec<String>) -> String {
    seen.push(name.to_string());
    let (order, generic) = match (defs.structs.get(name), defs.enums.get(name)) {
        (Some(s), _) => (s.order, !s.generics.is_empty()),
        (_, Some(e)) if e.std => return "std's `Ordering` is not ordered in v0".into(),
        (_, Some(e)) => (e.order, !e.generics.is_empty()),
        _ => return format!("`{name}` is not a struct or enum of the crate"),
    };
    match order {
        _ if generic => format!(
            "`{name}` has type parameters, and its `Ord` compares their values, whose `cmp` v0 does not pass yet; order by a key of a known type (`max_by_key`)"
        ),
        Order::Total if !std_ordering_free(defs) => format!(
            "`{name}`'s derived `cmp` gives `std::cmp::Ordering`, which the crate's own `Ordering` hides; rename the crate's type"
        ),
        Order::Total => match field_unordered(defs, name, &|m| has_cmp(defs, m), seen) {
            Some(why) => format!("`{name}` derives `Ord`, but {why}"),
            None => format!("`{name}` derives `Ord`, but it has its own method `cmp`"),
        },
        Order::Partial => format!(
            "`{name}` derives `PartialOrd` without `Ord`, and v0 orders a crate type by its `cmp`: derive `PartialEq, Eq, PartialOrd, Ord` or write `impl Ord` (a type with a float field has no `Ord`, as `f64` has none)"
        ),
        Order::None => format!(
            "`{name}` has no `Ord`: derive `PartialEq, Eq, PartialOrd, Ord`, write `impl Ord`, or use `matches!(x, {name}::Variant)`, a `match`, or compare the parts and chain them with `then`"
        ),
    }
}

/// The first field of `name` (a struct, or a variant of an enum) whose type
/// has no comparator, as "its field `f` is `..`"; `ordered` says which crate
/// types are taken to have a `cmp`.
fn field_unordered(
    defs: &Defs<'_>,
    name: &str,
    ordered: &dyn std::ops::Fn(&str) -> bool,
    seen: &mut Vec<String>,
) -> Option<String> {
    let mut fields: Vec<(String, &Ty)> = Vec::new();
    if let Some(s) = defs.structs.get(name) {
        fields.extend(s.fields.iter().map(|f| (format!("`{}`", f.name.as_str()), &f.ty)));
    }
    if let Some(e) = defs.enums.get(name) {
        for v in &e.variants {
            match &v.fields {
                VariantFields::Unit => {}
                VariantFields::Tuple(tys) => fields
                    .extend(tys.iter().enumerate().map(|(i, t)| (format!("`{}::{}.{i}`", name, v.name.as_str()), t))),
                VariantFields::Struct(fs) => fields.extend(
                    fs.iter().map(|f| (format!("`{}::{} {{ {} }}`", name, v.name.as_str(), f.name.as_str()), &f.ty)),
                ),
            }
        }
    }
    fields.into_iter().find_map(|(field, ty)| {
        comparator_in(defs, ty, ordered, seen).err().map(|why| format!("its field {field} holds {why}"))
    })
}

/// How values of `ty` compare by `cmp`: integers and `bool` by `<`, `char`,
/// strings, and `Uuid` by code point, a crate type by its `cmp` (`ordered`
/// says which have one), and `Option`, `Vec`, tuples, and `Box` of those
/// as std orders them. Else what has no order, for a message.
pub fn comparator(defs: &Defs<'_>, ty: &Ty, ordered: &dyn std::ops::Fn(&str) -> bool) -> Result<Cmp, String> {
    comparator_in(defs, ty, ordered, &mut Vec::new())
}

fn comparator_in(
    defs: &Defs<'_>,
    ty: &Ty,
    ordered: &dyn std::ops::Fn(&str) -> bool,
    seen: &mut Vec<String>,
) -> Result<Cmp, String> {
    let t = defs.expand(ty, Peel::Every);
    let mut each = |t: &Ty| comparator_in(defs, t, ordered, seen);
    match t {
        Ty::Prim(Prim::Bool) => Ok(Cmp::Num),
        Ty::Prim(p) if p.int().is_some() => Ok(Cmp::Num),
        Ty::Prim(Prim::Char | Prim::String | Prim::Str | Prim::Uuid) => Ok(Cmp::Text),
        Ty::Prim(Prim::F32 | Prim::F64) => {
            Err(format!("`{}`: floats are not `Ord` (`NaN` has no place in the order)", show(t)))
        }
        Ty::Option(inner) => Ok(Cmp::Option(Box::new(each(inner)?))),
        Ty::Vec(inner) => Ok(Cmp::Vec(Box::new(each(inner)?))),
        Ty::Tuple(ts) if !ts.is_empty() => Ok(Cmp::Tuple(ts.iter().map(&mut each).collect::<Result<_, _>>()?)),
        Ty::Named(n) if ordered(n.as_str()) => Ok(Cmp::Type(n.clone())),
        Ty::Named(n) if seen.iter().any(|s| s == n.as_str()) => Err(format!("`{}`, which is not ordered", n.as_str())),
        Ty::Named(n) => Err(format!("`{}`, which is not ordered: {}", n.as_str(), why_in(defs, n.as_str(), seen))),
        Ty::Param(p) => Err(format!(
            "`{}`, a type parameter, whose `cmp` v0 does not pass yet; order by a key of a known type (`max_by_key`)",
            p.as_str()
        )),
        other => Err(format!(
            "`{}`, which v0 does not order: it orders integers, `bool`, `char`, strings, `Uuid`, the crate's types with `Ord`, and `Option`, `Vec`, and tuples of them",
            show(other)
        )),
    }
}

/// `self.f` / `other.f`.
fn field(base: &str, name: &Name) -> Expr {
    Expr::Field { base: Box::new(Expr::var(base)), name: name.clone() }
}

/// `a.cmp(&b)`, typed by the field's own comparator.
fn field_cmp(a: Expr, b: Expr) -> Expr {
    Expr::MethodCall { receiver: Box::new(a), name: Name::new("cmp"), args: vec![b] }
}

/// `Ordering::Equal`.
fn equal() -> Expr {
    Expr::Construct { ty: Name::new(ORDERING), variant: Some(Name::new("Equal")), fields: Fields::Unit, base: None }
}

/// `c0.then_with(|| c1.then_with(|| ..))`: each comparison runs only where
/// the ones before it are `Equal`, as the derive's code stops at the first
/// that differs; `Equal` with none.
fn chain(cmps: Vec<Expr>) -> Expr {
    let mut rest = cmps.into_iter().rev();
    let Some(last) = rest.next() else { return equal() };
    rest.fold(last, |then, first| Expr::MethodCall {
        receiver: Box::new(first),
        name: Name::new("then_with"),
        args: vec![Expr::Closure { params: Vec::<ClosureParam>::new(), ret: None, body: Box::new(then) }],
    })
}

/// An enum's `cmp`: the discriminants first (each variant's, explicit or
/// its place in the declaration, read from a table by `kind`), then, where
/// both are one variant, its fields in order. `None` where an explicit
/// discriminant does not fold, which is reported on the enum.
fn enum_cmp(defs: &Defs<'_>, e: &Enum) -> Option<Expr> {
    let explicit = e.variants.iter().any(|v| v.discriminant.is_some());
    let table = if explicit {
        crate::consts::discriminants(defs, e).ok()?
    } else {
        e.variants.iter().enumerate().map(|(i, v)| (v.name.clone(), i as i128)).collect()
    };
    let to = e.repr.unwrap_or(if explicit { IntTy::I64 } else { IntTy::U32 });
    let rank = |side: &str| Expr::Call {
        callee: Callee::Discriminant { to, table: table.clone(), of: e.name.clone() },
        args: vec![Expr::var(side)],
    };
    let ranks = field_cmp(rank(SELF), rank(OTHER));
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
    let mut arms = Vec::new();
    for v in e.variants.iter().filter(|v| !matches!(v.fields, VariantFields::Unit)) {
        let (mine, a) = bind(&v.fields, "a");
        let (theirs, b) = bind(&v.fields, "b");
        let same = chain(a.into_iter().zip(b).map(|(a, b)| field_cmp(a, b)).collect());
        // The other side is the same variant: the discriminants were equal.
        let inner = Expr::Match {
            scrutinee: Box::new(Expr::var(OTHER)),
            arms: vec![Arm::new(variant(&v.name, theirs), same), Arm::new(Pattern::Wildcard, equal())],
        };
        arms.push(Arm::new(variant(&v.name, mine), inner));
    }
    // One variant: both are it.
    let ranks = (e.variants.len() > 1).then_some(ranks);
    if arms.is_empty() {
        return Some(ranks.unwrap_or_else(equal));
    }
    if arms.len() < e.variants.len() {
        arms.push(Arm::new(Pattern::Wildcard, equal()));
    }
    Some(chain(ranks.into_iter().chain([Expr::Match { scrutinee: Box::new(Expr::var(SELF)), arms }]).collect()))
}
