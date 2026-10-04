//! Expressions generated from a seed, not written by hand: each function
//! nests integer, `bool`, `Option`, and `Result` operators, methods,
//! closures, `if`, `match`, and `?` in shapes no fixture spells. The Rust
//! side is compiled by `rustc` with debug-build checks and run once per
//! input; the TS side is the generated package, compared case by case as
//! every other equivalence test is.
//!
//! The audits of 0.8.1, 0.9.0, and 0.9.1 each found output that disagreed
//! with Rust only in shapes the fixtures lacked (a `?:` under `!== null`, a
//! `?` inside `ok_or(..)?`, a guard reading a shadowed name). This test
//! looks for that class mechanically. `PURECRATE_GEN_SEED` (with
//! `PURECRATE_GEN_FNS`) draws one other set instead of `SEEDS`;
//! `PURECRATE_GEN_DUMP=<file>` writes the generated crate there, for
//! `purecrate-ts build`.
//!
//! `SEEDS` run on every `cargo test`; seeds 1..=120 have all passed, with
//! no value disagreeing with Rust (again after 0.10.0, whose `matches!`
//! no longer draws a `?`). A sweep may set `PURECRATE_GEN_TYPES=report`
//! with `PURECRATE_GEN_SEED`: what tsc refuses is printed, and the values
//! are still compared. Getting there took the runtime's
//! `Result.ok` / `Result.err` defaults (`E = never`, `T = never`), so a
//! lone `Result.ok(v)` is not `Result<T, unknown>`, and folding what TS
//! has narrowed (`join.rs`: a `match` on a place an arm or a `?` has
//! decided, a constructor's `match`, the bindings and `let`s that leaves
//! unread), which TS otherwise refuses as "no overlap" or an unread
//! `const`.
//!
//! Integer literals are suffixed and `Option` / `Result` constructors are
//! kept out of receivers and tests: unsuffixed, rustc cannot type them;
//! as receivers they fold to `a ?? b` with `a` never nullish, and constant
//! tests to unreachable code, which TS refuses for reasons that are not
//! about equivalence.

use std::fmt::Write as _;
use std::fs;
use std::process::Command;

use crate::support::{self, Case, Js, Rng};

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Ty {
    Int,
    Bool,
    Opt,
    Res,
}

impl Ty {
    pub(crate) fn rust(self) -> &'static str {
        match self {
            Ty::Int => "i32",
            Ty::Bool => "bool",
            Ty::Opt => "Option<i32>",
            Ty::Res => "Result<i32, i32>",
        }
    }
}

pub(crate) struct Gen {
    pub(crate) rng: Rng,
    /// Names in scope, innermost last, and whether each is a `let mut`.
    pub(crate) scope: Vec<(String, Ty, bool)>,
    /// What `?` leaves the function with: `Opt`, `Res`, or neither.
    pub(crate) ret: Ty,
    /// Inside a closure, where `?` would leave the closure instead, or
    /// inside `&&`, `||`, `if`, or `match`, where v0 refuses it
    /// (`[expr/position]`).
    pub(crate) no_try: usize,
    /// Inside a closure, which reads only immutable bindings.
    pub(crate) closure: usize,
    pub(crate) next_name: usize,
}

impl Gen {
    pub(crate) fn new(seed: u64) -> Self {
        Gen { rng: Rng::new(seed), scope: Vec::new(), ret: Ty::Int, no_try: 0, closure: 0, next_name: 0 }
    }

    /// The parameters every generated function takes ([`PARAMS`]).
    pub(crate) fn params() -> Vec<(String, Ty, bool)> {
        [("a", Ty::Int), ("b", Ty::Int), ("o", Ty::Opt), ("r", Ty::Res), ("c", Ty::Bool)]
            .into_iter()
            .map(|(n, t)| (n.to_string(), t, false))
            .collect()
    }

    pub(crate) fn fresh(&mut self) -> String {
        // Now and then a name that shadows a parameter, as a closure or arm
        // in Rust may.
        if self.rng.below(5) == 0 {
            return self.rng.pick(&["a", "b"]).to_string();
        }
        self.next_name += 1;
        format!("v{}", self.next_name)
    }

    pub(crate) fn var(&mut self, ty: Ty) -> Option<String> {
        self.var_where(ty, false)
    }

