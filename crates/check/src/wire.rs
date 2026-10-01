//! `#[serde(try_from = "T")]` on `S` needs `impl TryFrom<T> for S`, lowered
//! as the method `S::try_from(T) -> Result<S, E>`. rustc only sees a
//! stand-in serde (`cli::rustc`), so the pairing is checked here, and so is
//! what the real derive checks: a type that derives `Serialize` or
//! `Deserialize` holds only types that derive it too, and no std `Ordering`.

use purecrate_ir::{Crate, Item, Name, Reason, Serde, Ty, VariantFields};

use crate::Diagnostic;

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    derive_fields(krate, &mut out);
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
        Ty::Option(t) | Ty::Vec(t) | Ty::Ignored { inner: t, .. } => named_in(krate, t, depth, out),
        Ty::Result { ok, err } => {
            named_in(krate, ok, depth, out);
            named_in(krate, err, depth, out);
        }
        Ty::Tuple(ts) => ts.iter().for_each(|t| named_in(krate, t, depth, out)),
        Ty::Fn { params, ret } => {
            params.iter().chain(std::iter::once(&**ret)).for_each(|t| named_in(krate, t, depth, out))
        }
        Ty::Prim(_) | Ty::Never => {}
    }
}
