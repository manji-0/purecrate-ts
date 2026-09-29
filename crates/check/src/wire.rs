//! `#[serde(try_from = "T")]` on `S` needs `impl TryFrom<T> for S`, lowered
//! as the method `S::try_from(T) -> Result<S, E>`. rustc only sees a
//! stand-in serde (`cli::rustc`), so the pairing is checked here.

use purecrate_ir::{Crate, Item, Reason, Ty};

use crate::Diagnostic;

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = Vec::new();
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
