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
use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::parse_source;

pub mod corpus;
mod driver;
pub mod generated;
mod values;

use driver::driver;
pub use values::{Js, Show};

pub struct Case {
    /// Name of the called function.
    pub name: &'static str,
    /// TS call expression.
    pub call: String,
    pub rust: String,
}

pub fn run<T: Show>(name: &'static str, call: String, f: impl FnOnce() -> T + UnwindSafe) -> Case {
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
            // The TS spelling of the function (`check::rename`).
            format!("{}({})", purecrate_ir::to_camel(stringify!($f)), <[String]>::join(&[$(support::Js::js(&$a)),*], ", ")),
            || $m::$f($($a),*),
        )
    };
}

thread_local! {
    static QUIET: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Inputs for randomized cases: SplitMix64 from a fixed seed, so every run
/// draws the same cases and a failure names an input that reproduces.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`.
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    pub fn pick<T: Clone>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize].clone()
    }

    /// An integer of `bits` bits, half the time near an edge of the width
    /// (0, the maximum, the minimum, ±1 from them), where overflow lives.
    pub fn edgy(&mut self, bits: u32, signed: bool) -> i128 {
        let (lo, hi) =
            if signed { (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1) } else { (0, (1i128 << bits) - 1) };
        if self.below(2) == 0 {
            let edge = self.pick(&[lo, lo + 1, -1, 0, 1, hi - 1, hi]);
            edge.clamp(lo, hi)
        } else {
            lo + (self.next() as i128).rem_euclid(hi - lo + 1)
        }
    }
}

/// Runs the Rust side with panic output silenced on this thread. The hook is
/// installed once for the whole binary and asks a thread-local flag: tests
/// run in parallel threads of one process, and swapping the process-wide
/// hook per call would let one test restore another's silent hook.
pub fn quietly<T>(f: impl FnOnce() -> T) -> T {
    static HOOK: std::sync::Once = std::sync::Once::new();
    HOOK.call_once(|| {
        let default = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            if !QUIET.with(|q| q.get()) {
                default(info);
            }
        }));
    });
    // Restored on drop: a panic that escapes `f`, or a nested `quietly`,
    // leaves the flag as it found it.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            QUIET.with(|q| q.set(self.0));
        }
    }
    let _restore = Restore(QUIET.with(|q| q.replace(true)));
    f()
}

/// A fresh directory for one test. The tests share one process, so the pid
/// alone would not tell two of them apart: a counter does.
pub fn scratch(tag: &str) -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("purecrate-{tag}-{}-{n}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    fs::create_dir_all(&dir).expect("mkdir scratch");
    dir
}

/// The `tsc` of a TypeScript major, resolved through npx once per run and
/// then run directly: npx costs about half a second a call, and parallel
/// calls would race to install into its cache.
pub fn tsc(major: &str) -> &'static std::path::Path {
    static RESOLVED: [std::sync::OnceLock<std::path::PathBuf>; TS_MAJORS.len()] =
        [const { std::sync::OnceLock::new() }; TS_MAJORS.len()];
    let i = TS_MAJORS.iter().position(|m| *m == major).expect("a supported TypeScript major");
    RESOLVED[i].get_or_init(|| {
        // From an empty directory: from a package dir, npx would find the
        // `typescript` a linked adapter installed and never link `tsc`.
        let neutral = std::env::temp_dir().join("purecrate-npx");
        fs::create_dir_all(&neutral).expect("create npx directory");
        let out = Command::new("npx")
            .args(["-y", "-p", &format!("typescript@{major}"), "-c", "command -v tsc"])
            .current_dir(&neutral)
            .output()
            .expect("run npx (set PURECRATE_SKIP_NODE=1 to skip)");
        assert!(out.status.success(), "resolve tsc {major}: {}", String::from_utf8_lossy(&out.stderr));
        std::path::PathBuf::from(String::from_utf8(out.stdout).expect("utf-8").trim())
    })
}

