//! The members a type's companion takes from the traits it implements
//! (design/01 §7.25): each default the `impl` does not write, as a call of
//! the trait's body with the companion as its dictionary,
//! `doubled: (self: Item): I64 => Priced.doubled(Item, self)`.

use purecrate_ir::{Callee, Crate, Expr, Fn, Item, Name, Param, Vis, SELF};

/// `krate` with a member for each default an `impl` takes, after the
/// crate's other items. Its explicit return type keeps the member's
/// reference to its own companion legal in TS (TS7022 otherwise).
pub fn forward(mut krate: Crate) -> Crate {
    let mut extra = Vec::new();
    for item in &krate.items {
        let Item::Trait(t) = item else { continue };
        for imp in &t.impls {
            let owner = imp.owner();
            for m in t.methods.iter().filter(|m| m.default) {
                let method = |o: &Name| {
                    krate.items.iter().find_map(|i| match i {
                        Item::Fn(f) if f.owner.as_ref() == Some(o) && f.name == m.name => Some(f),
                        _ => None,
                    })
                };
                if method(owner).is_some() {
                    continue;
                }
                let Some(default) = method(&t.name) else { continue };
                let binds = [(Name::new(SELF), imp.ty.clone())];
                // The default's first parameter is its dictionary.
                let params: Vec<Param> = default.params[1..]
                    .iter()
                    .map(|p| Param { name: p.name.clone(), ty: p.ty.subst(&binds) })
                    .collect();
                let args = std::iter::once(Expr::Var(owner.clone()))
                    .chain(params.iter().map(|p| Expr::Var(p.name.clone())))
                    .collect();
                extra.push(Item::Fn(Fn {
                    vis: Vis::Pub,
                    name: m.name.clone(),
                    owner: Some(owner.clone()),
                    generics: imp.generics.clone(),
                    params,
                    ret: default.ret.subst(&binds),
                    body: Expr::Call { callee: Callee::Method { ty: t.name.clone(), name: m.name.clone() }, args },
                    doc: None,
                    display: false,
                }));
            }
        }
    }
    krate.items.extend(extra);
    krate
}
