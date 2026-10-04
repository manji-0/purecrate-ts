//! What every generated equivalence test shares: the seeds, the Rust
//! baseline (compiled by `rustc` with debug-build checks and run once), and
//! the comparison with the generated package.

use std::fs;
use std::process::Command;

use super::Case;

/// Seeds checked on every run; any seed may be drawn with
/// `PURECRATE_GEN_SEED`.
pub const SEEDS: &[u64] = &[1, 2, 3, 4, 5, 6, 7, 8];

/// The seeds a test draws: `PURECRATE_GEN_SEED` alone, or `defaults`.
pub fn seeds(defaults: &[u64]) -> Vec<u64> {
    match std::env::var("PURECRATE_GEN_SEED").ok().and_then(|s| s.parse().ok()) {
        Some(seed) => vec![seed],
        None => defaults.to_vec(),
    }
}

/// How many functions a seed draws: `PURECRATE_GEN_FNS`, or `default`.
pub fn fn_count(default: usize) -> usize {
    std::env::var("PURECRATE_GEN_FNS").ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Writes the generated crate to `PURECRATE_GEN_DUMP`, if set, before
/// anything can fail on it.
pub fn dump(source: &str) {
    if let Some(path) = std::env::var_os("PURECRATE_GEN_DUMP") {
        fs::write(path, source).expect("write PURECRATE_GEN_DUMP");
    }
}

/// What every baseline starts with: `Show` for the generated types, and
/// `run`, which prints a result as `support::Show` would, or
/// `panic(message)`.
pub const SHOW_PRELUDE: &str = r#"
trait Show { fn show(&self) -> String; }
impl Show for i32 { fn show(&self) -> String { self.to_string() } }
impl Show for bool { fn show(&self) -> String { self.to_string() } }
impl<T: Show> Show for Option<T> {
    fn show(&self) -> String { match self { Some(v) => format!("Some({})", v.show()), None => "None".into() } }
}
impl<T: Show, E: Show> Show for Result<T, E> {
    fn show(&self) -> String { match self { Ok(v) => format!("Ok({})", v.show()), Err(e) => format!("Err({})", e.show()) } }
}
fn run<T: Show>(f: impl FnOnce() -> T + std::panic::UnwindSafe) -> String {
    match std::panic::catch_unwind(f) {
        Ok(v) => v.show(),
        Err(p) => {
            let m = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            format!("panic({})", m.unwrap_or_default())
        }
    }
}
"#;

/// One row of arguments, as Rust source and as TS.
pub struct Args {
    pub rust: String,
    pub js: String,
}

/// Runs `fns` of `source` in Rust (with `show`, `Show` for the types
/// beyond `SHOW_PRELUDE`'s) and in the generated TS on each of `rows`, and
/// asserts they agree; on a failure, prints the source.
pub fn compare_rows(crate_name: &str, seeds: &[u64], source: &str, show: &str, fns: &[String], rows: &[Args]) {
    dump(source);
    let mut program =
        format!("{source}{SHOW_PRELUDE}{show}fn main() {{\n    std::panic::set_hook(Box::new(|_| {{}}));\n");
    for row in rows {
        for name in fns {
            program.push_str(&format!("    println!(\"{{}}\", run(|| {name}({})));\n", row.rust));
        }
    }
    program.push_str("}\n");
    let rust = rust_lines(&program);
    assert_eq!(rust.len(), rows.len() * fns.len());
    let mut results = rust.into_iter();
    let mut cases = Vec::new();
    for row in rows {
        for name in fns {
            cases.push(Case {
                name: Box::leak(name.clone().into_boxed_str()),
                call: format!("{}({})", purecrate_ir::to_camel(name), row.js),
                rust: results.next().expect("a result per case"),
            });
        }
    }
    check_cases(crate_name, seeds, source, &cases);
}

/// Compiles `program` with debug-build checks, runs it, and returns the
/// lines it prints.
pub fn rust_lines(program: &str) -> Vec<String> {
    let dir = super::scratch("generated-rust");
    let main = dir.join("main.rs");
    fs::write(&main, program).expect("write harness");
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = Command::new(rustc)
        .args([
            "--edition",
            "2021",
            "-C",
            "overflow-checks=on",
            "-C",
            "debug-assertions=on",
            "--cap-lints",
            "allow",
            "-o",
        ])
        .arg(dir.join("main"))
        .arg(&main)
        .output()
        .expect("run rustc");
    assert!(out.status.success(), "rustc rejects the harness:\n{}", String::from_utf8_lossy(&out.stderr));
    let run = Command::new(dir.join("main")).output().expect("run harness");
    assert!(run.status.success(), "harness failed:\n{}", String::from_utf8_lossy(&run.stderr));
    fs::remove_dir_all(&dir).ok();
    String::from_utf8(run.stdout).expect("utf8").lines().map(str::to_string).collect()
}

/// Asserts the generated TS agrees with `cases`; on a failure, prints the
/// source.
pub fn check_cases(crate_name: &str, seeds: &[u64], source: &str, cases: &[Case]) {
    let label = format!("{crate_name} (seeds {seeds:?}; PURECRATE_GEN_SEED reruns one)");
    // A sweep (`PURECRATE_GEN_SEED`) may ask for the values alone with
    // `PURECRATE_GEN_TYPES=report`: what tsc refuses is printed, and node
    // still runs. The seeds every run checks always type-check.
    let report = std::env::var_os("PURECRATE_GEN_SEED").is_some()
        && std::env::var("PURECRATE_GEN_TYPES").is_ok_and(|v| v == "report");
    let run = || {
        if report {
            if !super::assert_values_equivalent(crate_name, source, cases) {
                eprintln!("{label}: values agree; tsc refuses the package");
            }
        } else {
            super::assert_equivalent(crate_name, source, cases);
        }
    };
    if let Err(e) = std::panic::catch_unwind(run) {
        eprintln!("{label}\n{source}");
        std::panic::resume_unwind(e);
    }
}