    /// A name of type `ty` in scope; with `mutable`, only a `let mut`.
    pub(crate) fn var_where(&mut self, ty: Ty, mutable: bool) -> Option<String> {
        let in_closure = self.closure > 0;
        let names: Vec<String> = self
            .scope
            .iter()
            .enumerate()
            // The innermost binding of a name is the one in scope.
            .filter(|(i, (n, t, _))| *t == ty && !self.scope[i + 1..].iter().any(|(m, _, _)| m == n))
            .filter(|(_, (_, _, m))| if mutable { *m } else { !(in_closure && *m) })
            .map(|(_, (n, _, _))| n.clone())
            .collect();
        if names.is_empty() {
            None
        } else {
            Some(self.rng.pick(&names))
        }
    }

    fn with<T>(&mut self, name: &str, ty: Ty, f: impl FnOnce(&mut Self) -> T) -> T {
        self.scope.push((name.to_string(), ty, false));
        let out = f(self);
        self.scope.pop();
        out
    }

    pub(crate) fn without_try<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.no_try += 1;
        let out = f(self);
        self.no_try -= 1;
        out
    }

    /// A closure's body: no `?`, and no `let mut` read.
    fn closure<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.closure += 1;
        let out = self.without_try(f);
        self.closure -= 1;
        out
    }

    pub(crate) fn can_try(&self, ty: Ty) -> bool {
        self.no_try == 0 && self.ret == ty
    }

    pub(crate) fn lit(&mut self) -> String {
        let n = self.rng.pick(&[0i64, 1, 2, 3, 7, 10, 100, 65535, 2147483647]);
        // Suffixed: a literal receiver, or one a closure reads, would leave
        // rustc without a type.
        if self.rng.below(4) == 0 && n != 0 {
            format!("(-{n}i32)")
        } else {
            format!("{n}i32")
        }
    }

    /// An `Option` or `Result` in a receiver or scrutinee position. Not a
    /// constructor (`Some(a).unwrap_or(b)` is `a ?? b`, which TS refuses as
    /// never nullish, and nobody writes it); a choice of values leaves rustc
    /// without its type, so it is bound to an annotated local first.
    pub(crate) fn place(&mut self, ty: Ty, depth: u32) -> String {
        let built = |e: &str| ["None", "Some(", "Ok(", "Err("].iter().any(|p| e.starts_with(p));
        let bare = |e: &str| ["(if ", "(match "].iter().any(|p| e.starts_with(p));
        let mut e = self.expr(ty, depth);
        for _ in 0..8 {
            if !built(&e) {
                break;
            }
            e = self.expr(ty, depth);
        }
        if built(&e) {
            e = if ty == Ty::Opt { "o" } else { "r" }.to_string();
        }
        // A block inside a larger expression may not hold a `?`.
        if bare(&e) && e.contains('?') {
            e = self.without_try(|g| g.expr(ty, depth));
        }
        if bare(&e) {
            self.next_name += 1;
            let t = format!("t{}", self.next_name);
            format!("{{ let {t}: {} = {e}; {t} }}", ty.rust())
        } else {
            e
        }
    }

    pub(crate) fn expr(&mut self, ty: Ty, depth: u32) -> String {
        match ty {
            Ty::Int => self.int(depth),
            Ty::Bool => self.boolean(depth),
            Ty::Opt => self.opt(depth),
            Ty::Res => self.res(depth),
        }
    }

    pub(crate) fn int(&mut self, depth: u32) -> String {
        if depth == 0 || self.rng.below(6) == 0 {
            return match self.var(Ty::Int) {
                Some(v) if self.rng.below(3) != 0 => v,
                _ => self.lit(),
            };
        }
        let d = depth - 1;
        match self.rng.below(22) {
            0..=3 => {
                let op = self.rng.pick(&["+", "-", "*", "/", "%", "&", "|", "^"]);
                format!("({} {op} {})", self.int(d), self.int(d))
            }
            4 => {
                let k = self.rng.pick(&[0, 1, 3, 31]);
                let op = self.rng.pick(&["<<", ">>"]);
                format!("({} {op} {k})", self.int(d))
            }
            5 => format!("(-{})", self.int(d)),
            6 => {
                let m =
                    self.rng.pick(&["min", "max", "wrapping_add", "wrapping_mul", "saturating_sub", "saturating_add"]);
                format!("({}).{m}({})", self.int(d), self.int(d))
            }
            7 => format!("({}).abs()", self.int(d)),
            8 => format!("({}).pow({})", self.int(d), self.rng.pick(&[0, 1, 2, 3])),
            9 => format!("({}).unwrap_or({})", self.place(Ty::Opt, d), self.int(d)),
            10 => format!("({}).ok().unwrap_or({})", self.place(Ty::Res, d), self.int(d)),
            11..=13 => self.without_try(|g| g.int_branch(d)),
            14 => self.without_try(|g| {
                let v = g.fresh();
                let value = g.int(d);
                let body = g.with(&v, Ty::Int, |g| g.int(d));
                format!("{{ let {v} = {value}; {body} }}")
            }),
            15 => {
                let m = self.rng.pick(&["checked_add", "checked_sub", "checked_mul", "checked_div"]);
                format!("({}).{m}({}).unwrap_or({})", self.int(d), self.int(d), self.int(d))
            }
            16 | 17 if self.can_try(Ty::Opt) => format!("{}?", self.place(Ty::Opt, d)),
            16 | 17 if self.can_try(Ty::Res) => match self.rng.below(3) {
                0 => format!("({}).ok_or({})?", self.place(Ty::Opt, d), self.int(d)),
                1 => {
                    let e = self.fresh();
                    let res = self.place(Ty::Res, d);
                    let body = self.closure(|g| g.with(&e, Ty::Int, |g| g.int(d)));
                    format!("({res}).map_err(|{e}| {body})?")
                }
                _ => format!("{}?", self.place(Ty::Res, d)),
            },
            18 => {
                let m = self.rng.pick(&["checked_add", "checked_mul"]);
                if self.can_try(Ty::Opt) {
                    format!("({}).{m}({})?", self.int(d), self.int(d))
                } else if self.can_try(Ty::Res) {
                    format!("({}).{m}({}).ok_or({})?", self.int(d), self.int(d), self.int(d))
                } else {
                    format!("({}).{m}({}).unwrap_or(0)", self.int(d), self.int(d))
                }
            }
            _ => self.int(d),
        }
    }

    /// An `if` or `match` giving an integer.
    fn int_branch(&mut self, d: u32) -> String {
        match self.rng.below(3) {
            0 => format!("(if {} {{ {} }} else {{ {} }})", self.boolean(d), self.int(d), self.int(d)),
            1 => {
                let scrutinee = self.place(Ty::Opt, d);
                let v = self.fresh();
                let some = self.with(&v, Ty::Int, |g| g.int(d));
                let none = self.int(d);
                format!("(match {scrutinee} {{ Some({v}) => {some}, None => {none} }})")
            }
            _ => {
                let scrutinee = self.place(Ty::Res, d);
                let v = self.fresh();
                let ok = self.with(&v, Ty::Int, |g| g.int(d));
                let e = self.fresh();
                let err = self.with(&e, Ty::Int, |g| g.int(d));
                format!("(match {scrutinee} {{ Ok({v}) => {ok}, Err({e}) => {err} }})")
            }
        }
    }

    pub(crate) fn boolean(&mut self, depth: u32) -> String {
        if depth == 0 || self.rng.below(6) == 0 {
            // `c`, not `true`: a constant test folds away, and what TS then
            // reports (unreachable code) is not about equivalence.
            return self.var(Ty::Bool).unwrap_or_else(|| "c".to_string());
        }
        let d = depth - 1;
        match self.rng.below(14) {
            0..=2 => {
                let op = self.rng.pick(&["<", "<=", ">", ">=", "==", "!="]);
                format!("({} {op} {})", self.int(d), self.int(d))
            }
            3 => format!("(!{})", self.boolean(d)),
            4 | 5 => {
                let op = self.rng.pick(&["&&", "||", "==", "!="]);
                self.without_try(|g| format!("({} {op} {})", g.boolean(d), g.boolean(d)))
            }
            6 => format!("({}).is_some()", self.place(Ty::Opt, d)),
            7 => format!("({}).is_none()", self.place(Ty::Opt, d)),
            8 => format!("({}).ok().is_some()", self.place(Ty::Res, d)),
            9 => {
                let v = self.fresh();
                let opt = self.place(Ty::Opt, d);
                let body = self.closure(|g| g.with(&v, Ty::Int, |g| g.int(d)));
                format!("({opt}).map(|{v}| {body}).is_none()")
            }
            10 => self
                .without_try(|g| format!("(if {} {{ {} }} else {{ {} }})", g.boolean(d), g.boolean(d), g.boolean(d))),
            11 => self.without_try(|g| {
                let scrutinee = g.place(Ty::Opt, d);
                let v = g.fresh();
                let guard = g.with(&v, Ty::Int, |g| g.boolean(d));
                format!("matches!({scrutinee}, Some({v}) if {guard})")
            }),
            // `matches!` is a `match`: inside an `if` test, a `?` in its
            // scrutinee is `[check/position]`.
            12 => self.without_try(|g| format!("matches!({}, 0..=9 | 100)", g.int(d))),
            _ => self.boolean(d),
        }
    }

    fn opt(&mut self, depth: u32) -> String {
        if depth == 0 || self.rng.below(5) == 0 {
            return match self.rng.below(4) {
                0 => "None".to_string(),
                1 => format!("Some({})", self.lit()),
                _ => self.var(Ty::Opt).unwrap_or_else(|| "None".to_string()),
            };
        }
        let d = depth - 1;
        match self.rng.below(9) {
            0 => format!("Some({})", self.int(d)),
            1 | 2 => {
                let v = self.fresh();
                let opt = self.place(Ty::Opt, d);
                let body = self.closure(|g| g.with(&v, Ty::Int, |g| g.int(d)));
                format!("({opt}).map(|{v}| {body})")
            }
            3 => format!("({}).ok()", self.place(Ty::Res, d)),
            4 => {
                let m = self.rng.pick(&["checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem"]);
                format!("({}).{m}({})", self.int(d), self.int(d))
            }
            5 => self.without_try(|g| format!("(if {} {{ {} }} else {{ {} }})", g.boolean(d), g.opt(d), g.opt(d))),
            6 => self.without_try(|g| {
                let scrutinee = g.place(Ty::Res, d);
                let v = g.fresh();
                let ok = g.with(&v, Ty::Int, |g| g.opt(d));
                format!("(match {scrutinee} {{ Ok({v}) => {ok}, Err(_) => None }})")
            }),
            _ => self.opt(d),
        }
    }

    fn res(&mut self, depth: u32) -> String {
        if depth == 0 || self.rng.below(5) == 0 {
            return match self.rng.below(4) {
                0 => format!("Ok({})", self.lit()),
                1 => format!("Err({})", self.lit()),
                _ => self.var(Ty::Res).unwrap_or_else(|| "Err(0)".to_string()),
            };
        }
        let d = depth - 1;
        match self.rng.below(8) {
            0 => format!("Ok({})", self.int(d)),
            1 => format!("Err({})", self.int(d)),
            2 | 3 => {
                let v = self.fresh();
                let res = self.place(Ty::Res, d);
                let m = self.rng.pick(&["map", "map_err"]);
                let body = self.closure(|g| g.with(&v, Ty::Int, |g| g.int(d)));
                format!("({res}).{m}(|{v}| {body})")
            }
            4 => format!("({}).ok_or({})", self.place(Ty::Opt, d), self.int(d)),
            5 => self.without_try(|g| format!("(if {} {{ {} }} else {{ {} }})", g.boolean(d), g.res(d), g.res(d))),
            _ => self.res(d),
        }
    }
}

