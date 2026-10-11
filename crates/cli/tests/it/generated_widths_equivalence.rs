//! Integer expressions generated from a seed across widths: `u8`, `i8`,
//! `u16`, `i16`, `u32`, `i32` (numbers in TS), `i64`, `u64`, `i128`, and
//! `u128` (`bigint`), each with its own operators, `!`, shifts, `checked_*` /
//! `wrapping_*` / `saturating_*`, `min` / `max`, `pow`, `abs` and `-` on the
//! signed, `T::from(x)` from every narrower type std widens, and `x as T`
//! from every width std does not, in `if`s on comparisons and `let`s; and
//! `usize`, kept below 2^53 (the documented gap): no `*`, `pow`, bitwise,
//! or wrapping, and widened only from `u8` / `u16` / `u32`. Overflow,
//! division by zero, and a shift past the width panic as in Rust, and are
//! compared as panics.
//!
//! Each seed also starts with `const`s of every width, folded at check time
//! from literals, earlier consts, `-`, `!`, the operators, and shifts (by a
//! literal or a `u32` const), that rustc accepts: no overflow, a `u128`
//! below 2^127 (the documented limit) at every step. The functions read
//! them as leaves.
//!
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.

use std::fmt::Write as _;

use crate::support::generated::{compare_rows, fn_count, seeds, Args, SEEDS};
use crate::support::Rng;

#[derive(Clone, Copy, PartialEq, Debug)]
enum W {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    I64,
    U64,
    I128,
    U128,
    Usize,
}

const ALL: [W; 11] = [W::U8, W::I8, W::U16, W::I16, W::U32, W::I32, W::I64, W::U64, W::I128, W::U128, W::Usize];

/// Where a generated `usize` stays: a parameter is at most 2^32, and nothing
/// multiplies, so sums of a few stay far below 2^53.
const USIZE_LIMIT: i128 = 1 << 40;

impl W {
    fn rust(self) -> &'static str {
        match self {
            W::U8 => "u8",
            W::I8 => "i8",
            W::U16 => "u16",
            W::I16 => "i16",
            W::U32 => "u32",
            W::I32 => "i32",
            W::I64 => "i64",
            W::U64 => "u64",
            W::I128 => "i128",
            W::U128 => "u128",
            W::Usize => "usize",
        }
    }

    fn bits(self) -> u32 {
        match self {
            W::U8 | W::I8 => 8,
            W::U16 | W::I16 => 16,
            W::U32 | W::I32 => 32,
            W::I64 | W::U64 | W::Usize => 64,
            W::I128 | W::U128 => 128,
        }
    }

    fn signed(self) -> bool {
        matches!(self, W::I8 | W::I16 | W::I32 | W::I64 | W::I128)
    }

    fn bigint(self) -> bool {
        matches!(self, W::I64 | W::U64 | W::I128 | W::U128)
    }

    /// The range a generated value of this width may take: Rust's, except a
    /// `u128` stops below 2^127 (a literal or const above is refused by
    /// design) and a `usize` far below 2^53.
    fn gen_bounds(self) -> (i128, i128) {
        match self {
            W::U128 => (0, i128::MAX),
            W::I128 => (i128::MIN, i128::MAX),
            W::Usize => (0, USIZE_LIMIT),
            w if w.signed() => (-(1i128 << (w.bits() - 1)), (1i128 << (w.bits() - 1)) - 1),
            w => (0, (1i128 << w.bits()) - 1),
        }
    }

    /// The parameter of this width.
    fn param(self) -> &'static str {
        match self {
            W::U8 => "a",
            W::I16 => "b",
            W::U32 => "c",
            W::I64 => "d",
            W::U64 => "e",
            W::I8 => "g",
            W::U16 => "h",
            W::I32 => "i",
            W::I128 => "j",
            W::U128 => "k",
            W::Usize => "m",
        }
    }

    /// The narrower widths std has `From` for into this one.
    fn from(self) -> &'static [W] {
        match self {
            W::U8 | W::I8 => &[],
            W::U16 => &[W::U8],
            W::I16 => &[W::U8, W::I8],
            W::U32 => &[W::U8, W::U16],
            W::I32 => &[W::U8, W::I8, W::U16, W::I16],
            W::I64 => &[W::U8, W::I8, W::U16, W::I16, W::U32, W::I32],
            W::U64 => &[W::U8, W::U16, W::U32],
            W::I128 => &[W::U8, W::I8, W::U16, W::I16, W::U32, W::I32, W::U64, W::I64],
            W::U128 => &[W::U8, W::U16, W::U32, W::U64],
            W::Usize => &[W::U8, W::U16],
        }
    }

    /// The widths `x as T` is drawn from: every one std's `From` does not
    /// take; into `usize`, only the unsigned widths up to 32 bits (anything
    /// else may pass 2^53).
    fn cast_from(self) -> Vec<W> {
        match self {
            W::Usize => vec![W::U8, W::U16, W::U32],
            w => ALL.into_iter().filter(|o| *o != w && !w.from().contains(o)).collect(),
        }
    }
}

