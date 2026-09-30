//! `fixture!` for the differential tests: includes Rust sources into a module
//! and gives every struct and enum in them a canonical `Show`, derived from
//! the PureCrate IR. The TS side prints the same text from the same IR
//! (`crates/cli/tests/support`), so a case compares the whole value, not a
//! projection a test chose by hand (design/01 §7).
//!
//! ```ignore
//! purecrate_canon::fixture!(mod order = "../../examples/order/src/lib.rs", "fixtures/order_driver.rs");
//! ```
//!
//! Paths are relative to the test crate's `tests/`. The module also gets
//! `SOURCE`, the files joined by newlines, for the generated package.
//!
//! The canonical text, per type:
//!
//! | Type | Text |
//! | --- | --- |
//! | integers | decimal |
//! | `f32`, `f64` | the bits of the value as `f64`, decimal |
//! | `bool` | `true` / `false` |
//! | `String`, `str` | `"…"`; printable ASCII but `"` and `\` as is, others `\u{hex}` |
//! | `char` | `'"…"'`: the string text between `'` |
//! | `uuid::Uuid` | `Uuid("…")`: the hyphenated lowercase form |
//! | `uuid::Error` | `UuidError` |
//! | `()` | `()` |
//! | `Option` | `None` / `Some(x)` |
//! | `Result` | `Ok(x)` / `Err(e)` |
//! | `Vec` | `[a, b]` |
//! | tuple | `(a, b)` |
//! | `Box`, `Arc`, `Mutex` | the inner value |
//! | struct | `S { a: x, b: y }`; newtype `S(x)`; no fields `S` |
//! | enum | `E::V`, `E::V(x, y)`, `E::V { a: x }` |
//!
//! Every struct and enum also gets `Js`, the TS literal of the same value
//! (a newtype is its content, as brands exist only in types; enums are
//! `kind` objects), so a case can pass whole domain values as arguments.
//!
//! Every struct and enum also gets `serde_core::Serialize`, making the same
//! data-model calls `#[derive(Serialize)]` makes with no attributes, so
//! `serde_json::to_string` gives the JSON a Rust server would send
//! (design/04). The derive itself is not used: its vendored `syn` is older
//! than `serde_derive` accepts.

use proc_macro::TokenStream;
use purecrate_ir::{Item, Struct, VariantFields, NEWTYPE_FIELD};

const SHOW: &str = "crate::support::Show";

