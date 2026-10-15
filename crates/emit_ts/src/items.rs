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

/// `export const NAME: T = value;\n` for a const's folded value: no
/// annotation where the value is cast to the type already.
pub(crate) fn const_decl(name: &str, c: &purecrate_ir::Const) -> String {
    let (value, ty) = const_value(c);
    let annotation =
        if crate::tidy::cast_type(&value) == Some(ty.as_str()) { String::new() } else { format!(": {ty}") };
    format!("export const {name}{annotation} = {value};\n")
}

/// A const's folded value as printed, and its type.
fn const_value(c: &purecrate_ir::Const) -> (String, String) {
    let value = emit_expr(&c.value, 0);
    (crate::tidy::strip_outer(&value).to_string(), emit_ty(&c.ty))
}

/// The `pub` associated consts and methods, closing the companion object,
/// then the others as `tyName`, marked `@internal` (see `private_method`).
/// A const member is cast to its type where the value is not already, as
/// `as const` would give it the literal's type.
pub(crate) fn companion_methods(krate: &Crate, ty: &str) -> String {
    let mut out = String::new();
    let (public_consts, private_consts): (Vec<&purecrate_ir::Const>, Vec<&purecrate_ir::Const>) =
        consts_on(krate, ty).into_iter().partition(|c| c.vis == Vis::Pub);
    for c in public_consts {
        let (value, t) = const_value(c);
        let value = if crate::tidy::cast_type(&value) == Some(t.as_str()) { value } else { format!("{value} as {t}") };
        out.push_str(&jsdoc(&c.doc, "  "));
        out.push_str(&format!("  {}: {value},\n", c.name.as_str()));
    }
    let (public, private): (Vec<&Fn>, Vec<&Fn>) = methods_on(krate, ty).into_iter().partition(|m| m.vis == Vis::Pub);
    for m in public {
        out.push_str(&jsdoc(&m.doc, "  "));
        out.push_str(&format!(
            "  {n}: {impl},\n",
            n = m.name.as_str(),
            impl = fn_arrow(m, 1)
        ));
    }
    out.push_str("} as const;\n");
    for c in private_consts {
        out.push('\n');
        out.push_str(&jsdoc(&internal_doc(&c.doc), ""));
        out.push_str(&const_decl(&private_method(ty, c.name.as_str()), c));
    }
    for m in private {
        out.push('\n');
        out.push_str(&jsdoc(&internal_doc(&m.doc), ""));
        out.push_str(&format!(
            "export const {n} = {impl};\n",
            n = private_method(ty, m.name.as_str()),
            impl = fn_arrow(m, 0)
        ));
    }
    out
}

/// Not `pub` in Rust: for the generated files only.
fn internal_doc(doc: &Option<String>) -> Option<String> {
    Some(match doc {
        Some(d) => format!("{d}\n@internal"),
        None => "@internal".to_string(),
    })
}

/// The associated consts of `ty`, in declaration order.
pub(crate) fn consts_on<'a>(krate: &'a Crate, ty: &str) -> Vec<&'a purecrate_ir::Const> {
    krate
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(c) if c.owner.as_ref().map(|n| n.as_str()) == Some(ty) => Some(c),
            _ => None,
        })
        .collect()
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

/// `<A, B>`, or nothing where there are no names.
fn angled<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    let names: Vec<&str> = names.into_iter().collect();
    if names.is_empty() {
        String::new()
    } else {
        format!("<{}>", names.join(", "))
    }
}

/// `Page<T>` where a generic type is declared, `Page` otherwise.
fn declared(name: &str, generics: &[Name]) -> String {
    format!("{name}{}", angled(generics.iter().map(Name::as_str)))
}

/// A constructor's type parameters and the type it gives: the parameters
/// its fields name, the others `never` (`Tree.Leaf(): Tree<never>`, which
/// any `Tree<T>` takes, its fields being read only).
fn ctor_head(name: &str, generics: &[Name], fields: &[&Ty]) -> (String, String) {
    let used: Vec<&Name> =
        generics.iter().filter(|g| fields.iter().any(|t| t.any(&|x| matches!(x, Ty::Param(p) if p == *g)))).collect();
    let args = generics.iter().map(|g| if used.contains(&g) { g.as_str() } else { "never" });
    (angled(used.iter().map(|g| g.as_str())), format!("{name}{}", angled(args)))
}

