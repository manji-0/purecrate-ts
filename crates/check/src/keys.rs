//! The keys of a `BTreeMap` and a `BTreeSet` (design/01 §7.19, §7.26): a
//! `String`, or another type whose values are JS primitives, which a `Map`
//! finds by value (SameValueZero), sorted by the key type's `cmp`.

use purecrate_ir::{Cmp, Order, Prim, Ty};

use crate::defs::{Defs, Peel};
use crate::types::show;

/// How a key type is read in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// `String`: §7.19's `BTree.get`, `BTree.keys`, .. by code point.
    String,
    /// Another key, sorted by this comparator, `Cmp::Num` (an integer or
    /// `bool`, by JS `<`) or `Cmp::Text` (a `char` or a `Uuid`, by code
    /// point); a newtype's is its content's, as its derived `Ord` is.
    By(Cmp),
}

/// How `ty`'s values order as keys, or why `ty` is no key, as the end of a
/// sentence that names the map or set.
pub fn key(defs: &Defs<'_>, ty: &Ty) -> Result<Key, String> {
    let t = defs.expand(ty, Peel::Every);
    match prim(t) {
        Some(Ok(k)) => return Ok(k),
        Some(Err(why)) => return Err(why),
        None => {}
    }
    let object = "a JS `Map` compares an object key by identity, so `get` with an equal but distinct value would miss where Rust's `BTreeMap` finds it; key by an integer, `char`, `bool`, `Uuid`, `String`, or a newtype of one";
    match t {
        Ty::Named(n) => {
            if defs.enums.contains_key(n.as_str()) {
                return Err(format!("the key `{}` is an enum, an object in TS (`{{ kind }}`): {object}", n.as_str()));
            }
            let Some(s) = defs.structs.get(n.as_str()) else {
                return Err(format!("the key `{}` is not a type of the crate v0 orders", n.as_str()));
            };
            let Some(inner) = s.newtype_inner() else {
                return Err(format!(
                    "the key `{}` is a struct with named fields or more than one, an object in TS: {object}",
                    n.as_str()
                ));
            };
            let content = match prim(defs.expand(inner, Peel::Every)) {
                Some(Ok(Key::String)) => Key::By(Cmp::Text),
                Some(Ok(k)) => k,
                _ => {
                    return Err(format!(
                        "the key `{}` is a newtype of `{}`, which is no key: {object}",
                        n.as_str(),
                        show(inner)
                    ))
                }
            };
            match s.order {
                Order::Total => Ok(content),
                // `order::written`: an `impl Ord` by hand.
                Order::None if defs.methods.contains_key(&(n.as_str(), "cmp")) => Err(format!(
                    "the key `{}` has a hand-written `Ord`, and the output sorts a newtype key by its content; derive `PartialEq, Eq, PartialOrd, Ord`",
                    n.as_str()
                )),
                _ => Err(format!(
                    "the key `{}` has no derived `Ord`, which a key's order is; derive `PartialEq, Eq, PartialOrd, Ord`",
                    n.as_str()
                )),
            }
        }
        Ty::Param(p) => {
            Err(format!("the key is the type parameter `{}`, which may be an object in TS: {object}", p.as_str()))
        }
        Ty::App { name, .. } => Err(format!(
            "the key `{}` is a generic type, which v0 does not take as a key; key by a newtype without type parameters",
            name.as_str()
        )),
        other => Err(format!("the key `{}` is an object in TS: {object}", show(other))),
    }
}

/// `key` for a primitive: `None` for any other type.
fn prim(t: &Ty) -> Option<Result<Key, String>> {
    let Ty::Prim(p) = t else { return None };
    Some(match p {
        Prim::String => Ok(Key::String),
        Prim::Bool => Ok(Key::By(Cmp::Num)),
        p if p.int().is_some() => Ok(Key::By(Cmp::Num)),
        Prim::Char | Prim::Uuid => Ok(Key::By(Cmp::Text)),
        Prim::F32 | Prim::F64 => {
            Err(format!("the key `{}` is a float, which Rust does not order (`f64` has no `Ord`)", show(t)))
        }
        Prim::Str => Err("the key `&str` borrows; a map or set owns its keys: use `String`".to_string()),
        _ => Err(format!("the key `{}` is no key in v0", show(t))),
    })
}
