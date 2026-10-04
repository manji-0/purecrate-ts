//! Integer expressions generated from a seed across widths: `u8`, `i16`,
//! `u32` (numbers in TS), `i64`, and `u64` (`bigint`), each with its own
//! operators, shifts, `checked_*` / `wrapping_*` / `saturating_*`, `min` /
//! `max`, `pow`, `abs` and `-` on the signed, and `T::from(x)` from every
//! narrower type std widens, in `if`s on comparisons and `let`s. Overflow,
//! division by zero, and a shift past the width panic as in Rust, and are
//! compared as panics.
//!
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.

use std::fmt::Write as _;

use crate::generated_equivalence::{check_cases, dump, fn_count, rust_lines, seeds, SHOW_PRELUDE};
use crate::support::{Case, Js, Rng};

#[derive(Clone, Copy, PartialEq, Debug)]
enum W {
    U8,
    I16,
    U32,
    I64,
    U64,
}

const ALL: [W; 5] = [W::U8, W::I16, W::U32, W::I64, W::U64];

impl W {
    fn rust(self) -> &'static str {
        match self {
            W::U8 => "u8",
            W::I16 => "i16",
            W::U32 => "u32",
            W::I64 => "i64",
            W::U64 => "u64",
        }
    }

    fn bits(self) -> u32 {
        match self {
            W::U8 => 8,
            W::I16 => 16,
            W::U32 => 32,
            W::I64 | W::U64 => 64,
        }
    }

    fn signed(self) -> bool {
        matches!(self, W::I16 | W::I64)
    }

    /// The parameter of this width.
    fn param(self) -> &'static str {
        match self {
            W::U8 => "a",
            W::I16 => "b",
            W::U32 => "c",
            W::I64 => "d",
            W::U64 => "e",
        }
    }

    /// The narrower widths std has `From` for into this one.
    fn from(self) -> &'static [W] {
        match self {
            W::U8 => &[],
            W::I16 => &[W::U8],
            W::U32 => &[W::U8],
            W::I64 => &[W::U8, W::I16, W::U32],
            W::U64 => &[W::U8, W::U32],
        }
    }
}

const PARAMS: &str = "a: u8, b: i16, c: u32, d: i64, e: u64, f: bool";

struct G {
    rng: Rng,
    /// Locals in scope, with their widths.
    scope: Vec<(String, W)>,
    next: usize,
}

impl G {
    fn lit(&mut self, w: W) -> String {
        let max = if w.signed() { (1i128 << (w.bits() - 1)) - 1 } else { (1i128 << w.bits()) - 1 };
        let n = self.rng.pick(&[0i128, 1, 2, 3, 7, 100, max, max - 1]).min(max);
        if w.signed() && self.rng.below(4) == 0 && n != 0 {
            format!("(-{n}{})", w.rust())
        } else {
            format!("{n}{}", w.rust())
        }
    }

    fn leaf(&mut self, w: W) -> String {
        let locals: Vec<String> = self.scope.iter().filter(|(_, v)| *v == w).map(|(n, _)| n.clone()).collect();
        match self.rng.below(4) {
            0 => self.lit(w),
            1 if !locals.is_empty() => self.rng.pick(&locals),
            _ => w.param().to_string(),
        }
    }