/// A struct constructor's parameters (`lastError: I32`) and the fields of
/// the object it builds (`last_error: lastError`).
fn ctor_fields(fields: &[purecrate_ir::Field]) -> (String, String) {
    let params =
        fields.iter().map(|f| format!("{}: {}", ctor_param(f.name.as_str()), emit_ty(&f.ty))).collect::<Vec<_>>();
    let assigns = fields.iter().map(|f| ctor_assign(f.name.as_str())).collect::<Vec<_>>();
    (params.join(", "), assigns.join(", "))
}

pub(crate) fn emit_enum(krate: &Crate, en: &Enum) -> String {
    let name = en.name.as_str();
    let typed = declared(name, &en.generics);
    let mut out = jsdoc(&en.doc, "");
    // On one line where it fits, else a variant per line, as oxfmt prints it.
    let members: Vec<String> = en.variants.iter().map(variant_type).collect();
    let one = format!("export type {typed} = {};", members.join(" | "));
    if one.len() <= crate::WIDTH {
        out.push_str(&one);
        out.push('\n');
    } else {
        out.push_str(&format!("export type {typed} ="));
        for m in &members {
            out.push_str(&format!("\n  | {m}"));
        }
        out.push_str(";\n");
    }
    out.push('\n');
    out.push_str(&format!("export const {name} = {{\n"));
    for v in &en.variants {
        out.push_str(&jsdoc(&v.doc, "  "));
        out.push_str(&format!("  {}: ", v.name.as_str()));
        out.push_str(&variant_ctor(name, &en.generics, v));
        out.push_str(",\n");
    }
    out.push_str(&companion_methods(krate, name));
    out
}

