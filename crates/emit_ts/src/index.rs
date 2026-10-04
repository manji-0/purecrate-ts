//! `index.ts`: the package's entry, re-exporting each item from its file.

use super::*;

/// Some public type, field, or signature satisfies `pred`.
fn surface_ty(krate: &Crate, pred: impl std::ops::Fn(&Ty) -> bool) -> bool {
    krate.items.iter().filter(|i| i.vis() == Vis::Pub).any(|item| match item {
        Item::Struct(s) => s.fields.iter().any(|f| f.ty.any(&pred)),
        Item::Enum(e) => e.variants.iter().any(|v| match &v.fields {
            VariantFields::Unit => false,
            VariantFields::Tuple(ts) => ts.iter().any(|t| t.any(&pred)),
            VariantFields::Struct(fs) => fs.iter().any(|f| f.ty.any(&pred)),
        }),
        Item::Alias(a) => a.ty.any(&pred),
        Item::Const(c) => c.ty.any(&pred),
        Item::Fn(f) => f.params.iter().any(|p| p.ty.any(&pred)) || f.ret.any(&pred),
    })
}

/// Some public type, field, or signature holds `prim`.
fn surface_holds(krate: &Crate, prim: Prim) -> bool {
    surface_ty(krate, |ty| matches!(ty, Ty::Prim(p) if *p == prim))
}

pub(super) fn emit_index(krate: &Crate) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    // A single value export carries both the companion and its same-named
    // type. `pack` points `"purecrate"` at the package's copy of the runtime.
    // Runtime names go out when the public surface holds them, so two packages
    // re-exported from one barrel do not collide on unused `I8` / `Result`.
    // `pack` adds `parseJson` with a schema.
    let mut runtime = Vec::new();
    if surface_ty(krate, |ty| matches!(ty, Ty::Result { .. })) {
        runtime.push("Result");
    }
    runtime.extend(["Panic", "assertNever"]);
    // `Int` makes the values of the brands the surface holds; without one,
    // the runtime's copy has no `Int` to export.
    let numeric = IntTy::ALL.into_iter().any(|t| surface_holds(krate, Prim::from(t)))
        || [FloatTy::F32, FloatTy::F64].into_iter().any(|t| surface_holds(krate, Prim::from(t)));
    if numeric {
        runtime.push("Int");
    }
    if surface_holds(krate, Prim::Char) {
        runtime.push("Char");
    }
    if surface_holds(krate, Prim::Uuid) || surface_holds(krate, Prim::UuidError) {
        runtime.extend(["Uuid", "type UuidError"]);
    }
    if surface_holds(krate, Prim::ParseIntError) {
        runtime.push("type ParseIntError");
    }
    out.push_str(&format!("export {{ {} }} from \"purecrate\";\n", runtime.join(", ")));
    let mut types: Vec<&str> =
        IntTy::ALL.into_iter().filter(|t| surface_holds(krate, Prim::from(*t))).map(IntTy::ts_name).collect();
    for t in [FloatTy::F32, FloatTy::F64] {
        if surface_holds(krate, Prim::from(t)) {
            types.push(t.ts_name());
        }
    }
    if !types.is_empty() {
        out.push_str(&format!("export type {{ {} }} from \"purecrate\";\n", types.join(", ")));
    }
    for item in krate.exported() {
        match item {
            Item::Fn(f) if f.owner.is_some() => {}
            Item::Alias(al) => {
                out.push_str(&format!(
                    "export type {{ {name} }} from \"./{stem}.ts\";\n",
                    name = al.name.as_str(),
                    stem = item.file_stem(),
                ));
            }
            other => {
                out.push_str(&format!(
                    "export {{ {name} }} from \"./{stem}.ts\";\n",
                    name = other.name().as_str(),
                    stem = other.file_stem(),
                ));
            }
        }
    }
    out
}
