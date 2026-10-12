//! `#[serde(try_from = "T")]` on `S` needs `impl TryFrom<T> for S`, lowered
//! as the method `S::try_from(T) -> Result<S, E>`. rustc only sees a
//! stand-in serde (`cli::rustc`), so the pairing is checked here, and so is
//! what the real derive checks: a type that derives `Serialize` or
//! `Deserialize` holds only types that derive it too, and no std `Ordering`;
//! and what `rename`, `rename_all`, `tag`, and `content` leave serde unable
//! to read or write (design/04 §3.6).

use purecrate_ir::{Crate, Enum, Field, Item, Name, Reason, Serde, Tagging, Ty, VariantFields};

use crate::Diagnostic;

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    derive_fields(krate, &mut out);
    names_and_tags(krate, &mut out);
    for (i, item) in krate.items.iter().enumerate() {
        let Item::Struct(s) = item else { continue };
        let Some(from) = &s.wire_from else { continue };
        let found = krate.items.iter().any(|other| match other {
            Item::Fn(f) => {
                f.owner.as_ref() == Some(&s.name)
                    && f.name.as_str() == "try_from"
                    && matches!(f.params.as_slice(), [p] if p.ty == *from)
                    && matches!(&f.ret, Ty::Result { ok, .. } if **ok == Ty::Named(s.name.clone()))
            }
            _ => false,
        });
        if !found {
            out.push(
                Diagnostic::at(i, Reason::SerdeAttr, format!(
                    "`#[serde(try_from = \"{}\")]` on `{}` needs `impl TryFrom<{0}> for {1}` returning `Result<{1}, _>`",
                    crate::types::show(from),
                    s.name.as_str()
                ))
                .about(s.name.as_str()),
            );
        }
    }
    out
}

/// A type that derives `Serialize` (or `Deserialize`) has a wire form only
/// when every type it holds has one too; the real derive refuses the rest,
/// but rustc sees the stand-in. serde implements neither for std's
/// `Ordering`, and a crate type has the trait only when it derives it.
fn derive_fields(krate: &Crate, out: &mut Vec<Diagnostic>) {
    for (i, item) in krate.items.iter().enumerate() {
        // `#[serde(try_from = "T")]` reads `T`, not the fields.
        let (serde, tys, read): (Serde, Vec<&Ty>, Option<&Ty>) = match item {
            Item::Struct(s) => (s.serde, s.fields.iter().map(|f| &f.ty).collect(), s.wire_from.as_ref()),
            Item::Enum(e) => (
                e.serde,
                e.variants
                    .iter()
                    .flat_map(|v| match &v.fields {
                        VariantFields::Unit => Vec::new(),
                        VariantFields::Tuple(tys) => tys.iter().collect(),
                        VariantFields::Struct(fs) => fs.iter().map(|f| &f.ty).collect(),
                    })
                    .collect(),
                None,
            ),
            _ => continue,
        };
        let written = names(krate, &tys);
        let read = read.map_or_else(|| written.clone(), |t| names(krate, &[t]));
        let missing = [(serde.ser, "Serialize", &written), (serde.de, "Deserialize", &read)]
            .into_iter()
            .filter(|(on, ..)| *on)
            .find_map(|(_, derive, named)| {
                named.iter().find_map(|n| {
                    krate.items.iter().find_map(|other| match other {
                        Item::Enum(e) if e.name == **n && e.std => Some(format!(
                            "`{}` derives `{derive}` but holds a `std::cmp::Ordering`, which has no serde form (serde implements neither `Serialize` nor `Deserialize` for it); keep `Ordering` in functions, or hold a crate enum",
                            item.name().as_str()
                        )),
                        Item::Struct(_) | Item::Enum(_) if other.name() == *n && !derives(other, derive) => Some(format!(
                            "`{}` derives `{derive}` but holds `{}`, which does not; serde's derive needs it on `{1}` too",
                            item.name().as_str(),
                            n.as_str()
                        )),
                        _ => None,
                    })
                })
            });
        if let Some(message) = missing {
            out.push(Diagnostic::at(i, Reason::SerdeDerive, message).about(item.name().as_str()));
        }
    }
}

fn derives(item: &Item, derive: &str) -> bool {
    let serde = match item {
        Item::Struct(s) => s.serde,
        Item::Enum(e) => e.serde,
        _ => return false,
    };
    if derive == "Serialize" {
        serde.ser
    } else {
        serde.de
    }
}

