//! Differential harness: each case runs in Rust (this test binary, so debug
//! overflow checks apply) and in the package generated from the same source
//! under node. Results compare as text; a Rust panic must be a TS throw
//! with the same message. Values compare whole, as canonical text derived
//! from the IR on both sides (`Show`, `purecrate_canon::fixture!`). The
//! package must also pass `tsc` with its own strict tsconfig first: node
//! only strips types, so a type error would otherwise go unnoticed.
//! Needs `node` and `npx` on PATH; `PURECRATE_SKIP_NODE=1` skips.

use std::fs;
use std::panic::{self, UnwindSafe};
use std::process::Command;

use purecrate_check::accept;
use purecrate_ir::{Crate, Item, Prim, Ty, VariantFields, NEWTYPE_FIELD};
use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::parse_source;

/// A Rust value as a TS argument expression.
pub trait Js {
    fn js(&self) -> String;
}

macro_rules! js_number {
    ($($t:ty),*) => {$(
        impl Js for $t {
            fn js(&self) -> String {
                self.to_string()
            }
        }
    )*};
}
js_number!(i8, i16, i32, u8, u16, u32, usize, bool);

macro_rules! js_bigint {
    ($($t:ty),*) => {$(
        impl Js for $t {
            fn js(&self) -> String {
                format!("{self}n")
            }
        }
    )*};
}
js_bigint!(i64, u64);

impl Js for f32 {
    fn js(&self) -> String {
        format!("Math.fround({:?})", f64::from(*self))
    }
}

impl Js for f64 {
    fn js(&self) -> String {
        format!("{self:?}")
    }
}

impl<T: Js + ?Sized> Js for &T {
    fn js(&self) -> String {
        (**self).js()
    }
}

/// `{:?}` is a JS string literal too: JS reads its `\u{…}` escapes, and a
/// code point Rust prints as is stays as is.
impl Js for str {
    fn js(&self) -> String {
        format!("{self:?}")
    }
}

impl Js for String {
    fn js(&self) -> String {
        self.as_str().js()
    }
}

/// A one-code-point string; the `Char` brand exists only in types.
impl Js for char {
    fn js(&self) -> String {
        self.to_string().js()
    }
}

impl<T: Js> Js for [T] {
    fn js(&self) -> String {
        format!("[{}]", self.iter().map(Js::js).collect::<Vec<_>>().join(", "))
    }
}

impl<T: Js> Js for Vec<T> {
    fn js(&self) -> String {
        self.as_slice().js()
    }
}

impl<T: Js> Js for Option<T> {
    fn js(&self) -> String {
        match self {
            Some(v) => v.js(),
            None => "null".into(),
        }
    }
}

/// `Box`, `Arc` and `Mutex` are erased in TS.
impl<T: Js + ?Sized> Js for Box<T> {
    fn js(&self) -> String {
        (**self).js()
    }
}

impl<T: Js + ?Sized> Js for std::sync::Arc<T> {
    fn js(&self) -> String {
        (**self).js()
    }
}

impl<T: Js> Js for std::sync::Mutex<T> {
    fn js(&self) -> String {
        self.lock().expect("unpoisoned").js()
    }
}

impl Js for () {
    fn js(&self) -> String {
        "undefined".into()
    }
}

impl<A: Js, B: Js> Js for (A, B) {
    fn js(&self) -> String {
        format!("[{}, {}]", self.0.js(), self.1.js())
    }
}

impl<A: Js, B: Js, C: Js> Js for (A, B, C) {
    fn js(&self) -> String {
        format!("[{}, {}, {}]", self.0.js(), self.1.js(), self.2.js())
    }
}

/// A value as canonical text: the format `purecrate_canon::fixture!` gives
/// every struct and enum, and the TS driver prints from the same IR
/// (`ts_printer`). Floats are their bits, so `-0` and `0` differ.
pub trait Show {
    fn show(&self) -> String;
}

macro_rules! show_plain {
    ($($t:ty),*) => {$(
        impl Show for $t {
            fn show(&self) -> String {
                self.to_string()
            }
        }
    )*};
}
show_plain!(i8, i16, i32, i64, u8, u16, u32, u64, usize, bool);

