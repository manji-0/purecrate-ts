//! `fixture!` for the differential tests: includes Rust sources into a module
//! and gives every struct and enum in them a canonical `Show`, derived from
//! the PureCrate IR. The TS side prints the same text from the same IR
//! (`crates/cli/tests/support`), so a case compares the whole value, not a
//! projection a test chose by hand (design/04 §1.4.1).
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
//! | `()` | `()` |
//! | `Option` | `None` / `Some(x)` |
//! | `Result` | `Ok(x)` / `Err(e)` |
//! | `Vec` | `[a, b]` |
//! | tuple | `(a, b)` |
//! | `Box`, `Arc`, `Mutex` | the inner value |
//! | struct | `S { a: x, b: y }`; newtype `S(x)`; no fields `S` |
//! | enum | `E::V`, `E::V(x, y)`, `E::V { a: x }` |

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
    let mut out = format!("#[allow(dead_code)]\nmod {module} {{\n");
    for p in &paths {
        out.push_str(&format!("    include!({});\n", at(p)));
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
            Item::Struct(st) => out.push_str(&show_struct(st)),
            Item::Enum(en) => out.push_str(&show_enum(en)),
            Item::Alias(_) | Item::Fn(_) => {}
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
