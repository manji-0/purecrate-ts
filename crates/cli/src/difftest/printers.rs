//! The TS half of the canonical text a differential case compares: each
//! value printed from the generated package as the Rust side prints it
//! (design/01 §8). `test` uses it, and the differential tests include this
//! file by path, so both print alike.

use purecrate_ir::{Crate, Field, Item, Prim, Ty, VariantFields, NEWTYPE_FIELD};

/// What every driver starts with: the package, a printer per primitive and
/// per wrapper, and `run`, which prints a result or `panic(message)`.
pub const PRELUDE: &str = r#"import * as pkg from "./src/index.ts";
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
// A map's or a set's keys in Rust's order: by code point, not UTF-16 unit.
const cps = (s) => [...s].map((c) => c.codePointAt(0));
const byCp = (a, b) => {
  const x = cps(a);
  const y = cps(b);
  for (let i = 0; i < Math.min(x.length, y.length); i++) if (x[i] !== y[i]) return x[i] - y[i];
  return x.length - y.length;
};
const map = (f) => (m) => `{${[...m.keys()].sort(byCp).map((k) => `${str(k)}: ${f(m.get(k))}`).join(", ")}}`;
const set = (s) => `{${[...s].sort(byCp).map(str).join(", ")}}`;
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

/// A JS function that prints a value of `ty` as `Show` does in Rust. `None`
/// for a type no value of which is printed (a closure, `!`).
pub fn ts_printer(krate: &Crate, ty: &Ty) -> Option<String> {
    Some(match ty {
        Ty::Prim(p) => match p {
            Prim::Bool => "String".into(),
            Prim::F32 | Prim::F64 => "bits".into(),
            Prim::String | Prim::Str => "str".into(),
            Prim::Char => "chr".into(),
            Prim::Uuid => "((x) => `Uuid(${str(x)})`)".into(),
            Prim::UuidError => "(() => \"UuidError\")".into(),
            Prim::ParseIntError => "(() => \"ParseIntError\")".into(),
            Prim::Unit => "unit".into(),
            _ => "int".into(),
        },
        Ty::Option(inner) => format!("opt({})", ts_printer(krate, inner)?),
        Ty::Result { ok, err } => format!("res({}, {})", ts_printer(krate, ok)?, ts_printer(krate, err)?),
        Ty::Vec(inner) => format!("vec({})", ts_printer(krate, inner)?),
        Ty::Map { value, .. } => format!("map({})", ts_printer(krate, value)?),
        Ty::Set(_) => "set".into(),
        Ty::Tuple(elems) => {
            format!("tup([{}])", elems.iter().map(|t| ts_printer(krate, t)).collect::<Option<Vec<_>>>()?.join(", "))
        }
        Ty::Ignored { inner, .. } => ts_printer(krate, inner)?,
        Ty::Named(n) => match krate.items.iter().find(|i| i.name() == n) {
            Some(Item::Alias(al)) => ts_printer(krate, &al.ty)?,
            // Wrapped: the printer may be declared further down.
            Some(Item::Struct(_) | Item::Enum(_)) => format!("((x) => show${}(x))", n.as_str()),
            _ => return None,
        },
        Ty::Fn { .. } | Ty::Never => return None,
    })
}

/// `show$T` for every struct and enum: the TS shape of the value (design/03
/// §2) printed as the Rust side prints it.
pub fn ts_printers(krate: &Crate) -> String {
    let mut out = String::new();
    for item in &krate.items {
        match item {
            Item::Struct(st) => {
                let name = st.name.as_str();
                let body = match st.fields.as_slice() {
                    [] => format!("{name:?}"),
                    [f] if f.name.as_str() == NEWTYPE_FIELD => {
                        format!("`{name}(${{({})(v)}})`", printer_or_fail(krate, &f.ty))
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
                                    format!("${{({})({field})}}", printer_or_fail(krate, t))
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

fn ts_fields(krate: &Crate, fields: &[Field], value: &str) -> String {
    fields
        .iter()
        .map(|f| {
            let n = f.name.as_str();
            format!("{n}: ${{({})({value}.{n})}}", printer_or_fail(krate, &f.ty))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A field's printer. A field holds no closure, so this prints one that
/// says so rather than fail the whole driver.
fn printer_or_fail(krate: &Crate, ty: &Ty) -> String {
    ts_printer(krate, ty).unwrap_or_else(|| "(() => \"unprintable\")".into())
}
