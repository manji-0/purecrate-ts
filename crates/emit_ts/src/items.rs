//! The declarations of a file: struct and enum types with their companion
//! objects, and free functions.

use super::*;

/// `doc` as a JSDoc comment at `indent`, ending in a newline: one line when
/// the text is one line, else one ` * ` line per line. Empty without one.
pub(crate) fn jsdoc(doc: &Option<String>, indent: &str) -> String {
    let Some(text) = doc else { return String::new() };
    // `*/` would end the comment early.
    let text = text.replace("*/", "*\\/");
    if !text.contains('\n') {
        return format!("{indent}/** {text} */\n");
    }
    let mut out = format!("{indent}/**\n");
    for line in text.lines() {
        match line {
            "" => out.push_str(&format!("{indent} *\n")),
            l => out.push_str(&format!("{indent} * {l}\n")),
        }
    }
    out.push_str(&format!("{indent} */\n"));
    out
}

/// The `pub` methods, closing the companion object, then the others as
/// `Ty$name` (see `private_method`).
pub(crate) fn companion_methods(krate: &Crate, ty: &str) -> String {
    let mut out = String::new();
    let (public, private): (Vec<&Fn>, Vec<&Fn>) =
        methods_on(krate, ty).into_iter().partition(|m| m.vis == Vis::Pub);
    for m in public {
        out.push_str(&jsdoc(&m.doc, "  "));
        out.push_str(&format!(
            "  {n}: {impl},\n",
            n = m.name.as_str(),
            impl = fn_arrow(m, 1)
        ));
    }
    out.push_str("} as const;\n");
    for m in private {
        out.push('\n');
        out.push_str(&jsdoc(&m.doc, ""));
        out.push_str(&format!(
            "export const {n} = {impl};\n",
            n = private_method(ty, m.name.as_str()),
            impl = fn_arrow(m, 0)
        ));
    }
    out
}

pub(crate) fn methods_on<'a>(krate: &'a Crate, ty: &str) -> Vec<&'a Fn> {
    krate
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) if f.owner.as_ref().map(|n| n.as_str()) == Some(ty) => Some(f),
            _ => None,
        })
        .collect()
}

pub(crate) fn emit_enum(krate: &Crate, en: &Enum) -> String {
    let name = en.name.as_str();
    let mut out = jsdoc(&en.doc, "");
    out.push_str(&format!("export type {name} =\n"));
    for (i, v) in en.variants.iter().enumerate() {
        let sep = if i + 1 == en.variants.len() {
            ";\n"
        } else {
            "\n"
        };
        out.push_str("  | ");
        out.push_str(&variant_type(v));
        out.push_str(sep);
    }
    out.push('\n');
    out.push_str(&format!("export const {name} = {{\n"));
    for v in &en.variants {
        out.push_str(&jsdoc(&v.doc, "  "));
        out.push_str(&format!("  {}: ", v.name.as_str()));
        out.push_str(&variant_ctor(name, v));
        out.push_str(",\n");
    }
    out.push_str(&companion_methods(krate, name));
    out
}

pub(crate) fn variant_type(v: &purecrate_ir::Variant) -> String {
    let kind = v.name.as_str();
    match &v.fields {
        VariantFields::Unit => format!("Readonly<{{ kind: \"{kind}\" }}>"),
        VariantFields::Tuple(elems) => {
            let inner = elems
                .iter()
                .map(emit_ty)
                .collect::<Vec<_>>()
                .join(", ");
            format!("Readonly<{{ kind: \"{kind}\"; content: readonly [{inner}] }}>")
        }
        VariantFields::Struct(fields) => {
            let mut parts = vec![format!("kind: \"{kind}\"")];
            for f in fields {
                parts.push(format!("{}: {}", f.name.as_str(), emit_ty(&f.ty)));
            }
            format!("Readonly<{{ {} }}>", parts.join("; "))
        }
    }
}

pub(crate) fn variant_ctor(ty: &str, v: &purecrate_ir::Variant) -> String {
    let kind = v.name.as_str();
    match &v.fields {
        VariantFields::Unit => format!("(): {ty} => ({{ kind: \"{kind}\" }})"),
        VariantFields::Tuple(elems) => {
            let params: Vec<String> = elems
                .iter()
                .enumerate()
                .map(|(i, t)| format!("_{i}: {}", emit_ty(t)))
                .collect();
            let args: Vec<String> = (0..elems.len()).map(|i| format!("_{i}")).collect();
            format!(
                "({params}): {ty} => ({{ kind: \"{kind}\", content: [{args}] }})",
                params = params.join(", "),
                args = args.join(", ")
            )
        }
        VariantFields::Struct(fields) => {
            let params: Vec<String> = fields
                .iter()
                .map(|f| format!("{}: {}", ctor_param(f.name.as_str()), emit_ty(&f.ty)))
                .collect();
            let assigns: Vec<String> = fields.iter().map(|f| ctor_assign(f.name.as_str())).collect();
            format!(
                "({params}): {ty} => ({{ kind: \"{kind}\", {assigns} }})",
                params = params.join(", "),
                assigns = assigns.join(", ")
            )
        }
    }
}