pub(crate) const PARAMS: &str = "a: i32, b: i32, o: Option<i32>, r: Result<i32, i32>, c: bool";

type Row = (i32, i32, Option<i32>, Result<i32, i32>, bool);

fn rows(rng: &mut Rng) -> Vec<Row> {
    let int = |rng: &mut Rng| -> i32 {
        if rng.below(2) == 0 {
            rng.pick(&[-3, -2, -1, 0, 1, 2, 3])
        } else {
            rng.edgy(32, true) as i32
        }
    };
    (0..12)
        .map(|_| {
            let a = int(rng);
            let b = int(rng);
            let o = if rng.below(3) == 0 { None } else { Some(int(rng)) };
            let r = if rng.below(3) == 0 { Err(int(rng)) } else { Ok(int(rng)) };
            (a, b, o, r, rng.below(2) == 0)
        })
        .collect()
}

/// What every baseline starts with: `Show` for the generated types, and
/// `run`, which prints a result as `support::Show` would, or
/// `panic(message)`.
pub(crate) const SHOW_PRELUDE: &str = r#"
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

/// The Rust baseline: the functions plus a `main` that prints each result.
fn harness(source: &str, fns: &[(String, Ty)], rows: &[Row]) -> String {
    let mut out = String::from(source);
    out.push_str(SHOW_PRELUDE);
    out.push_str("fn main() {\n    std::panic::set_hook(Box::new(|_| {}));\n");
    for (a, b, o, r, c) in rows {
        for (name, _) in fns {
            writeln!(out, "    println!(\"{{}}\", run(|| {name}({a}, {b}, {o:?}, {r:?}, {c})));").expect("write");
        }
    }
    out.push_str("}\n");
    out
}

