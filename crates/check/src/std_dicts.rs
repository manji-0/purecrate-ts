//! The dictionaries of std's traits (design/01 §7.25) once the crate is
//! typed: each primitive's that a call passes becomes a const of the crate,
//! `export const ordI32: Ord<I32> = { cmp: Ord.cmp };` in `consts.ts`, as
//! std's `Ordering` is an enum of the crate, and only the ones used; and a
//! dictionary a function only passes on to its own recursive call is read
//! once, so a lint does not ask to drop it.

use std::collections::BTreeMap;

use purecrate_ir::{
    dict_ty, std_dict_name, Callee, ClosureParam, Cmp, Const, Crate, Expr, FmtPiece, FmtSpec, Fn, Item, Lit, Name,
    Prim, Ty, Vis, STD_DICT_TRAITS,
};

/// The one member of the std trait `trait_`'s record for the primitive `of`,
/// typed: `Ord.cmp` or `Ord.cmpStr` for `Ord` and `PartialOrd`; for
/// `Display`, what `format!("{}", x)` prints (the value itself for a
/// string, a `char`, or a `Uuid`, as `to_string` is); for `Default`, the
/// value `Default::default()` gives. `None` where the type has none v0
/// passes: a float's `cmp` or `Display`, a `Uuid`'s `Default`, anything not
/// a primitive.
pub fn member(trait_: &str, of: &Ty) -> Option<Expr> {
    let Ty::Prim(p) = of else { return None };
    let int = p.int();
    match trait_ {
        "Ord" | "PartialOrd" => {
            let by = match p {
                Prim::Bool => Cmp::Num,
                Prim::Char | Prim::String | Prim::Str | Prim::Uuid => Cmp::Text,
                _ if int.is_some() => Cmp::Num,
                _ => return None,
            };
            Some(Expr::Call { callee: Callee::OrdBy(by), args: Vec::new() })
        }
        "Display" => {
            let this = Expr::var("self");
            let body = match p {
                Prim::String | Prim::Str | Prim::Char | Prim::Uuid => this,
                Prim::Bool => format(this),
                _ if int.is_some() => format(this),
                _ => return None,
            };
            let params = vec![ClosureParam { name: Name::new("self"), ty: Some(of.clone()) }];
            Some(Expr::Closure { params, ret: Some(Ty::Prim(Prim::String)), body: Box::new(body) })
        }
        "Default" => {
            let value = match p {
                Prim::Bool => Expr::Lit(Lit::Bool(false)),
                Prim::Char => Expr::Lit(Lit::Char('\0')),
                Prim::String => Expr::Call { callee: Callee::StringNew, args: Vec::new() },
                Prim::Str => Expr::Lit(Lit::Str(String::new())),
                _ if int.is_some() => Expr::Lit(Lit::Int { value: 0, ty: int, byte: false, hex: false }),
                _ if p.float().is_some() => Expr::Lit(Lit::Float { digits: "0.0".into(), ty: p.float() }),
                _ => return None,
            };
            Some(Expr::Closure { params: Vec::new(), ret: Some(of.clone()), body: Box::new(value) })
        }
        _ => None,
    }
}

/// `format!("{}", e)`.
fn format(e: Expr) -> Expr {
    Expr::Call { callee: Callee::Format(vec![FmtPiece::Arg { index: 0, spec: FmtSpec::default() }]), args: vec![e] }
}

/// `krate` with each primitive's std dictionary its calls pass made a
/// public const (`ordI32`), in name order after the crate's other items,
/// and each call reading it by name. A `Vec`'s or a tuple's `Ord`
/// dictionary stays where it is passed: its comparator may call a crate
/// type's `cmp`, which a module's top must not.
pub fn hoist(mut krate: Crate) -> Crate {
    let mut found: BTreeMap<String, (Name, Ty)> = BTreeMap::new();
    for item in &mut krate.items {
        if let Item::Fn(f) = item {
            replace(&mut f.body, &mut found);
        }
    }
    for (name, (trait_, of)) in found {
        let Some(value) = member(trait_.as_str(), &of) else { continue };
        krate.items.push(Item::Const(Const {
            vis: Vis::Pub,
            name: Name::new(name),
            owner: None,
            ty: dict_ty(&trait_, of.clone()),
            value: Expr::Call { callee: Callee::StdDict { trait_, of }, args: vec![value] },
            doc: None,
            comment: Vec::new(),
        }));
    }
    krate
}

/// Each primitive's `StdDict` in `e` as a read of its const, recorded in
/// `found` by name. `&str`'s is `String`'s, one record.
fn replace(e: &mut Expr, found: &mut BTreeMap<String, (Name, Ty)>) {
    if let Expr::Call { callee: Callee::StdDict { trait_, of }, .. } = e {
        if let Some(name) = std_dict_name(trait_.as_str(), of) {
            let of = if *of == Ty::Prim(Prim::Str) { Ty::Prim(Prim::String) } else { of.clone() };
            found.entry(name.clone()).or_insert((trait_.clone(), of));
            *e = Expr::Var(Name::new(name));
            return;
        }
    }
    for c in e.children_mut() {
        replace(c, found);
    }
}

/// `krate` with `void d;` first in each function printed as a binding of
/// its own (a free function, and a method that is not `pub`, `xName`)
/// whose dictionary parameter `d` is read only as the same argument of its
/// own recursive calls (`count(xs, n - 1, ordT)`): oxlint's
/// `only-used-in-recursion` would ask to drop it, and the bound fixes the
/// signature (design/07 §9). A `pub` method is a member of its companion,
/// which the rule does not follow.
pub fn read_forwarded(mut krate: Crate) -> Crate {
    let traits: Vec<String> = krate
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Trait(t) => Some(t.name.as_str().to_string()),
            _ => None,
        })
        .chain(STD_DICT_TRAITS.iter().map(|t| t.to_string()))
        .collect();
    for item in &mut krate.items {
        let Item::Fn(f) = item else { continue };
        if f.owner.is_some() && f.vis == Vis::Pub {
            continue;
        }
        let forwarded: Vec<Name> = f
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(&p.ty, Ty::App { name, .. } if traits.iter().any(|t| t == name.as_str())))
            .filter(|(i, p)| only_forwarded(f, *i, &p.name))
            .map(|(_, p)| p.name.clone())
            .collect();
        for d in forwarded.into_iter().rev() {
            let read = Expr::Call { callee: Callee::Read, args: vec![Expr::Var(d)] };
            let body = std::mem::replace(&mut f.body, Expr::Lit(Lit::Unit));
            f.body = Expr::Seq { first: Box::new(read), then: Box::new(body) };
        }
    }
    krate
}

/// Whether `f`'s parameter `name`, its `index`th, is read, and only as the
/// `index`th argument of a call of `f` itself.
fn only_forwarded(f: &Fn, index: usize, name: &Name) -> bool {
    let (mut reads, mut forwarded) = (0, 0);
    f.body.walk(|e| match e {
        Expr::Var(n) if n == name => reads += 1,
        Expr::Call { callee, args } if itself(f, callee) => {
            forwarded += usize::from(matches!(args.get(index), Some(Expr::Var(n)) if n == name));
        }
        _ => {}
    });
    reads > 0 && reads == forwarded
}

/// Whether `callee` is `f` itself: its name, or its owner's method.
fn itself(f: &Fn, callee: &Callee) -> bool {
    match (callee, &f.owner) {
        (Callee::Fn(g), None) => *g == f.name,
        (Callee::Method { ty, name }, Some(owner)) => ty == owner && *name == f.name,
        _ => false,
    }
}