impl Show for f32 {
    fn show(&self) -> String {
        f64::from(*self).to_bits().to_string()
    }
}

impl Show for f64 {
    fn show(&self) -> String {
        self.to_bits().to_string()
    }
}

/// Printable ASCII but `"` and `\` as is, any other code point `\u{hex}`.
impl Show for str {
    fn show(&self) -> String {
        let mut out = String::from("\"");
        for c in self.chars() {
            if (' '..='~').contains(&c) && c != '"' && c != '\\' {
                out.push(c);
            } else {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
            }
        }
        out.push('"');
        out
    }
}

/// As a string, between `'`: the TS side cannot tell a one-code-point
/// string from a `char` otherwise.
impl Show for char {
    fn show(&self) -> String {
        format!("'{}'", self.to_string().show())
    }
}

impl Show for String {
    fn show(&self) -> String {
        self.as_str().show()
    }
}

impl Show for () {
    fn show(&self) -> String {
        "()".into()
    }
}

impl<T: Show + ?Sized> Show for &T {
    fn show(&self) -> String {
        (**self).show()
    }
}

impl<T: Show + ?Sized> Show for Box<T> {
    fn show(&self) -> String {
        (**self).show()
    }
}

impl<T: Show + ?Sized> Show for std::sync::Arc<T> {
    fn show(&self) -> String {
        (**self).show()
    }
}

impl<T: Show> Show for std::sync::Mutex<T> {
    fn show(&self) -> String {
        self.lock().expect("lock").show()
    }
}

impl<T: Show> Show for Option<T> {
    fn show(&self) -> String {
        match self {
            Some(v) => format!("Some({})", v.show()),
            None => "None".into(),
        }
    }
}

impl<T: Show, E: Show> Show for Result<T, E> {
    fn show(&self) -> String {
        match self {
            Ok(v) => format!("Ok({})", v.show()),
            Err(e) => format!("Err({})", e.show()),
        }
    }
}

impl<T: Show> Show for [T] {
    fn show(&self) -> String {
        format!("[{}]", self.iter().map(Show::show).collect::<Vec<_>>().join(", "))
    }
}

impl<T: Show> Show for Vec<T> {
    fn show(&self) -> String {
        self.as_slice().show()
    }
}

macro_rules! show_tuple {
    ($($t:ident $i:tt),+) => {
        impl<$($t: Show),+> Show for ($($t,)+) {
            fn show(&self) -> String {
                format!("({})", [$(self.$i.show()),+].join(", "))
            }
        }
    };
}
show_tuple!(A 0, B 1);
show_tuple!(A 0, B 1, C 2);
show_tuple!(A 0, B 1, C 2, D 3);

pub struct Case {
    /// Name of the called function.
    pub name: &'static str,
    /// TS call expression.
    pub call: String,
    pub rust: String,
}

pub fn run<T: Show>(
    name: &'static str,
    call: String,
    f: impl FnOnce() -> T + UnwindSafe,
) -> Case {
    let rust = match panic::catch_unwind(f) {
        Ok(v) => v.show(),
        Err(payload) => {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            format!("panic({message})")
        }
    };
    Case { name, call, rust }
}

/// `case!(module::f(a, b))` runs `module::f` in Rust and records the TS call.
macro_rules! case {
    ($m:ident :: $f:ident($($a:expr),*)) => {
        support::run(
            stringify!($f),
            format!("{}({})", stringify!($f), <[String]>::join(&[$(support::Js::js(&$a)),*], ", ")),
            || $m::$f($($a),*),
        )
    };
}

/// Runs the Rust side with panic output silenced.
pub fn quietly<T>(f: impl FnOnce() -> T) -> T {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let out = f();
    panic::set_hook(hook);
    out
}

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
            Prim::Unit => "unit".into(),
            _ => "int".into(),
        },
        Ty::Option(inner) => format!("opt({})", ts_printer(krate, inner)),
        Ty::Result { ok, err } => format!("res({}, {})", ts_printer(krate, ok), ts_printer(krate, err)),
        Ty::Vec(inner) => format!("vec({})", ts_printer(krate, inner)),
        Ty::Tuple(elems) => format!(
            "tup([{}])",
            elems.iter().map(|t| ts_printer(krate, t)).collect::<Vec<_>>().join(", ")
        ),
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
                                .map(|(i, t)| format!("${{({})(v.content[{i}])}}", ts_printer(krate, t)))
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
            Item::Alias(_) | Item::Fn(_) => {}
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