#[proc_macro]
pub fn fixture(input: TokenStream) -> TokenStream {
    // Parse with proc-macro2's own implementation: the compiler's spans
    // have no line and column outside nightly.
    proc_macro2::fallback::force();
    let (module, paths) = parse_input(input);
    let root =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join("tests");
    let source = paths
        .iter()
        .map(|p| {
            std::fs::read_to_string(root.join(p))
                .unwrap_or_else(|e| panic!("fixture!: read {p}: {e}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let krate = purecrate_syntax::parse_source(&module, &source)
        .unwrap_or_else(|e| panic!("fixture!: {module} does not parse: {e}"));

    let at = |p: &str| format!("concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/tests/\", {p:?})");
    // The fixture is written to the subset, which has none of the std
    // methods clippy would suggest (`is_multiple_of`, `contains`, ..).
    let mut out = format!("#[allow(dead_code, clippy::all)]\nmod {module} {{\n");
    for p in &paths {
        let text = std::fs::read_to_string(root.join(p)).expect("read again");
        if text.contains("serde") {
            out.push_str(&without_serde(&text));
            out.push('\n');
        } else {
            out.push_str(&format!("    include!({});\n", at(p)));
        }
    }
    let joined = paths
        .iter()
        .map(|p| format!("include_str!({})", at(p)))
        .collect::<Vec<_>>()
        .join(", \"\\n\", ");
    out.push_str(&format!(
        "    pub const SOURCE: &str = concat!({joined});\n"
    ));
    for item in &krate.items {
        match item {
            Item::Struct(st) => {
                out.push_str(&show_struct(st));
                out.push_str(&serialize_struct(st));
                out.push_str(&js_struct(st));
            }
            // std's `Ordering`: its `Show` and `Js` are the harness's own,
            // and serde has no `Serialize` for it.
            Item::Enum(en) if en.std => {}
            Item::Enum(en) => {
                out.push_str(&show_enum(en));
                out.push_str(&serialize_enum(en));
                out.push_str(&js_enum(en));
            }
            Item::Alias(_) | Item::Fn(_) | Item::Const(_) => {}
        }
    }
    out.push_str("}\n");
    out.parse().expect("fixture!: generated tokens")
}

/// `mod name = "a.rs", "b.rs"`
fn parse_input(input: TokenStream) -> (String, Vec<String>) {
    use proc_macro::TokenTree;
    let tokens: Vec<TokenTree> = input.into_iter().collect();
    let usage = "fixture!(mod name = \"path.rs\", ...)";
    match tokens.as_slice() {
        [TokenTree::Ident(kw), TokenTree::Ident(name), TokenTree::Punct(eq), rest @ ..]
            if kw.to_string() == "mod" && eq.as_char() == '=' =>
        {
            let mut paths = Vec::new();
            for (i, t) in rest.iter().enumerate() {
                match (i % 2, t) {
                    (0, TokenTree::Literal(lit)) => {
                        let text = lit.to_string();
                        let path = text
                            .strip_prefix('"')
                            .and_then(|t| t.strip_suffix('"'))
                            .unwrap_or_else(|| {
                                panic!("{usage}: expected a string path, got {text}")
                            });
                        paths.push(path.to_string());
                    }
                    (1, TokenTree::Punct(p)) if p.as_char() == ',' => {}
                    _ => panic!("{usage}: unexpected {t}"),
                }
            }
            assert!(!paths.is_empty(), "{usage}: no path");
            (name.to_string(), paths)
        }
        _ => panic!("{usage}"),
    }
}

fn show_struct(st: &Struct) -> String {
    let name = st.name.as_str();
    let body = match st.fields.as_slice() {
        [] => format!("{name:?}.to_string()"),
        [f] if f.name.as_str() == NEWTYPE_FIELD => {
            format!("format!(\"{name}({{}})\", {SHOW}::show(&self.0))")
        }
        fields => {
            let (pattern, args) = named(fields.iter().map(|f| f.name.as_str()), "&self.");
            format!("format!(\"{name} {{{{ {pattern} }}}}\", {args})")
        }
    };
    format!("    impl {SHOW} for {name} {{\n        fn show(&self) -> String {{\n            {body}\n        }}\n    }}\n")
}

fn show_enum(en: &purecrate_ir::Enum) -> String {
    let name = en.name.as_str();
    let mut arms = String::new();
    for v in &en.variants {
        let var = v.name.as_str();
        let arm = match &v.fields {
            VariantFields::Unit => format!("{name}::{var} => \"{name}::{var}\".to_string(),"),
            VariantFields::Tuple(tys) => {
                let binds = (0..tys.len()).map(|i| format!("f{i}")).collect::<Vec<_>>();
                let holes = vec!["{}"; tys.len()].join(", ");
                let args = binds
                    .iter()
                    .map(|b| format!("{SHOW}::show({b})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{name}::{var}({}) => format!(\"{name}::{var}({holes})\", {args}),",
                    binds.join(", ")
                )
            }
            VariantFields::Struct(fields) => {
                let names = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
                let (pattern, args) = named(names.iter().copied(), "");
                format!(
                    "{name}::{var} {{ {} }} => format!(\"{name}::{var} {{{{ {pattern} }}}}\", {args}),",
                    names.join(", ")
                )
            }
        };
        arms.push_str(&format!("                {arm}\n"));
    }
    format!(
        "    impl {SHOW} for {name} {{\n        fn show(&self) -> String {{\n            match self {{\n{arms}            }}\n        }}\n    }}\n"
    )
}

/// `a: {}, b: {}` and the matching `Show::show(<prefix>a), ...`.
fn named<'a>(names: impl Iterator<Item = &'a str>, prefix: &str) -> (String, String) {
    let names: Vec<&str> = names.collect();
    let pattern = names
        .iter()
        .map(|n| format!("{n}: {{}}"))
        .collect::<Vec<_>>()
        .join(", ");
    let args = names
        .iter()
        .map(|n| format!("{SHOW}::show({prefix}{n})"))
        .collect::<Vec<_>>()
        .join(", ");
    (pattern, args)
}

const SER: &str = "::serde_core::Serialize";

fn serialize_impl(name: &str, body: &str) -> String {
    format!(
        "    impl {SER} for {name} {{\n        fn serialize<S: ::serde_core::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {{\n            #[allow(unused_imports)]\n            use ::serde_core::ser::{{SerializeStruct, SerializeStructVariant, SerializeTupleVariant}};\n            {body}\n        }}\n    }}\n"
    )
}

fn serialize_struct(st: &Struct) -> String {
    let name = st.name.as_str();
    let body = match st.fields.as_slice() {
        [f] if f.name.as_str() == NEWTYPE_FIELD => format!("s.serialize_newtype_struct({name:?}, &self.0)"),
        fields => {
            let mut body = format!("let mut t = s.serialize_struct({name:?}, {})?;", fields.len());
            for f in fields {
                let field = f.name.as_str();
                body.push_str(&format!(" t.serialize_field({field:?}, &self.{field})?;"));
            }
            body.push_str(" t.end()");
            body
        }
    };
    serialize_impl(name, &body)
}

fn serialize_enum(en: &purecrate_ir::Enum) -> String {
    let name = en.name.as_str();
    let mut arms = String::new();
    for (i, v) in en.variants.iter().enumerate() {
        let var = v.name.as_str();
        let arm = match &v.fields {
            VariantFields::Unit => format!("{name}::{var} => s.serialize_unit_variant({name:?}, {i}, {var:?}),"),
            VariantFields::Tuple(tys) if tys.len() == 1 => {
                format!("{name}::{var}(f0) => s.serialize_newtype_variant({name:?}, {i}, {var:?}, f0),")
            }
            VariantFields::Tuple(tys) => {
                let binds = (0..tys.len()).map(|i| format!("f{i}")).collect::<Vec<_>>();
                let fields = binds
                    .iter()
                    .map(|b| format!(" t.serialize_field({b})?;"))
                    .collect::<String>();
                format!(
                    "{name}::{var}({}) => {{ let mut t = s.serialize_tuple_variant({name:?}, {i}, {var:?}, {})?;{fields} t.end() }}",
                    binds.join(", "),
                    tys.len()
                )
            }
            VariantFields::Struct(fs) => {
                let names = fs.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
                let fields = names
                    .iter()
                    .map(|n| format!(" t.serialize_field({n:?}, {n})?;"))
                    .collect::<String>();
                format!(
                    "{name}::{var} {{ {} }} => {{ let mut t = s.serialize_struct_variant({name:?}, {i}, {var:?}, {})?;{fields} t.end() }}",
                    names.join(", "),
                    fs.len()
                )
            }
        };
        arms.push_str(&format!("                {arm}\n"));
    }
    serialize_impl(name, &format!("match self {{\n{arms}            }}"))
}

/// The file without its serde derives, `#[serde(...)]` attributes, and `use
/// serde` items: the test crate has no serde_derive, and the `Serialize`
/// impls above stand in for the derive. Spans are lost, so only files that
/// mention serde go this way; the rest are `include!`d.
fn without_serde(text: &str) -> String {
    use quote::ToTokens;
    let mut file: syn::File = syn::parse_str(text).unwrap_or_else(|e| panic!("fixture!: {e}"));
    file.items.retain(|item| match item {
        syn::Item::Use(u) => !u.to_token_stream().to_string().contains("serde"),
        _ => true,
    });
    for item in &mut file.items {
        let attrs = match item {
            syn::Item::Struct(s) => &mut s.attrs,
            syn::Item::Enum(e) => &mut e.attrs,
            _ => continue,
        };
        attrs.retain(|a| !a.path().is_ident("serde"));
        for attr in attrs.iter_mut().filter(|a| a.path().is_ident("derive")) {
            let kept: Vec<syn::Path> = attr
                .parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
                .expect("derive list")
                .into_iter()
                .filter(|p| !matches!(p.segments.last().map(|s| s.ident.to_string()).as_deref(), Some("Serialize" | "Deserialize")))
                .collect();
            *attr = syn::parse_quote!(#[derive(#(#kept),*)]);
        }
    }
    file.to_token_stream().to_string()
}

const JS: &str = "crate::support::Js";

fn js_impl(name: &str, body: &str) -> String {
    format!("    impl {JS} for {name} {{\n        fn js(&self) -> String {{\n            {body}\n        }}\n    }}\n")
}

/// `{ a: .., b: .. }` from the `Js` of each `<prefix><name>`.
fn js_fields<'a>(names: impl Iterator<Item = &'a str>, prefix: &str) -> (String, String) {
    let names: Vec<&str> = names.collect();
    let pattern = names.iter().map(|n| format!("{n}: {{}}")).collect::<Vec<_>>().join(", ");
    let args = names.iter().map(|n| format!("{JS}::js({prefix}{n})")).collect::<Vec<_>>().join(", ");
    (pattern, args)
}

fn js_struct(st: &Struct) -> String {
    let name = st.name.as_str();
    let body = match st.fields.as_slice() {
        [f] if f.name.as_str() == NEWTYPE_FIELD => format!("{JS}::js(&self.0)"),
        [] => "\"({})\".to_string()".to_string(),
        fields => {
            let (pattern, args) = js_fields(fields.iter().map(|f| f.name.as_str()), "&self.");
            format!("format!(\"({{{{ {pattern} }}}})\", {args})")
        }
    };
    js_impl(name, &body)
}

fn js_enum(en: &purecrate_ir::Enum) -> String {
    let name = en.name.as_str();
    let mut arms = String::new();
    for v in &en.variants {
        let var = v.name.as_str();
        let arm = match &v.fields {
            VariantFields::Unit => format!("{name}::{var} => \"({{ kind: \\\"{var}\\\" }})\".to_string(),"),
            VariantFields::Tuple(tys) => {
                let binds = (0..tys.len()).map(|i| format!("f{i}")).collect::<Vec<_>>();
                let holes = vec!["{}"; tys.len()].join(", ");
                let args = binds.iter().map(|b| format!("{JS}::js({b})")).collect::<Vec<_>>().join(", ");
                format!(
                    "{name}::{var}({}) => format!(\"({{{{ kind: \\\"{var}\\\", content: [{holes}] }}}})\", {args}),",
                    binds.join(", ")
                )
            }
            VariantFields::Struct(fields) => {
                let names = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
                let (pattern, args) = js_fields(names.iter().copied(), "");
                format!(
                    "{name}::{var} {{ {} }} => format!(\"({{{{ kind: \\\"{var}\\\", {pattern} }}}})\", {args}),",
                    names.join(", ")
                )
            }
        };
        arms.push_str(&format!("                {arm}\n"));
    }
    js_impl(name, &format!("match self {{\n{arms}            }}"))
}