/// Compiles `program` with debug-build checks, runs it, and returns the
/// lines it prints.
pub(crate) fn rust_lines(program: &str) -> Vec<String> {
    let dir = support::scratch("generated-rust");
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

/// Seeds checked on every run; any seed may be drawn with
/// `PURECRATE_GEN_SEED`.
const SEEDS: &[u64] = &[1, 2, 3, 4, 5, 6, 7, 8];

fn generate(seed: u64, count: usize, source: &mut String, fns: &mut Vec<(String, Ty)>) {
    let mut g = Gen::new(seed);
    for i in 0..count {
        let ret = g.rng.pick(&[Ty::Int, Ty::Bool, Ty::Opt, Ty::Res]);
        g.ret = ret;
        g.next_name = 0;
        g.scope = Gen::params();
        let body = match ret {
            // A `?` needs the value wrapped back.
            Ty::Opt if g.rng.below(2) == 0 => format!("Some({})", g.int(4)),
            Ty::Res if g.rng.below(2) == 0 => format!("Ok({})", g.int(4)),
            _ => g.expr(ret, 4),
        };
        let name = format!("s{seed}_f{i}");
        writeln!(source, "pub fn {name}({PARAMS}) -> {} {{\n    {body}\n}}\n", ret.rust()).expect("write");
        fns.push((name, ret));
    }
}

/// Writes the generated crate to `PURECRATE_GEN_DUMP`, if set, before
/// anything can fail on it.
pub(crate) fn dump(source: &str) {
    if let Some(path) = std::env::var_os("PURECRATE_GEN_DUMP") {
        fs::write(path, source).expect("write PURECRATE_GEN_DUMP");
    }
}

/// The seeds a test draws: `PURECRATE_GEN_SEED` alone, or `defaults`.
pub(crate) fn seeds(defaults: &[u64]) -> Vec<u64> {
    match std::env::var("PURECRATE_GEN_SEED").ok().and_then(|s| s.parse().ok()) {
        Some(seed) => vec![seed],
        None => defaults.to_vec(),
    }
}

/// How many functions a seed draws: `PURECRATE_GEN_FNS`, or `default`.
pub(crate) fn fn_count(default: usize) -> usize {
    std::env::var("PURECRATE_GEN_FNS").ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}

/// Runs `fns` of `source` in Rust and in the generated TS on the same rows
/// and asserts they agree; on a failure, prints the source.
pub(crate) fn compare(crate_name: &str, seeds: &[u64], source: &str, fns: &[(String, Ty)]) {
    dump(source);
    let rows = rows(&mut Rng::new(seeds[0] ^ 0xa5a5));
    let rust = rust_lines(&harness(source, fns, &rows));
    assert_eq!(rust.len(), rows.len() * fns.len());
    let mut results = rust.into_iter();
    let mut cases = Vec::new();
    for (a, b, o, r, c) in &rows {
        for (name, _) in fns {
            let args = [a.js(), b.js(), o.js(), r.js(), c.js()].join(", ");
            cases.push(Case {
                name: Box::leak(name.clone().into_boxed_str()),
                call: format!("{}({args})", purecrate_ir::to_camel(name)),
                rust: results.next().expect("a result per case"),
            });
        }
    }
    check_cases(crate_name, seeds, source, &cases);
}

/// Asserts the generated TS agrees with `cases`; on a failure, prints the
/// source.
pub(crate) fn check_cases(crate_name: &str, seeds: &[u64], source: &str, cases: &[Case]) {
    let label = format!("{crate_name} (seeds {seeds:?}; PURECRATE_GEN_SEED reruns one)");
    // A sweep (`PURECRATE_GEN_SEED`) may ask for the values alone with
    // `PURECRATE_GEN_TYPES=report`: what tsc refuses is printed, and node
    // still runs. The seeds every run checks always type-check.
    let report = std::env::var_os("PURECRATE_GEN_SEED").is_some()
        && std::env::var("PURECRATE_GEN_TYPES").is_ok_and(|v| v == "report");
    let run = || {
        if report {
            if !support::assert_values_equivalent(crate_name, source, cases) {
                eprintln!("{label}: values agree; tsc refuses the package");
            }
        } else {
            support::assert_equivalent(crate_name, source, cases);
        }
    };
    if let Err(e) = std::panic::catch_unwind(run) {
        eprintln!("{label}\n{source}");
        std::panic::resume_unwind(e);
    }
}

#[test]
fn generated_expressions_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(60);
    let (mut source, mut fns) = (String::new(), Vec::new());
    for &seed in &seeds {
        generate(seed, count, &mut source, &mut fns);
    }
    compare("generated", &seeds, &source, &fns);
}