/// The repository's root.
pub fn repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The file `stem` of the package generated from `source` (no schema).
pub fn emitted(name: &str, source: &str, stem: &str) -> String {
    let krate = parse_source(name, source).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("{name} rejected: {d:#?}"));
    assemble(&typed).files.into_iter().find(|f| f.stem == stem).unwrap_or_else(|| panic!("missing {stem}")).source
}

/// Writes each file of `package` under `dir`.
pub fn write_package(dir: &std::path::Path, package: &purecrate_emit_ts::Package) {
    for file in &package.files {
        let path = dir.join(disk_path(&file.stem));
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, &file.source).expect("write");
    }
}

/// `dir/node_modules/<name>` as a link to `packages/<rel>` in this
/// repository.
pub fn link(dir: &std::path::Path, name: &str, rel: &str) {
    let target = repo().join("packages").join(rel);
    let modules = dir.join("node_modules").join(name);
    fs::create_dir_all(modules.parent().expect("node_modules")).expect("mkdir");
    let _ = fs::remove_file(&modules);
    std::os::unix::fs::symlink(&target, &modules).unwrap_or_else(|e| panic!("link {name}: {e} ({target:?})"));
}

/// TypeScript majors the generated package must type-check under: 6 is the
/// last JS compiler, 7 the native one. `scripts/verify.sh` uses the same list.
pub const TS_MAJORS: &[&str] = &["6", "7"];

/// Resolves the runtime packages in this repository to their sources; their
/// `exports` point at `dist`, which is a build output.
pub const SOURCE_CONDITION: &str = purecrate_pack::SOURCE_CONDITION;

/// `tsc -p` over the generated package under each of `TS_MAJORS`.
pub fn typecheck(dir: &std::path::Path) {
    assert!(types_checked(dir), "tsc rejects the generated package in {}", dir.display());
}

/// Whether `tsc -p` accepts the generated package under each of
/// `TS_MAJORS`; what it refuses is printed.
fn types_checked(dir: &std::path::Path) -> bool {
    let mut ok = true;
    for major in TS_MAJORS {
        let project = dir.to_str().expect("utf-8 path");
        let output = Command::new(tsc(major))
            .args(["-p", project, "--customConditions", SOURCE_CONDITION])
            .output()
            .expect("run tsc");
        if !output.status.success() {
            eprintln!(
                "tsc {major} rejects the generated package in {}:\n{}{}",
                dir.display(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            ok = false;
        }
    }
    ok
}

/// Accepts `source`, generates its package, and checks every case agrees.
pub fn assert_equivalent(crate_name: &str, source: &str, cases: &[Case]) {
    equivalent(crate_name, source, cases, true);
}

/// As `assert_equivalent`, with what tsc refuses printed instead of
/// failing, so the values are still compared; whether tsc accepted the
/// package. Only for sweeps over generated seeds.
pub fn assert_values_equivalent(crate_name: &str, source: &str, cases: &[Case]) -> bool {
    equivalent(crate_name, source, cases, false)
}

#[allow(clippy::assertions_on_constants, reason = "a guard against running the tests in release")]
fn equivalent(crate_name: &str, source: &str, cases: &[Case], types_required: bool) -> bool {
    assert!(cfg!(debug_assertions), "the Rust baseline needs overflow checks; run without --release");
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        eprintln!("PURECRATE_SKIP_NODE set: skipping TS equivalence");
        return true;
    }
    let krate = parse_source(crate_name, source).unwrap_or_else(|e| panic!("parse {crate_name}: {e}"));
    let typed = accept(&krate).unwrap_or_else(|d| panic!("{crate_name} rejected: {d:#?}"));
    let dir = scratch(&format!("{crate_name}-eq"));
    write_package(&dir, &assemble(&typed));
    fs::write(dir.join("driver.ts"), driver(&krate, cases)).expect("write driver");
    let types_ok = if types_required {
        typecheck(&dir);
        true
    } else {
        types_checked(&dir)
    };

    let output = Command::new("node")
        .arg(format!("--conditions={SOURCE_CONDITION}"))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("run node (set PURECRATE_SKIP_NODE=1 to skip)");
    assert!(output.status.success(), "node failed:\n{}", String::from_utf8_lossy(&output.stderr));
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
    types_ok
}