fn names<'a>(krate: &'a Crate, tys: &[&'a Ty]) -> Vec<&'a Name> {
    let mut out = Vec::new();
    for ty in tys {
        named_in(krate, ty, 0, &mut out);
    }
    out
}

/// The names `ty` holds, looking through aliases (to a fixed depth, as alias
/// cycles are reported elsewhere).
fn named_in<'a>(krate: &'a Crate, ty: &'a Ty, depth: usize, out: &mut Vec<&'a Name>) {
    match ty {
        Ty::Named(n) => match krate.items.iter().find_map(|i| match i {
            Item::Alias(a) if a.name == *n => Some(&a.ty),
            _ => None,
        }) {
            Some(aliased) if depth < 32 => named_in(krate, aliased, depth + 1, out),
            Some(_) => {}
            None => out.push(n),
        },
        ty => ty.children().into_iter().for_each(|t| named_in(krate, t, depth, out)),
    }
}

/// Two fields or variants under one JSON name, which serde reads as one;
/// a tag that is also a field's name, which the derive refuses; and the
/// variants an internally tagged enum cannot write: serde puts the tag in
/// the variant's own object, so a variant must be one (a unit, a struct
/// variant, or a newtype of a struct with named fields).
fn names_and_tags(krate: &Crate, out: &mut Vec<Diagnostic>) {
    for (i, item) in krate.items.iter().enumerate() {
        let message = match item {
            Item::Struct(s) => {
                twice(&s.fields).map(|w| format!("two fields of `{}` are named `{w}` on the wire", s.name.as_str()))
            }
            Item::Enum(e) if !e.std => enum_problem(krate, e),
            _ => None,
        };
        if let Some(message) = message {
            out.push(Diagnostic::at(i, Reason::SerdeAttr, message).about(item.name().as_str()));
        }
    }
}

fn twice(fields: &[Field]) -> Option<&str> {
    fields
        .iter()
        .enumerate()
        .find_map(|(i, f)| fields[..i].iter().any(|g| g.wire_name() == f.wire_name()).then_some(f.wire_name()))
}

fn enum_problem(krate: &Crate, e: &Enum) -> Option<String> {
    let name = e.name.as_str();
    for (i, v) in e.variants.iter().enumerate() {
        if e.variants[..i].iter().any(|w| w.wire_name() == v.wire_name()) {
            return Some(format!("two variants of `{name}` are named `{}` on the wire", v.wire_name()));
        }
        if let VariantFields::Struct(fs) = &v.fields {
            if let Some(w) = twice(fs) {
                return Some(format!("two fields of `{name}::{}` are named `{w}` on the wire", v.name.as_str()));
            }
        }
    }
    match &e.tagging {
        Tagging::External => None,
        Tagging::Adjacent { tag, content } => (tag == content)
            .then(|| format!("`{name}` names its tag and its content both `{tag}`; serde needs two keys")),
        Tagging::Internal { tag } => e.variants.iter().find_map(|v| {
            let var = v.name.as_str();
            let fields: Option<&[Field]> = match &v.fields {
                VariantFields::Unit => None,
                VariantFields::Struct(fs) => Some(fs),
                VariantFields::Tuple(tys) => match tys.as_slice() {
                    [ty] => match tagged_newtype(krate, ty) {
                        Ok(fs) => fs,
                        Err(()) => {
                            return Some(format!(
                                "`{name}::{var}` holds `{}`, which `#[serde(tag = \"{tag}\")]` cannot write: the tag goes into the variant's object, so a newtype variant of an internally tagged enum holds a struct with named fields",
                                crate::types::show(ty)
                            ))
                        }
                    },
                    _ => {
                        return Some(format!(
                            "`{name}::{var}` is a tuple variant, which `#[serde(tag = \"{tag}\")]` cannot write (serde's derive refuses it); name its fields, or use `content` beside `tag`"
                        ))
                    }
                },
            };
            fields.and_then(|fs| fs.iter().find(|f| f.wire_name() == tag)).map(|_| {
                format!("`{name}::{var}` has a field named `{tag}` on the wire, the enum's tag; serde's derive refuses it")
            })
        }),
    }
}

/// The fields a newtype variant of an internally tagged enum writes beside
/// the tag: a crate struct's with named fields (through aliases), or none
/// for `()`; `Err` for anything serde writes as no object.
fn tagged_newtype<'a>(krate: &'a Crate, ty: &'a Ty) -> Result<Option<&'a [Field]>, ()> {
    let mut ty = ty;
    for _ in 0..32 {
        match ty {
            Ty::Prim(purecrate_ir::Prim::Unit) => return Ok(None),
            Ty::Named(n) => match krate.items.iter().find(|i| i.name() == n) {
                Some(Item::Alias(a)) => ty = &a.ty,
                Some(Item::Struct(s)) if s.newtype_inner().is_none() && s.wire_from.is_none() => {
                    return Ok(Some(&s.fields))
                }
                _ => return Err(()),
            },
            _ => return Err(()),
        }
    }
    Err(())
}
