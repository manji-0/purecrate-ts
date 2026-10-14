//! What `test`'s harness and `purecrate_canon::fixture!` both write for a
//! crate's structs and enums, as Rust source: `Show`, the canonical text
//! (design/01 §8), and the TS literal of a value (design/03 §2). The canon
//! crate includes this file by path, so it uses `purecrate_ir` alone; each
//! caller names its own trait path and the attributes its impls carry.

use purecrate_ir::{Enum, Name, Struct, VariantFields, NEWTYPE_FIELD};

/// `impl<T: Bound> ... for Name<T>`'s two halves, or the bare name.
pub fn head(name: &str, generics: &[Name], bound: &str) -> (String, String) {
    if generics.is_empty() {
        return (String::new(), name.to_string());
    }
    let params = generics.iter().map(|g| format!("{}: {bound}", g.as_str())).collect::<Vec<_>>().join(", ");
    let args = generics.iter().map(|g| g.as_str()).collect::<Vec<_>>().join(", ");
    (format!("<{params}>"), format!("{name}<{args}>"))
}

/// `impl trait_path for name` with the one method `sig` and its `body`, each
/// type parameter bound by the trait, after `attrs`.
pub fn impl_block(attrs: &str, trait_path: &str, name: &str, generics: &[Name], sig: &str, body: &str) -> String {
    let (h, ty) = head(name, generics, trait_path);
    format!("\n{attrs}impl{h} {trait_path} for {ty} {{\n    {sig} {{\n        {body}\n    }}\n}}\n")
}

/// `f0, f1, ..`: the bindings of a tuple variant's `n` fields.
pub fn binds(n: usize) -> String {
    (0..n).map(|i| format!("f{i}")).collect::<Vec<_>>().join(", ")
}

/// `a: {}, b: {}` and the matching `call(<prefix>a), ...`.
pub fn named<'a>(names: impl Iterator<Item = &'a str>, prefix: &str, call: &str) -> (String, String) {
    let names: Vec<&str> = names.collect();
    let pattern = names.iter().map(|n| format!("{n}: {{}}")).collect::<Vec<_>>().join(", ");
    let args = names.iter().map(|n| format!("{call}({prefix}{n})")).collect::<Vec<_>>().join(", ");
    (pattern, args)
}

/// `Show` for `st` as the trait at `show`: `S { a: x }`, a newtype `S(x)`,
/// no fields `S`.
pub fn show_struct(st: &Struct, show: &str, attrs: &str) -> String {
    let name = st.name.as_str();
    let body = match st.fields.as_slice() {
        [] => format!("{name:?}.to_string()"),
        [f] if f.name.as_str() == NEWTYPE_FIELD => format!("format!(\"{name}({{}})\", {show}::show(&self.0))"),
        fields => {
            let (pattern, args) = named(fields.iter().map(|f| f.name.as_str()), "&self.", &format!("{show}::show"));
            format!("format!(\"{name} {{{{ {pattern} }}}}\", {args})")
        }
    };
    impl_block(attrs, show, name, &st.generics, "fn show(&self) -> String", &body)
}

/// `Show` for `en` as the trait at `show`: `E::V`, `E::V(x, y)`,
/// `E::V { a: x }`.
pub fn show_enum(en: &Enum, show: &str, attrs: &str) -> String {
    let name = en.name.as_str();
    let mut arms = String::new();
    for v in &en.variants {
        let var = v.name.as_str();
        let arm = match &v.fields {
            VariantFields::Unit => format!("{name}::{var} => \"{name}::{var}\".to_string(),"),
            VariantFields::Tuple(tys) => {
                let holes = vec!["{}"; tys.len()].join(", ");
                let args = (0..tys.len()).map(|i| format!("{show}::show(f{i})")).collect::<Vec<_>>().join(", ");
                format!("{name}::{var}({}) => format!(\"{name}::{var}({holes})\", {args}),", binds(tys.len()))
            }
            VariantFields::Struct(fields) => {
                let names = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
                let (pattern, args) = named(names.iter().copied(), "", &format!("{show}::show"));
                format!(
                    "{name}::{var} {{ {} }} => format!(\"{name}::{var} {{{{ {pattern} }}}}\", {args}),",
                    names.join(", ")
                )
            }
        };
        arms.push_str(&format!("            {arm}\n"));
    }
    impl_block(attrs, show, name, &en.generics, "fn show(&self) -> String", &format!("match self {{\n{arms}        }}"))
}

/// A struct's or a variant's fields, as its TS literal holds them.
pub enum Fields<'a> {
    Named(Vec<&'a str>),
    /// `value` for one, `content` for more, nothing for none.
    Tuple(usize),
}

impl<'a> Fields<'a> {
    pub fn of(fields: &'a VariantFields) -> Self {
        match fields {
            VariantFields::Unit => Fields::Tuple(0),
            VariantFields::Tuple(tys) => Fields::Tuple(tys.len()),
            VariantFields::Struct(fs) => Fields::Named(fs.iter().map(|f| f.name.as_str()).collect()),
        }
    }
}

/// The Rust expression giving the TS literal of a struct (`kind` none) or
/// of a variant: `({ a: .., b: .. })`, `({ kind: "V", value: .. })`, each
/// field's TS from `args` in order. A newtype's literal is its content, as
/// brands exist only in types, so it is not made here.
pub fn js_literal(kind: Option<&str>, fields: &Fields<'_>, args: &[String]) -> String {
    let mut parts: Vec<String> = kind.map(|k| format!("kind: \\\"{k}\\\"")).into_iter().collect();
    match fields {
        Fields::Named(names) => parts.extend(names.iter().map(|n| format!("{n}: {{}}"))),
        Fields::Tuple(0) => {}
        Fields::Tuple(1) => parts.push("value: {}".to_string()),
        Fields::Tuple(n) => parts.push(format!("content: [{}]", vec!["{}"; *n].join(", "))),
    }
    match (parts.is_empty(), args.is_empty()) {
        (true, _) => "\"({})\".to_string()".to_string(),
        (false, true) => format!("\"({{ {} }})\".to_string()", parts.join(", ")),
        (false, false) => format!("format!(\"({{{{ {} }}}})\", {})", parts.join(", "), args.join(", ")),
    }
}
