//! Numeric semantics: every case in `fixtures/arith.rs` gives the same result
//! (or panics where the TS throws) in Rust and in the generated package.
//! The Rust side is this test binary, so it needs overflow checks on (the
//! default `test` profile). Needs `node` on PATH; `PURECRATE_SKIP_NODE=1`
//! skips.

use std::fs;
use std::panic::{self, UnwindSafe};
use std::process::Command;

use purecrate_check::accept;
use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::parse_source;

#[allow(dead_code)]
mod arith {
    include!("fixtures/arith.rs");
}

const SOURCE: &str = include_str!("fixtures/arith.rs");

trait Js {
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
js_number!(i8, i16, i32, u8, u16, u32);

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

/// Results are compared as text. Floats compare by bit pattern; `-0` is kept
/// distinct from `0`.
trait Show {
    const FLOAT: bool = false;
    fn show(&self) -> String;
}

macro_rules! show_int {
    ($($t:ty),*) => {$(
        impl Show for $t {
            fn show(&self) -> String {
                self.to_string()
            }
        }
    )*};
}
show_int!(i8, i16, i32, i64, u8, u16, u32, u64, bool);

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

struct Case {
    js: String,
    rust: String,
}

fn run<T: Show>(js_call: String, f: impl FnOnce() -> T + UnwindSafe) -> Case {
    let rust = match panic::catch_unwind(f) {
        Ok(v) => v.show(),
        Err(_) => "panic".to_string(),
    };
    let js = if T::FLOAT {
        format!("bits({js_call})")
    } else {
        format!("show({js_call})")
    };
    Case { js, rust }
}

macro_rules! case {
    ($f:ident($($a:expr),*)) => {
        run(
            format!("{}({})", stringify!($f), <[String]>::join(&[$(Js::js(&$a)),*], ", ")),
            || arith::$f($($a),*),
        )
    };
}

fn cases() -> Vec<Case> {
    vec![
        case!(add_i32(1i32, 2i32)),
        case!(add_i32(i32::MAX, 1i32)),
        case!(sub_i32(i32::MIN, 1i32)),
        case!(mul_i32(46341i32, 46341i32)),
        case!(mul_i32(-46340i32, 46340i32)),
        case!(mul_i32(65536i32, 65536i32)),
        case!(div_i32(7i32, 2i32)),
        case!(div_i32(-7i32, 2i32)),
        case!(div_i32(7i32, 0i32)),
        case!(div_i32(i32::MIN, -1i32)),
        case!(div_i32(-1i32, 2i32)),
        case!(rem_i32(-7i32, 2i32)),
        case!(rem_i32(7i32, -2i32)),
        case!(rem_i32(-4i32, 2i32)),
        case!(rem_i32(7i32, 0i32)),
        case!(rem_i32(i32::MIN, -1i32)),
        case!(neg_i32(5i32)),
        case!(neg_i32(0i32)),
        case!(neg_i32(i32::MIN)),
        case!(half_i32(7i32)),
        case!(half_i32(-7i32)),
        case!(min_i32()),
        case!(add_u8(200u8, 55u8)),
        case!(add_u8(200u8, 56u8)),
        case!(sub_u8(0u8, 1u8)),
        case!(div_i8(i8::MIN, -1i8)),
        case!(rem_i8(i8::MIN, -1i8)),
        case!(mul_u32(65535u32, 65537u32)),
        case!(mul_u32(65536u32, 65536u32)),
        case!(mul_u32(4294967295u32, 4294967295u32)),
        case!(add_i64(i64::MAX, 0i64)),
        case!(add_i64(i64::MAX, 1i64)),
        case!(mul_i64(3037000499i64, 3037000499i64)),
        case!(mul_i64(3037000500i64, 3037000500i64)),
        case!(div_i64(-7i64, 2i64)),
        case!(div_i64(1i64, 0i64)),
        case!(div_i64(i64::MIN, -1i64)),
        case!(rem_i64(-7i64, 2i64)),
        case!(rem_i64(i64::MIN, -1i64)),
        case!(neg_i64(i64::MIN)),
        case!(sub_u64(0u64, 1u64)),
        case!(sub_u64(u64::MAX, 1u64)),
        case!(affine_i64(4i64)),
        case!(affine_i64(3074457345618258602i64)),
        case!(affine_i64(3074457345618258603i64)),
        case!(small_i64(9i64)),
        case!(small_i64(10i64)),
        case!(scaled(1i32)),
        case!(scaled(-1i32)),
        case!(grouped_i32(1i32, 2i32, 3i32)),
        case!(avg_f32(0.1f32, 0.2f32)),
        case!(avg_f32(f32::MAX, f32::MAX)),
        case!(third_f32(1.0f32)),
        case!(poly_f64(3.0f64)),
        case!(poly_f64(0.1f64)),
        case!(div_f64(1.0f64, 0.0f64)),
        case!(div_f64(-1.0f64, 3.0f64)),
    ]
}

const PRELUDE: &str = r#"import * as arith from "./src/index.ts";
const show = (x) => (Object.is(x, -0) ? "-0" : String(x));
const bits = (x) => String(new BigUint64Array(new Float64Array([x]).buffer)[0]);
const run = (f) => {
  try {
    return f();
  } catch {
    return "panic";
  }
};
const {
"#;

fn driver(cases: &[Case]) -> String {
    let names: Vec<&str> = cases
        .iter()
        .map(|c| {
            let start = c.js.find('(').expect("call") + 1;
            let end = c.js[start..].find('(').expect("call") + start;
            &c.js[start..end]
        })
        .collect();
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    let mut out = String::from(PRELUDE);
    out.push_str(&format!("  {}\n}} = arith;\n", unique.join(",\n  ")));
    out.push_str("const out = [\n");
    for c in cases {
        // `show`/`bits` must not swallow the throw, so the call is deferred.
        let (wrap, call) = c.js.split_once('(').expect("wrapper");
        let call = &call[..call.len() - 1];
        out.push_str(&format!(
            "  (() => {{ const r = run(() => {call}); return r === \"panic\" ? r : {wrap}(r); }})(),\n"
        ));
    }
    out.push_str("];\nconsole.log(out.join(\"\\n\"));\n");
    out
}

#[test]
fn generated_arithmetic_matches_rust_debug_build() {
    assert!(
        cfg!(debug_assertions),
        "the Rust baseline needs overflow checks; run without --release"
    );
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        eprintln!("PURECRATE_SKIP_NODE set: skipping TS equivalence");
        return;
    }

    let hook = panic::take_hook();
    panic::set_hook(Box::new(|_| {}));
    let cases = cases();
    panic::set_hook(hook);

    let krate = parse_source("arith", SOURCE).expect("parse arith");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("arith rejected: {d:#?}"));
    let dir = std::env::temp_dir().join(format!("purecrate-arith-eq-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    for file in assemble(&typed).files {
        let path = dir.join(disk_path(&file.stem));
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, file.source).expect("write");
    }
    fs::write(dir.join("driver.ts"), driver(&cases)).expect("write driver");

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
        .map(|(c, got)| format!("{}: rust {} / ts {got}", c.js, c.rust))
        .collect();
    fs::remove_dir_all(&dir).ok();
    assert!(mismatches.is_empty(), "mismatches:\n{}", mismatches.join("\n"));
}
