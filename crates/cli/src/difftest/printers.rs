//! The TS half of the canonical text a differential case compares: each
//! value printed from the generated package as the Rust side prints it
//! (design/01 §8). `test` uses it, and the differential tests include this
//! file by path, so both print alike.

use purecrate_ir::{Crate, Field, Item, Name, Prim, Ty, VariantFields, NEWTYPE_FIELD};

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

/// A node script: `PRELUDE`, the crate's printers, `defs`, and `out`,
/// the result of each of `runs` (a `run(..)` call), printed one per line
/// by `print` (`out.join("\n")`, or with each line escaped).
pub fn driver(krate: &Crate, defs: &str, runs: impl IntoIterator<Item = String>, print: &str) -> String {
    let runs: String = runs.into_iter().map(|r| format!("  {r},\n")).collect();
    format!("{PRELUDE}{}{defs}const out = [\n{runs}];\nconsole.log({print});\n", ts_printers(krate))
}

/// A JS function that prints a value of `ty` as `Show` does in Rust. `None`
/// for a type no value of which is printed (a closure, `!`).
pub fn ts_printer(krate: &Crate, ty: &Ty) -> Option<String> {
    printer(krate, &[], ty)
}

/// `ts_printer` inside the printer of a type with type parameters
/// `params`, each printed by the printer that one was given.
fn printer(krate: &Crate, params: &[Name], ty: &Ty) -> Option<String> {
    let each = |ts: &[Ty]| ts.iter().map(|t| printer(krate, params, t)).collect::<Option<Vec<_>>>();
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
        Ty::Option(inner) => format!("opt({})", printer(krate, params, inner)?),
        Ty::Result { ok, err } => format!("res({}, {})", printer(krate, params, ok)?, printer(krate, params, err)?),
        Ty::Vec(inner) => format!("vec({})", printer(krate, params, inner)?),
        Ty::Map { value, .. } => format!("map({})", printer(krate, params, value)?),
        Ty::Set(_) => "set".into(),
        // Inside a generic type's printer, the printer it was given; a
        // generic function is tested at `i32`.
        Ty::Param(p) if params.contains(p) => format!("p${}", p.as_str()),
        Ty::Param(_) => "int".into(),
        Ty::App { name, args } => format!("((x) => show${}(x, {}))", name.as_str(), each(args)?.join(", ")),
        Ty::Tuple(elems) => format!("tup([{}])", each(elems)?.join(", ")),
        Ty::Ignored { inner, .. } => printer(krate, params, inner)?,
        Ty::Named(n) => match krate.items.iter().find(|i| i.name() == n) {
            Some(Item::Alias(al)) => printer(krate, params, &al.ty)?,
            // Wrapped: the printer may be declared further down.
            Some(Item::Struct(_) | Item::Enum(_)) => format!("((x) => show${}(x))", n.as_str()),
            _ => return None,
        },
        Ty::Fn { .. } | Ty::Never => return None,
    })
}

/// `show$T` for every struct and enum: the TS shape of the value (design/03
/// §2) printed as the Rust side prints it; a generic one takes a printer per
/// type parameter.
pub fn ts_printers(krate: &Crate) -> String {
    let mut out = String::new();
    for item in &krate.items {
        let generics: &[Name] = match item {
            Item::Struct(s) => &s.generics,
            Item::Enum(e) => &e.generics,
            _ => &[],
        };
        // `show$Page = (v, p$T) => ..`: a printer per type parameter.
        let extra: String = generics.iter().map(|g| format!(", p${}", g.as_str())).collect();
        match item {
            Item::Struct(st) => {
                let name = st.name.as_str();
                let body = match st.fields.as_slice() {
                    [] => format!("{name:?}"),
                    [f] if f.name.as_str() == NEWTYPE_FIELD => {
                        format!("`{name}(${{({})(v)}})`", printer_or_fail(krate, generics, &f.ty))
                    }
                    fields => format!("`{name} {{ {} }}`", ts_fields(krate, generics, fields, "v")),
                };
                out.push_str(&format!("const show${name} = (v{extra}) => {body};\n"));
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
                                    format!("${{({})({field})}}", printer_or_fail(krate, generics, t))
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        VariantFields::Struct(fields) => {
                            format!("`{name}::{var} {{ {} }}`", ts_fields(krate, generics, fields, "v"))
                        }
                    };
                    arms.push_str(&format!("    case \"{var}\": return {text};\n"));
                }
                out.push_str(&format!(
                    "const show${name} = (v{extra}) => {{\n  switch (v.kind) {{\n{arms}    default: return `not a {name}: ${{String(v.kind)}}`;\n  }}\n}};\n"
                ));
            }
            Item::Alias(_) | Item::Fn(_) | Item::Const(_) => {}
        }
    }
    out
}

fn ts_fields(krate: &Crate, params: &[Name], fields: &[Field], value: &str) -> String {
    fields
        .iter()
        .map(|f| {
            let n = f.name.as_str();
            format!("{n}: ${{({})({value}.{n})}}", printer_or_fail(krate, params, &f.ty))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A field's printer. A field holds no closure, so this prints one that
/// says so rather than fail the whole driver.
fn printer_or_fail(krate: &Crate, params: &[Name], ty: &Ty) -> String {
    printer(krate, params, ty).unwrap_or_else(|| "(() => \"unprintable\")".into())
}
