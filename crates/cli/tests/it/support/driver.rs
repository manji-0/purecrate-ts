//! The node driver: each case's TS call, printed as `Show` prints the Rust
//! value.

use purecrate_ir::{Crate, Item, Prim, Ty, VariantFields, NEWTYPE_FIELD};

use super::Case;

/// The TS half of the canonical text (see `Show`). `ts_printer` composes
/// these by type.
const PRELUDE: &str = r#"import * as pkg from "./src/index.ts";
const int = (x) => (Object.is(x, -0) ? "-0" : String(x));
const bits = (x) => String(new BigUint64Array(new Float64Array([x]).buffer)[0]);
const str = (s) => {
  let out = '"';
  for (const c of s) {
    const p = c.codePointAt(0);
    out += p >= 0x20 && p <= 0x7e && c !== '"' && c !== "\\" ? c : `\\u{${p.toString(16)}}`;
  }
  return out + '"';
};
const chr = (c) => `'${str(c)}'`;
const unit = (x) => (x === undefined ? "()" : `not unit: ${String(x)}`);
const opt = (f) => (x) => (x === null ? "None" : `Some(${f(x)})`);
const res = (f, g) => (x) => (x.kind === "Ok" ? `Ok(${f(x.value)})` : `Err(${g(x.error)})`);
const vec = (f) => (xs) => `[${xs.map((x) => f(x)).join(", ")}]`;
const tup = (fs) => (xs) => `(${fs.map((f, i) => f(xs[i])).join(", ")})`;
const run = (f, print) => {
  let r;
  try {
    r = f();
  } catch (e) {
    return `panic(${e instanceof Error ? e.message : String(e)})`;
  }
  return print(r);
};
"#;

/// A JS function that prints a value of `ty` as `Show` does in Rust.
fn ts_printer(krate: &Crate, ty: &Ty) -> String {
    match ty {
        Ty::Prim(p) => match p {
            Prim::Bool => "String".into(),
            Prim::F32 | Prim::F64 => "bits".into(),
            Prim::String | Prim::Str => "str".into(),
            Prim::Char => "chr".into(),
            Prim::Uuid => "((x) => `Uuid(${str(x)})`)".into(),
            Prim::UuidError => "(() => \"UuidError\")".into(),
            Prim::Unit => "unit".into(),
            _ => "int".into(),
        },
        Ty::Option(inner) => format!("opt({})", ts_printer(krate, inner)),
        Ty::Result { ok, err } => format!("res({}, {})", ts_printer(krate, ok), ts_printer(krate, err)),
        Ty::Vec(inner) => format!("vec({})", ts_printer(krate, inner)),
        Ty::Tuple(elems) => {
            format!("tup([{}])", elems.iter().map(|t| ts_printer(krate, t)).collect::<Vec<_>>().join(", "))
        }
        Ty::Ignored { inner, .. } => ts_printer(krate, inner),
        Ty::Named(n) => match krate.items.iter().find(|i| i.name() == n) {
            Some(Item::Alias(al)) => ts_printer(krate, &al.ty),
            // Wrapped: the printer may be declared further down.
            Some(Item::Struct(_) | Item::Enum(_)) => format!("((x) => show${}(x))", n.as_str()),
            _ => panic!("no printable type `{}`", n.as_str()),
        },
        Ty::Fn { .. } | Ty::Never => panic!("a case cannot return {ty:?}"),
    }
}

/// `show$T` for every struct and enum: the TS shape of the value (design/03
/// §2) printed as `purecrate_canon` prints the Rust one.
fn ts_printers(krate: &Crate) -> String {
    let mut out = String::new();
    for item in &krate.items {
        match item {
            Item::Struct(st) => {
                let name = st.name.as_str();
                let body = match st.fields.as_slice() {
                    [] => format!("{name:?}"),
                    [f] if f.name.as_str() == NEWTYPE_FIELD => {
                        format!("`{name}(${{({})(v)}})`", ts_printer(krate, &f.ty))
                    }
                    fields => format!("`{name} {{ {} }}`", ts_fields(krate, fields, "v")),
                };
                out.push_str(&format!("const show${name} = (v) => {body};\n"));
            }
            Item::Enum(en) => {
                let name = en.name.as_str();
                let mut arms = String::new();
                for v in &en.variants {
                    let var = v.name.as_str();
                    let text = match &v.fields {
                        VariantFields::Unit => format!("\"{name}::{var}\""),
                        VariantFields::Tuple(tys) => format!(
                            "`{name}::{var}({})`",
                            tys.iter()
                                .enumerate()
                                .map(|(i, t)| {
                                    let field =
                                        if tys.len() == 1 { "v.value".to_string() } else { format!("v.content[{i}]") };
                                    format!("${{({})({field})}}", ts_printer(krate, t))
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        VariantFields::Struct(fields) => {
                            format!("`{name}::{var} {{ {} }}`", ts_fields(krate, fields, "v"))
                        }
                    };
                    arms.push_str(&format!("    case \"{var}\": return {text};\n"));
                }
                out.push_str(&format!(
                    "const show${name} = (v) => {{\n  switch (v.kind) {{\n{arms}    default: return `not a {name}: ${{String(v.kind)}}`;\n  }}\n}};\n"
                ));
            }
            Item::Alias(_) | Item::Fn(_) | Item::Const(_) => {}
        }
    }
    out
}

fn ts_fields(krate: &Crate, fields: &[purecrate_ir::Field], value: &str) -> String {
    fields
        .iter()
        .map(|f| {
            let n = f.name.as_str();
            format!("{n}: ${{({})({value}.{n})}}", ts_printer(krate, &f.ty))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn driver(krate: &Crate, cases: &[Case]) -> String {
    // The TS spelling of each function (`check::rename`).
    let mut names: Vec<String> = cases.iter().map(|c| purecrate_ir::to_camel(c.name)).collect();
    names.sort();
    names.dedup();
    let mut out = String::from(PRELUDE);
    out.push_str(&ts_printers(krate));
    out.push_str(&format!("const {{ {} }} = pkg;\n", names.join(", ")));
    out.push_str("const out = [\n");
    for c in cases {
        let ret = krate
            .items
            .iter()
            .find_map(|i| match i {
                Item::Fn(f) if f.owner.is_none() && f.name.as_str() == c.name => Some(&f.ret),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no free function `{}`", c.name));
        out.push_str(&format!("  run(() => {}, {}),\n", c.call, ts_printer(krate, ret)));
    }
    out.push_str("];\nconsole.log(out.join(\"\\n\"));\n");
    out
}
