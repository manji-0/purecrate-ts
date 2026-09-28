//! Differential harness: each case runs in Rust (this test binary, so debug
//! overflow checks apply) and in the package generated from the same source
//! under node. Results compare as text; a Rust panic must be a TS throw.
//! Needs `node` on PATH; `PURECRATE_SKIP_NODE=1` skips.

use std::fs;
use std::panic::{self, UnwindSafe};
use std::process::Command;

use purecrate_check::accept;
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

/// ASCII only: `{:?}` and a JS string literal agree there.
impl Js for str {
    fn js(&self) -> String {
        format!("{self:?}")
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

/// A result as text, in the same format the TS driver's `show` prints.
/// Floats compare by bit pattern; `-0` is kept distinct from `0`.
pub trait Show {
    const FLOAT: bool = false;
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
    const FLOAT: bool = true;
    fn show(&self) -> String {
        f64::from(*self).to_bits().to_string()
    }
}

impl Show for f64 {
    const FLOAT: bool = true;
    fn show(&self) -> String {
        self.to_bits().to_string()
    }
}

impl Show for &str {
    fn show(&self) -> String {
        self.to_string()
    }
}

impl<T: Show> Show for Option<T> {
    fn show(&self) -> String {
        match self {
            Some(v) => v.show(),
            None => "null".into(),
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

pub struct Case {
    /// Name of the called function.
    pub name: &'static str,
    /// TS call expression.
    pub call: String,
    pub float: bool,
    pub rust: String,
}

pub fn run<T: Show>(
    name: &'static str,
    call: String,
    f: impl FnOnce() -> T + UnwindSafe,
) -> Case {
    let rust = match panic::catch_unwind(f) {
        Ok(v) => v.show(),
        Err(_) => "panic".to_string(),
    };
    Case {
        name,
        call,
        float: T::FLOAT,
        rust,
    }
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

const PRELUDE: &str = r#"import * as pkg from "./src/index.ts";
const show = (x) =>
  x === null
    ? "null"
    : typeof x === "object" && "kind" in x && (x.kind === "Ok" || x.kind === "Err")
      ? `${x.kind}(${show(x.kind === "Ok" ? x.value : x.error)})`
      : typeof x === "object" && "kind" in x
        ? "content" in x
          ? `${x.kind}(${x.content.map(show).join(", ")})`
          : x.kind
      : Object.is(x, -0)
        ? "-0"
        : String(x);
const bits = (x) => String(new BigUint64Array(new Float64Array([x]).buffer)[0]);
const run = (f, wrap) => {
  let r;
  try {
    r = f();
  } catch {
    return "panic";
  }
  return wrap(r);
};
"#;

fn driver(cases: &[Case]) -> String {
    let mut names: Vec<&str> = cases.iter().map(|c| c.name).collect();
    names.sort();
    names.dedup();
    let mut out = String::from(PRELUDE);
    out.push_str(&format!("const {{ {} }} = pkg;\n", names.join(", ")));
    out.push_str("const out = [\n");
    for c in cases {
        let wrap = if c.float { "bits" } else { "show" };
        out.push_str(&format!("  run(() => {}, {wrap}),\n", c.call));
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
    fs::write(dir.join("driver.ts"), driver(cases)).expect("write driver");
    link_purecrate(&dir);

    let output = Command::new("node")
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