    fn int(&mut self, w: W, d: u32) -> String {
        if d == 0 || self.rng.below(5) == 0 {
            return self.leaf(w);
        }
        let d = d - 1;
        match self.rng.below(14) {
            0..=3 => {
                let op = self.rng.pick(&["+", "-", "*", "/", "%", "&", "|", "^"]);
                format!("({} {op} {})", self.int(w, d), self.int(w, d))
            }
            4 => {
                let k = self.rng.pick(&[0, 1, 3, w.bits() - 1, w.bits()]);
                let op = self.rng.pick(&["<<", ">>"]);
                format!("({} {op} {k})", self.int(w, d))
            }
            5 => {
                let m = self.rng.pick(&[
                    "min",
                    "max",
                    "wrapping_add",
                    "wrapping_sub",
                    "wrapping_mul",
                    "saturating_add",
                    "saturating_sub",
                    "saturating_mul",
                ]);
                format!("({}).{m}({})", self.int(w, d), self.int(w, d))
            }
            6 => {
                let m = self.rng.pick(&["checked_add", "checked_sub", "checked_mul", "checked_div", "checked_rem"]);
                format!("({}).{m}({}).unwrap_or({})", self.int(w, d), self.int(w, d), self.int(w, d))
            }
            7 => format!("({}).pow({})", self.int(w, d), self.rng.pick(&[0, 1, 2, 3, 5])),
            8 if w.signed() => {
                if self.rng.below(2) == 0 {
                    format!("({}).abs()", self.int(w, d))
                } else {
                    format!("(-{})", self.int(w, d))
                }
            }
            9 if !w.from().is_empty() => {
                let from = self.rng.pick(w.from());
                format!("{}::from({})", w.rust(), self.int(from, d))
            }
            10 => {
                let cw = self.rng.pick(&ALL);
                let op = self.rng.pick(&["<", "<=", ">", ">=", "==", "!="]);
                let test = format!("{} {op} {}", self.int(cw, d), self.int(cw, d));
                let test = if self.rng.below(3) == 0 { format!("{test} && f") } else { test };
                format!("(if {test} {{ {} }} else {{ {} }})", self.int(w, d), self.int(w, d))
            }
            11 => {
                let lw = self.rng.pick(&ALL);
                self.next += 1;
                let v = format!("v{}", self.next);
                let value = self.int(lw, d);
                self.scope.push((v.clone(), lw));
                let body = self.int(w, d);
                self.scope.pop();
                format!("{{ let {v}: {} = {value}; {body} }}", lw.rust())
            }
            _ => self.int(w, d),
        }
    }
}

/// Seeds checked on every run.
const SEEDS: &[u64] = &[1, 2, 3, 4, 5, 6, 7, 8];

#[test]
fn generated_widths_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let mut source = String::new();
    let mut fns = Vec::new();
    for &seed in &seeds {
        let mut g = G { rng: Rng::new(seed ^ 0x1d7), scope: Vec::new(), next: 0 };
        for i in 0..count {
            let w = g.rng.pick(&ALL);
            g.next = 0;
            let body = g.int(w, 4);
            let name = format!("w{seed}_f{i}");
            writeln!(source, "pub fn {name}({PARAMS}) -> {} {{\n    {body}\n}}\n", w.rust()).expect("write");
            fns.push(name);
        }
    }
    dump(&source);

    let mut rng = Rng::new(seeds[0] ^ 0x3a3a);
    let rows: Vec<(u8, i16, u32, i64, u64, bool)> = (0..12)
        .map(|_| {
            let small = |rng: &mut Rng| rng.below(2) == 0;
            let a = if small(&mut rng) { rng.pick(&[0, 1, 2, 3]) } else { rng.edgy(8, false) as u8 };
            let b = if small(&mut rng) { rng.pick(&[-2, 0, 1, 3]) } else { rng.edgy(16, true) as i16 };
            let c = if small(&mut rng) { rng.pick(&[0, 1, 2, 3]) } else { rng.edgy(32, false) as u32 };
            let d = if small(&mut rng) { rng.pick(&[-2, 0, 1, 3]) } else { rng.edgy(64, true) as i64 };
            let e = if small(&mut rng) { rng.pick(&[0, 1, 2, 3]) } else { rng.edgy(64, false) as u64 };
            (a, b, c, d, e, rng.below(2) == 0)
        })
        .collect();

    let mut program = source.clone();
    program.push_str(SHOW_PRELUDE);
    program.push_str(
        "impl Show for u8 { fn show(&self) -> String { self.to_string() } }
impl Show for i16 { fn show(&self) -> String { self.to_string() } }
impl Show for u32 { fn show(&self) -> String { self.to_string() } }
impl Show for i64 { fn show(&self) -> String { self.to_string() } }
impl Show for u64 { fn show(&self) -> String { self.to_string() } }
",
    );
    program.push_str("fn main() {\n    std::panic::set_hook(Box::new(|_| {}));\n");
    for (a, b, c, d, e, f) in &rows {
        for name in &fns {
            writeln!(program, "    println!(\"{{}}\", run(|| {name}({a}, {b}, {c}, {d}, {e}, {f})));").expect("write");
        }
    }
    program.push_str("}\n");
    let mut results = rust_lines(&program).into_iter();

    let mut cases = Vec::new();
    for (a, b, c, d, e, f) in &rows {
        for name in &fns {
            let args = [a.js(), b.js(), c.js(), d.js(), e.js(), f.js()].join(", ");
            cases.push(Case {
                name: Box::leak(name.clone().into_boxed_str()),
                call: format!("{}({args})", purecrate_ir::to_camel(name)),
                rust: results.next().expect("a result per case"),
            });
        }
    }
    check_cases("generated_widths", &seeds, &source, &cases);
}
