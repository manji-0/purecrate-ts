//! `#[serde(try_from = "T")]` on `S` needs `impl TryFrom<T> for S`, lowered
//! as the method `S::try_from(T) -> Result<S, E>`. rustc only sees a
//! stand-in serde (`cli::rustc`), so the pairing is checked here, and so is
//! a field of std's `Ordering`, which serde gives no form.

use purecrate_ir::{Crate, Item, Reason, Ty, VariantFields};

use crate::Diagnostic;

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    ordering_fields(krate, &mut out);
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

/// Every struct and enum has a wire schema (design/04), and serde implements
/// neither `Serialize` nor `Deserialize` for `std::cmp::Ordering`: a field
/// that holds one, through an alias or a container too, is refused.
fn ordering_fields(krate: &Crate, out: &mut Vec<Diagnostic>) {
    let Some(ordering) = krate.items.iter().find_map(|i| match i {
        Item::Enum(e) if e.std => Some(&e.name),
        _ => None,
    }) else {
        return;
    };
    let holds = |ty: &Ty| holds_named(krate, ty, ordering, 0);
    for (i, item) in krate.items.iter().enumerate() {
        let tys: Vec<&Ty> = match item {
            Item::Struct(s) => s.fields.iter().map(|f| &f.ty).collect(),
            Item::Enum(e) => e
                .variants
                .iter()
                .flat_map(|v| match &v.fields {
                    VariantFields::Unit => Vec::new(),
                    VariantFields::Tuple(tys) => tys.iter().collect(),
                    VariantFields::Struct(fs) => fs.iter().map(|f| &f.ty).collect(),
                })
                .collect(),
            _ => continue,
        };
        if tys.into_iter().any(holds) {
            out.push(
                Diagnostic::at(i, Reason::SerdeAttr, format!(
                    "`{}` holds a `std::cmp::Ordering`, which has no serde form (serde implements neither `Serialize` nor `Deserialize` for it), and every struct and enum has a wire schema; keep `Ordering` in functions, or hold a crate enum",
                    item.name().as_str()
                ))
                .about(item.name().as_str()),
            );
        }
    }
}

/// `ty` names `name`, looking through aliases (to a fixed depth, as alias
/// cycles are reported elsewhere).
fn holds_named(krate: &Crate, ty: &Ty, name: &purecrate_ir::Name, depth: usize) -> bool {
    match ty {
        Ty::Named(n) if n == name => true,
        Ty::Named(n) => depth < 32 && krate.items.iter().any(|i| matches!(i, Item::Alias(a) if a.name == *n && holds_named(krate, &a.ty, name, depth + 1))),
        Ty::Option(t) | Ty::Vec(t) | Ty::Ignored { inner: t, .. } => holds_named(krate, t, name, depth),
        Ty::Result { ok, err } => holds_named(krate, ok, name, depth) || holds_named(krate, err, name, depth),
        Ty::Tuple(ts) => ts.iter().any(|t| holds_named(krate, t, name, depth)),
        Ty::Fn { params, ret } => params.iter().chain(std::iter::once(&**ret)).any(|t| holds_named(krate, t, name, depth)),
        Ty::Prim(_) | Ty::Never => false,
    }
}