/// Whether unary `-` (and `abs`) is drawn on `w`.
fn negates(w: W) -> bool {
    w.signed()
}

const PARAMS: &str =
    "a: u8, b: i16, c: u32, d: i64, e: u64, f: bool, g: i8, h: u16, i: i32, j: i128, k: u128, m: usize";

/// A generated crate-level const.
struct Const {
    name: String,
    w: W,
    value: i128,
}

struct G {
    rng: Rng,
    /// Locals in scope, with their widths.
    scope: Vec<(String, W)>,
    next: usize,
    /// This seed's consts.
    consts: Vec<Const>,
}

/// Literal magnitudes worth drawing, before clamping to a width.
const MAGNITUDES: &[i128] = &[
    0,
    1,
    2,
    3,
    7,
    100,
    (1 << 31) - 1,
    1 << 31,
    (1 << 32) - 1,
    1 << 32,
    (1 << 53) - 1,
    1 << 53,
    (1 << 63) - 1,
    1 << 63,
    (1 << 64) - 1,
    1 << 64,
    1 << 126,
    i128::MAX,
];

impl G {
    fn lit(&mut self, w: W) -> String {
        let (_, max) = w.gen_bounds();
        let n = match self.rng.below(3) {
            0 => self.rng.pick(&[max, max - 1]),
            _ => self.rng.pick(MAGNITUDES),
        }
        .min(max);
        if w.signed() && self.rng.below(4) == 0 && n != 0 {
            format!("(-{n}{})", w.rust())
        } else {
            format!("{n}{}", w.rust())
        }
    }

    fn leaf(&mut self, w: W) -> String {
        let locals: Vec<String> = self.scope.iter().filter(|(_, v)| *v == w).map(|(n, _)| n.clone()).collect();
        let consts: Vec<String> = self.consts.iter().filter(|c| c.w == w).map(|c| c.name.clone()).collect();
        match self.rng.below(5) {
            0 => self.lit(w),
            1 if !locals.is_empty() => self.rng.pick(&locals),
            2 if !consts.is_empty() => self.rng.pick(&consts),
            _ => w.param().to_string(),
        }
    }