fn driver(krate: &Crate, cases: &[Case]) -> String {
    let mut names: Vec<&str> = cases.iter().map(|c| c.name).collect();
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

pub fn link_purecrate(dir: &std::path::Path) {
    let boundary = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/boundary");
    let modules = dir.join("node_modules/purecrate");
    fs::create_dir_all(modules.parent().expect("node_modules")).expect("mkdir node_modules");
    let _ = fs::remove_file(&modules);
    std::os::unix::fs::symlink(&boundary, &modules).expect("link purecrate");
}

/// TypeScript majors the generated package must type-check under: 6 is the
/// last JS compiler, 7 the native one. `scripts/verify.sh` uses the same list.
pub const TS_MAJORS: &[&str] = &["6", "7"];

/// Resolves the runtime packages in this repository to their sources; their
/// `exports` point at `dist`, which is a build output.
pub const SOURCE_CONDITION: &str = purecrate_pack::SOURCE_CONDITION;

/// `tsc -p` over the generated package under each of `TS_MAJORS`.
///
/// npx runs from an empty directory: from `dir`, it would find the
/// `typescript` that a linked adapter package (`node_modules/purecrate-zod`)
/// has installed, count `typescript@7` as present, and never link `tsc`.
pub fn typecheck(dir: &std::path::Path) {
    let neutral = std::env::temp_dir().join("purecrate-npx");
    std::fs::create_dir_all(&neutral).expect("create npx directory");
    for major in TS_MAJORS {
        let project = dir.to_str().expect("utf-8 path");
        let output = Command::new("npx")
            .args(["-y", "-p", &format!("typescript@{major}"), "tsc", "-p", project, "--customConditions", SOURCE_CONDITION])
            .current_dir(&neutral)
            .output()
            .expect("run npx tsc (set PURECRATE_SKIP_NODE=1 to skip)");
        assert!(
            output.status.success(),
            "tsc {major} rejects the generated package in {}:\n{}{}",
            dir.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Accepts `source`, generates its package, and checks every case agrees.
pub fn assert_equivalent(crate_name: &str, source: &str, cases: &[Case]) {
    assert!(
        cfg!(debug_assertions),
        "the Rust baseline needs overflow checks; run without --release"
    );
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        eprintln!("PURECRATE_SKIP_NODE set: skipping TS equivalence");
        return;
    }
    let krate = parse_source(crate_name, source).unwrap_or_else(|e| panic!("parse {crate_name}: {e}"));
    let typed = accept(&krate).unwrap_or_else(|d| panic!("{crate_name} rejected: {d:#?}"));
    let dir = std::env::temp_dir().join(format!("purecrate-{crate_name}-eq-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    for file in assemble(&typed).files {
        let path = dir.join(disk_path(&file.stem));
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, file.source).expect("write");
    }
    fs::write(dir.join("driver.ts"), driver(&krate, cases)).expect("write driver");
    link_purecrate(&dir);
    typecheck(&dir);

    let output = Command::new("node")
        .arg(format!("--conditions={SOURCE_CONDITION}"))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("run node (set PURECRATE_SKIP_NODE=1 to skip)");
    assert!(
        output.status.success(),
        "node failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let actual: Vec<&str> = stdout.lines().collect();
    assert_eq!(actual.len(), cases.len());
    let mismatches: Vec<String> = cases
        .iter()
        .zip(&actual)
        .filter(|(c, got)| c.rust != **got)
        .map(|(c, got)| format!("{}: rust {} / ts {got}", c.call, c.rust))
        .collect();
    fs::remove_dir_all(&dir).ok();
    assert!(mismatches.is_empty(), "mismatches:\n{}", mismatches.join("\n"));
}