pub(crate) fn variant_type(v: &purecrate_ir::Variant) -> String {
    let kind = v.name.as_str();
    match &v.fields {
        VariantFields::Unit => format!("Readonly<{{ kind: \"{kind}\" }}>"),
        VariantFields::Tuple(elems) if elems.len() == 1 => {
            format!("Readonly<{{ kind: \"{kind}\"; value: {} }}>", emit_ty(&elems[0]))
        }
        VariantFields::Tuple(elems) => {
            let inner = elems.iter().map(emit_ty).collect::<Vec<_>>().join(", ");
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

pub(crate) fn variant_ctor(name: &str, generics: &[Name], v: &purecrate_ir::Variant) -> String {
    let kind = v.name.as_str();
    let tys: Vec<&Ty> = match &v.fields {
        VariantFields::Unit => Vec::new(),
        VariantFields::Tuple(ts) => ts.iter().collect(),
        VariantFields::Struct(fs) => fs.iter().map(|f| &f.ty).collect(),
    };
    let (head, ty) = ctor_head(name, generics, &tys);
    let ty = ty.as_str();
    let ctor = match &v.fields {
        VariantFields::Unit => format!("(): {ty} => ({{ kind: \"{kind}\" }})"),
        VariantFields::Tuple(elems) if elems.len() == 1 => {
            format!("(value: {}): {ty} => ({{ kind: \"{kind}\", value }})", emit_ty(&elems[0]))
        }
        VariantFields::Tuple(elems) => {
            let params: Vec<String> = elems.iter().enumerate().map(|(i, t)| format!("_{i}: {}", emit_ty(t))).collect();
            let args: Vec<String> = (0..elems.len()).map(|i| format!("_{i}")).collect();
            format!(
                "({params}): {ty} => ({{ kind: \"{kind}\", content: [{args}] }})",
                params = params.join(", "),
                args = args.join(", ")
            )
        }
        VariantFields::Struct(fields) => {
            let (params, assigns) = ctor_fields(fields);
            format!("({params}): {ty} => ({{ kind: \"{kind}\", {assigns} }})")
        }
    };
    format!("{head}{ctor}")
}

pub(crate) fn emit_struct(krate: &Crate, st: &Struct) -> String {
    let name = st.name.as_str();
    let doc = jsdoc(&st.doc, "");
    if let Some(inner) = st.newtype_inner() {
        return doc + &emit_newtype(krate, st, inner);
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
    // A struct with no fields is an object with none: `{}` as a type would
    // take any value but `null` (and lint refuses it).
    if fields.trim().is_empty() {
        out.push_str(&format!("export type {} = Readonly<Record<string, never>>;\n\n", declared(name, &st.generics)));
    } else {
        out.push_str(&format!("export type {} = Readonly<{{\n{fields}\n}}>;\n\n", declared(name, &st.generics)));
    }
    let (params, assigns) = ctor_fields(&st.fields);
    out.push_str(&format!("export const {name} = {{\n"));
    let object = if assigns.trim().is_empty() { "{}".to_string() } else { format!("{{ {assigns} }}") };
    let (head, ty) = ctor_head(name, &st.generics, &st.fields.iter().map(|f| &f.ty).collect::<Vec<_>>());
    out.push_str(&format!("  of: {head}({params}): {ty} => ({object}),\n"));
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

/// A closed type's brand: keyed by a `unique symbol` its own file declares
/// and does not export, so no code outside the package can write the key,
/// and an object literal with the type's fields is not one (a string key
/// could be written out). Two copies of the package do not exchange closed
/// values: each has its own symbol, as each may hold its own invariants.
/// The declaration, then the brand.
fn closed_brand(name: &str) -> (String, String) {
    (format!("declare const {name}$brand: unique symbol;\n"), format!("{{ readonly [{name}$brand]: true }}"))
}

/// A closed struct's brand: an instance of a class its own file declares,
/// with a `private` member, which no object but one the package casts can
/// have. A symbol-keyed brand is a property, which object spread copies
/// (`{ ...line, qty: 0 }` would be a `Line` with no cast), where Rust refuses
/// the same update outside the module (E0451); TS leaves a class's private
/// members out of a spread's type. The class exists only in the types
/// (a `#brand` would read to oxlint as a member never used).
/// The declaration, then the brand.
fn struct_brand(name: &str) -> (String, String) {
    (format!("declare class {name}$brand {{\n  private brand: unknown;\n}}\n"), format!("{name}$brand"))
}

/// A newtype is its inner value at runtime (as in serde's JSON), branded so
/// that `Id` and the bare inner type do not mix. A generic one is generic
/// in TS too: `Wrap<T> = T & {...}`, built by `of: <T>(value: T): Wrap<T>`.
pub(crate) fn emit_newtype(krate: &Crate, st: &Struct, inner: &Ty) -> String {
    let name = st.name.as_str();
    let (head, ty) = ctor_head(name, &st.generics, &[inner]);
    let inner = emit_ty(inner);
    let (brand_decl, branded) = if st.closed { closed_brand(name) } else { (String::new(), brand(krate, name)) };
    let typed = declared(name, &st.generics);
    let mut out = format!("{brand_decl}export type {typed} = {inner} & {branded};\n\n");
    if st.closed {
        out.push_str(&closed_ctor_src(name, (&head, &ty), &format!("value: {inner}"), "value"));
    }
    out.push_str(&format!("export const {name} = {{\n"));
    if !st.closed {
        out.push_str(&format!("  of: {head}(value: {inner}): {ty} => value as {ty},\n"));
    }
    out.push_str(&companion_methods(krate, name));
    out
}

/// A struct with a field that is not `pub`: branded, so an object literal is
/// not one, and with no `of` on the companion (design/01 §4).
pub(crate) fn emit_closed_struct(krate: &Crate, st: &Struct, fields: &str) -> String {
    let name = st.name.as_str();
    let shape =
        st.fields.iter().map(|f| format!("{}: {}", f.name.as_str(), emit_ty(&f.ty))).collect::<Vec<_>>().join("; ");
    let (brand_decl, branded) = struct_brand(name);
    let typed = declared(name, &st.generics);
    // A type name after a many-line object type goes on a line of its own,
    // as oxfmt lays out the intersection.
    let mut out = format!("{brand_decl}export type {typed} = Readonly<{{\n{fields}\n}}> &\n  {branded};\n\n");
    let head = ctor_head(name, &st.generics, &st.fields.iter().map(|f| &f.ty).collect::<Vec<_>>());
    out.push_str(&closed_ctor_src(name, (&head.0, &head.1), &format!("fields: Readonly<{{ {shape} }}>"), "fields"));
    out.push_str(&format!("export const {name} = {{\n"));
    out.push_str(&companion_methods(krate, name));
    out
}

/// `(head, ty)` is `ctor_head`'s: `("<T>", "Hid<T>")` for a generic type,
/// `("", "Hid")` for another.
pub(crate) fn closed_ctor_src(name: &str, (head, ty): (&str, &str), param: &str, arg: &str) -> String {
    format!(
        "/**\n\
         \x20* Makes `{name}` values without a check.\n\
         \x20*\n\
         \x20* Its fields are not `pub` in Rust, so outside the crate a value comes only\n\
         \x20* from the crate's functions; the generated files build them here, and\n\
         \x20* `index.ts` does not export it.\n\
         \x20* @internal\n\
         \x20*/\n\
         export const {ctor} = {head}({param}): {ty} => {arg} as {ty};\n\n",
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

/// A trait (design/01 §7.25): the type of its record over `Self`, each
/// supertrait's record beside its own members, then its companion, which
/// holds the default bodies, each taking the dictionary first.
pub(crate) fn emit_trait(krate: &Crate, t: &purecrate_ir::Trait) -> String {
    let name = t.name.as_str();
    let self_ = purecrate_ir::SELF;
    let mut members: Vec<(String, &Option<String>)> = Vec::new();
    for c in &t.consts {
        members.push((format!("{}: {}", c.name.as_str(), emit_ty(&c.ty)), &c.doc));
    }
    for m in &t.methods {
        let params = m
            .params
            .iter()
            .map(|p| format!("{}: {}", purecrate_ir::to_camel(p.name.as_str()), emit_ty(&p.ty)))
            .collect::<Vec<_>>()
            .join(", ");
        members.push((format!("{}: ({params}) => {}", purecrate_ir::to_camel(m.name.as_str()), emit_ty(&m.ret)), &m.doc));
    }
    let supers: Vec<String> = t.supertraits.iter().map(|s| format!("{}<{self_}>", s.as_str())).collect();
    let head = format!("export type {name}<{self_}> = ");
    let one = members.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>().join("; ");
    let own = if members.is_empty() { None } else { Some(format!("Readonly<{{ {one} }}>")) };
    let parts: Vec<String> = supers.iter().cloned().chain(own.clone()).collect();
    let parts = if parts.is_empty() { vec!["Readonly<Record<string, never>>".to_string()] } else { parts };
    let line = format!("{head}{};", parts.join(" & "));
    let mut out = jsdoc(&t.doc, "");
    if line.len() <= crate::WIDTH && members.iter().all(|(_, d)| d.is_none()) {
        out.push_str(&line);
    } else {
        let mut record = String::from("Readonly<{\n");
        for (m, d) in &members {
            record.push_str(&jsdoc(d, "  "));
            record.push_str(&format!("  {m};\n"));
        }
        record.push_str("}>");
        let lead: String = supers.iter().map(|s| format!("{s} & ")).collect();
        out.push_str(&format!("{head}{lead}{record};"));
    }
    out.push_str("\n\n");
    out.push_str(&format!("export const {name} = {{\n"));
    out.push_str(&companion_methods(krate, name));
    out
}

pub(crate) fn emit_free_fn(f: &Fn) -> String {
    jsdoc(&f.doc, "")
        + &format!(
            "export const {name} = {impl};\n",
            name = f.name.as_str(),
            impl = fn_arrow(f, 0)
        )
}