pub(crate) fn emit_struct(krate: &Crate, st: &Struct) -> String {
    let name = st.name.as_str();
    let doc = jsdoc(&st.doc, "");
    if let Some(inner) = st.newtype_inner() {
        return doc + &emit_newtype(krate, name, inner, st.closed);
    }
    let fields = st
        .fields
        .iter()
        .map(|f| format!("{}  {}: {};", jsdoc(&f.doc, "  "), f.name.as_str(), emit_ty(&f.ty)))
        .collect::<Vec<_>>()
        .join("\n");
    if st.closed {
        return doc + &emit_closed_struct(krate, st, &fields);
    }
    let mut out = doc;
    out.push_str(&format!("export type {name} = Readonly<{{\n{fields}\n}}>;\n\n"));
    let params = st
        .fields
        .iter()
        .map(|f| format!("{}: {}", ctor_param(f.name.as_str()), emit_ty(&f.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let assigns = st
        .fields
        .iter()
        .map(|f| ctor_assign(f.name.as_str()))
        .collect::<Vec<_>>()
        .join(", ");
    out.push_str(&format!("export const {name} = {{\n"));
    out.push_str(&format!(
        "  of: ({params}): {name} => ({{ {assigns} }}),\n"
    ));
    out.push_str(&companion_methods(krate, name));
    out
}

/// `{ readonly "<crate>.<Type>": true }`. Keyed by string, as the runtime's
/// brands are: two copies of one crate's package (two versions installed)
/// exchange values, the key reads in a hover or a type error, and the crate
/// and type names keep a nested brand (`struct A(Yen)`) apart from its inner.
pub(crate) fn brand(krate: &Crate, name: &str) -> String {
    format!("{{ readonly \"{}.{name}\": true }}", krate.name.as_str())
}

/// A newtype is its inner value at runtime (as in serde's JSON), branded so
/// that `Id` and the bare inner type do not mix.
pub(crate) fn emit_newtype(krate: &Crate, name: &str, inner: &Ty, closed: bool) -> String {
    let inner = emit_ty(inner);
    let mut out = format!("export type {name} = {inner} & {};\n\n", brand(krate, name));
    if closed {
        out.push_str(&closed_ctor_src(name, &format!("value: {inner}"), "value"));
    }
    out.push_str(&format!("export const {name} = {{\n"));
    if !closed {
        out.push_str(&format!("  of: (value: {inner}): {name} => value as {name},\n"));
    }
    out.push_str(&companion_methods(krate, name));
    out
}

/// A struct with a field that is not `pub`: branded, so an object literal is
/// not one, and with no `of` on the companion (design/01 §4).
pub(crate) fn emit_closed_struct(krate: &Crate, st: &Struct, fields: &str) -> String {
    let name = st.name.as_str();
    let shape = st
        .fields
        .iter()
        .map(|f| format!("{}: {}", f.name.as_str(), emit_ty(&f.ty)))
        .collect::<Vec<_>>()
        .join("; ");
    let mut out = format!("export type {name} = Readonly<{{\n{fields}\n}}> & {};\n\n", brand(krate, name));
    out.push_str(&closed_ctor_src(name, &format!("fields: Readonly<{{ {shape} }}>"), "fields"));
    out.push_str(&format!("export const {name} = {{\n"));
    out.push_str(&companion_methods(krate, name));
    out
}

pub(crate) fn closed_ctor_src(name: &str, param: &str, arg: &str) -> String {
    format!(
        "// A field is not `pub` in Rust: outside the crate, `{name}` values come\n\
         // only from the crate's functions. The generated files build them here;\n\
         // `index.ts` does not export it.\n\
         export const {ctor} = ({param}): {name} => {arg} as {name};\n\n",
        ctor = closed_ctor(name)
    )
}

/// Constructor parameter for a field: camelCase, not the JSON key.
fn ctor_param(field: &str) -> String {
    purecrate_ir::to_camel(field)
}

/// `last_error: lastError`, or `color` when the two spellings match.
fn ctor_assign(field: &str) -> String {
    let param = ctor_param(field);
    if param == field {
        field.to_string()
    } else {
        format!("{field}: {param}")
    }
}

pub(crate) fn emit_free_fn(f: &Fn) -> String {
    jsdoc(&f.doc, "") + &format!(
        "export const {name} = {impl};\n",
        name = f.name.as_str(),
        impl = fn_arrow(f, 0)
    )
}