    fn int(&mut self, w: W, d: u32) -> String {
        if d == 0 || self.rng.below(5) == 0 {
            return self.leaf(w);
        }
        if w == W::Usize {
            return self.size(d);
        }
        let d = d - 1;
        match self.rng.below(15) {
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
            8 if negates(w) => {
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
            // `as` wraps; a pair std's `From` takes is written that way.
            12 => {
                let from = self.rng.pick(&w.cast_from());
                format!("(({}) as {})", self.int(from, d), w.rust())
            }
            10 => self.choice(w, d),
            11 => self.bound(w, d),
            13 => format!("(!{})", self.int(w, d)),
            _ => self.int(w, d),
        }
    }

    /// A `usize` that stays below 2^53: no `*`, `pow`, bitwise, or
    /// wrapping.
    fn size(&mut self, d: u32) -> String {
        let w = W::Usize;
        let d = d - 1;
        match self.rng.below(9) {
            0 | 1 => {
                let op = self.rng.pick(&["+", "-", "/", "%"]);
                format!("({} {op} {})", self.int(w, d), self.int(w, d))
            }
            2 => {
                let m = self.rng.pick(&["min", "max", "saturating_sub"]);
                format!("({}).{m}({})", self.int(w, d), self.int(w, d))
            }
            3 => {
                let m = self.rng.pick(&["checked_add", "checked_sub", "checked_div", "checked_rem"]);
                format!("({}).{m}({}).unwrap_or({})", self.int(w, d), self.int(w, d), self.int(w, d))
            }
            4 => {
                let from = self.rng.pick(w.from());
                format!("usize::from({})", self.int(from, d))
            }
            5 => {
                let from = self.rng.pick(&w.cast_from());
                format!("(({}) as usize)", self.int(from, d))
            }
            6 => self.choice(w, d),
            7 => self.bound(w, d),
            _ => self.int(w, d),
        }
    }

    fn choice(&mut self, w: W, d: u32) -> String {
        let cw = self.rng.pick(&ALL);
        let op = self.rng.pick(&["<", "<=", ">", ">=", "==", "!="]);
        let test = format!("{} {op} {}", self.int(cw, d), self.int(cw, d));
        let test = if self.rng.below(3) == 0 { format!("{test} && f") } else { test };
        format!("(if {test} {{ {} }} else {{ {} }})", self.int(w, d), self.int(w, d))
    }

    fn bound(&mut self, w: W, d: u32) -> String {
        let lw = self.rng.pick(&ALL);
        self.next += 1;
        let v = format!("v{}", self.next);
        let value = self.int(lw, d);
        self.scope.push((v.clone(), lw));
        let body = self.int(w, d);
        self.scope.pop();
        format!("{{ let {v}: {} = {value}; {body} }}", lw.rust())
    }

    /// A const expression of width `w` and the value rustc folds it to, or
    /// `None` where rustc would reject it (overflow, a shift past the width,
    /// division by zero) or the value leaves `gen_bounds`.
    fn const_expr(&mut self, w: W, d: u32) -> (String, Option<i128>) {
        let (lo, hi) = w.gen_bounds();
        let fit = |v: Option<i128>| v.filter(|v| (lo..=hi).contains(v));
        let earlier: Vec<(String, i128)> =
            self.consts.iter().filter(|c| c.w == w).map(|c| (c.name.clone(), c.value)).collect();
        if d == 0 || self.rng.below(4) == 0 {
            if !earlier.is_empty() && self.rng.below(2) == 0 {
                let (n, v) = self.rng.pick(&earlier);
                return (n, Some(v));
            }
            let n = match self.rng.below(3) {
                0 => self.rng.pick(&[hi, hi - 1, hi / 2]),
                _ => self.rng.pick(MAGNITUDES),
            }
            .min(hi);
            let text = if self.rng.below(3) == 0 { format!("{n:#x}") } else { n.to_string() };
            return (text, Some(n));
        }
        let d = d - 1;
        let bitwise = w != W::Usize;
        match self.rng.below(8) {
            0 if negates(w) => {
                let (e, v) = self.const_expr(w, d);
                (format!("(-{e})"), fit(v.and_then(i128::checked_neg)))
            }
            1 if bitwise && w != W::U128 => {
                let (e, v) = self.const_expr(w, d);
                let v = v.map(|v| if w.signed() { !v } else { ((1i128 << w.bits()) - 1) ^ v });
                (format!("(!{e})"), fit(v))
            }
            2 | 3 if bitwise => {
                let (e, v) = self.const_expr(w, d);
                let shr = self.rng.below(2) == 0;
                let amounts = [0, 1, 2, w.bits() / 2, w.bits() - 1];
                // A `u32` const of a fitting value may stand for the amount.
                let named: Vec<(String, i128)> = self
                    .consts
                    .iter()
                    .filter(|c| c.w == W::U32 && amounts.contains(&(c.value as u32)) && c.value < 128)
                    .map(|c| (c.name.clone(), c.value))
                    .collect();
                let (text, k) = if !named.is_empty() && self.rng.below(3) == 0 {
                    self.rng.pick(&named)
                } else {
                    let k = self.rng.pick(&amounts);
                    (k.to_string(), i128::from(k))
                };
                if k >= i128::from(w.bits()) {
                    return (String::new(), None);
                }
                let v = v.map(|v| if shr { v >> k } else { wrap(w, v << k) });
                (format!("({e} {} {text})", if shr { ">>" } else { "<<" }), fit(v))
            }
            _ => {
                let ops: &[&str] = if bitwise {
                    &["+", "-", "*", "/", "%", "|", "&", "^", "|", "*"]
                } else {
                    &["+", "-", "*", "/", "%"]
                };
                let op = self.rng.pick(ops);
                let (l, a) = self.const_expr(w, d);
                let (r, b) = self.const_expr(w, d);
                let v = match (a, b) {
                    (Some(a), Some(b)) => match op {
                        "+" => a.checked_add(b),
                        "-" => a.checked_sub(b),
                        "*" => a.checked_mul(b),
                        // `MIN / -1` and `MIN % -1` overflow in rustc.
                        "/" | "%" if b == 0 || (a == lo && b == -1 && w.signed()) => None,
                        "/" => a.checked_div(b),
                        "%" => a.checked_rem(b),
                        "|" => Some(a | b),
                        "&" => Some(a & b),
                        _ => Some(a ^ b),
                    },
                    _ => None,
                };
                (format!("({l} {op} {r})"), fit(v))
            }
        }
    }

    /// Draws this seed's consts, each one rustc and the folder both accept.
    fn draw_consts(&mut self, seed: u64, out: &mut String) {
        for n in 0..14 {
            let w = if n < ALL.len() { ALL[n] } else { self.rng.pick(&[W::U128, W::I128, W::U32]) };
            let name = format!("K{seed}_{n}");
            let mut found = None;
            for _ in 0..40 {
                let (text, v) = self.const_expr(w, 3);
                if let Some(v) = v {
                    found = Some((text, v));
                    break;
                }
            }
            let (text, value) = found.unwrap_or_else(|| ("1".into(), 1));
            writeln!(out, "pub const {name}: {} = {text};", w.rust()).expect("write");
            self.consts.push(Const { name, w, value });
        }
        out.push('\n');
    }
}

/// Keeps the low bits of `v` as `w` holds them (an `i128` shift already
/// keeps 128).
fn wrap(w: W, v: i128) -> i128 {
    let bits = w.bits();
    if bits >= 128 {
        return v;
    }
    let masked = v & ((1i128 << bits) - 1);
    if w.signed() && masked >= 1i128 << (bits - 1) {
        masked - (1i128 << bits)
    } else {
        masked
    }
}

/// `Show` for the widths beyond `i32`.
const SHOW_WIDTHS: &str = "impl Show for u8 { fn show(&self) -> String { self.to_string() } }
impl Show for i8 { fn show(&self) -> String { self.to_string() } }
impl Show for u16 { fn show(&self) -> String { self.to_string() } }
impl Show for i16 { fn show(&self) -> String { self.to_string() } }
impl Show for u32 { fn show(&self) -> String { self.to_string() } }
impl Show for i64 { fn show(&self) -> String { self.to_string() } }
impl Show for u64 { fn show(&self) -> String { self.to_string() } }
impl Show for i128 { fn show(&self) -> String { self.to_string() } }
impl Show for u128 { fn show(&self) -> String { self.to_string() } }
impl Show for usize { fn show(&self) -> String { self.to_string() } }
";

/// One argument of width `w`: small, an edge (MIN, MAX, ±1, powers of two
/// around 2^31, 2^32, 2^53, 2^63, 2^64, 2^127), or anywhere in the width.
fn arg(rng: &mut Rng, w: W) -> String {
    if w == W::U128 {
        let v: u128 = match rng.below(3) {
            0 => rng.pick(&[0, 1, 2, 3]),
            1 => {
                let mut edges: Vec<u128> = vec![u128::MAX, u128::MAX - 1, 1 << 127, (1 << 127) + 1, (1 << 127) - 1];
                edges.extend(edge_values().into_iter().filter(|v| *v >= 0).map(|v| v as u128));
                rng.pick(&edges)
            }
            _ => (u128::from(rng.next()) << 64 | u128::from(rng.next())) >> rng.below(128),
        };
        return format!("{v}{}", if w.bigint() { "n" } else { "" });
    }
    let (lo, hi) = match w {
        W::Usize => (0, 1 << 32),
        W::I128 => (i128::MIN, i128::MAX),
        _ => w.gen_bounds(),
    };
    let v: i128 = match rng.below(3) {
        0 if w.signed() => rng.pick(&[-2, -1, 0, 1, 3]),
        0 => rng.pick(&[0, 1, 2, 3]),
        1 => {
            let mut edges = vec![lo, lo + 1, hi - 1, hi];
            edges.extend(edge_values().into_iter().filter(|v| (lo..=hi).contains(v)));
            rng.pick(&edges)
        }
        _ if w == W::I128 => (i128::from(rng.next()) << 64 | i128::from(rng.next())) >> rng.below(128),
        _ => lo + (rng.next() as i128).rem_euclid(hi - lo + 1),
    };
    format!("{v}{}", if w.bigint() { "n" } else { "" })
}

/// The powers of two around which the representations change, ±1, and
/// their negations.
fn edge_values() -> Vec<i128> {
    let mut out = vec![-1, 0, 1];
    for p in [31, 32, 53, 63, 64, 126] {
        for v in [(1i128 << p) - 1, 1i128 << p, (1i128 << p) + 1] {
            out.push(v);
            out.push(-v);
        }
    }
    out.push(i128::MAX);
    out.push(i128::MIN);
    out
}

#[test]
fn generated_widths_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let mut source = String::new();
    let mut fns = Vec::new();
    for &seed in &seeds {
        let mut g = G { rng: Rng::new(seed ^ 0x1d7), scope: Vec::new(), next: 0, consts: Vec::new() };
        g.draw_consts(seed, &mut source);
        for i in 0..count {
            let w = g.rng.pick(&ALL);
            g.next = 0;
            let body = g.int(w, 4);
            let name = format!("w{seed}_f{i}");
            writeln!(source, "pub fn {name}({PARAMS}) -> {} {{\n    {body}\n}}\n", w.rust()).expect("write");
            fns.push(name);
        }
    }
    let mut rng = Rng::new(seeds[0] ^ 0x3a3a);
    let order = [W::U8, W::I16, W::U32, W::I64, W::U64];
    let rest = [W::I8, W::U16, W::I32, W::I128, W::U128, W::Usize];
    let rows: Vec<Args> = (0..16)
        .map(|_| {
            let mut rust = Vec::new();
            let mut js = Vec::new();
            for w in order {
                let a = arg(&mut rng, w);
                rust.push(a.trim_end_matches('n').to_string());
                js.push(a);
            }
            let f = rng.below(2) == 0;
            rust.push(f.to_string());
            js.push(f.to_string());
            for w in rest {
                let a = arg(&mut rng, w);
                rust.push(a.trim_end_matches('n').to_string());
                js.push(a);
            }
            Args { rust: rust.join(", "), js: js.join(", ") }
        })
        .collect();
    compare_rows("generated_widths", &seeds, &source, SHOW_WIDTHS, &fns, &rows);
}
