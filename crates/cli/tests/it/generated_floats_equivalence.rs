//! Float expressions generated from a seed: `f64` and `f32` arithmetic
//! (`+ - * / %`, unary `-`), `round` / `floor` / `ceil` / `trunc` / `abs`,
//! `is_nan` / `is_finite` / `is_infinite`, `NAN` / `INFINITY` /
//! `NEG_INFINITY`, comparisons in `if`s and `let`s, a float to every
//! integer type by `as`, an integer to a float by `as` (from the types std
//! has no `From` for) or `f64::from` / `f32::from`, `f64::from(f32)`, and
//! `f64` to `f32` by `as`; before the result, sometimes `let mut` float
//! locals updated by `+=` .. `%=` and `=`. A function returns a float, an integer, or a
//! `bool`; a float result is compared by its bits, a NaN made `f64::NAN`
//! first (Rust does not fix a NaN's sign or payload either).
//!
//! Into `usize`, only values below 2^40 are cast: a result above 2^53 − 1
//! panics in TS, the documented gap.
//!
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.

use std::fmt::Write as _;

use crate::support::generated::{compare_rows, fn_count, seeds, Args, SEEDS};
use crate::support::{Js, Rng};

#[derive(Clone, Copy, PartialEq, Debug)]
enum I {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

const INTS: [I; 11] = [I::I8, I::I16, I::I32, I::I64, I::I128, I::U8, I::U16, I::U32, I::U64, I::U128, I::Usize];

impl I {
    fn rust(self) -> &'static str {
        match self {
            I::I8 => "i8",
            I::I16 => "i16",
            I::I32 => "i32",
            I::I64 => "i64",
            I::I128 => "i128",
            I::U8 => "u8",
            I::U16 => "u16",
            I::U32 => "u32",
            I::U64 => "u64",
            I::U128 => "u128",
            I::Usize => "usize",
        }
    }

    fn signed(self) -> bool {
        matches!(self, I::I8 | I::I16 | I::I32 | I::I64 | I::I128)
    }

    /// The largest literal written: 2^127 − 1 for the 128-bit types (a
    /// literal stops below 2^127), 2^32 − 1 for `usize`.
    fn max(self) -> i128 {
        match self {
            I::I8 => i8::MAX.into(),
            I::I16 => i16::MAX.into(),
            I::I32 => i32::MAX.into(),
            I::I64 => i64::MAX.into(),
            I::U8 => u8::MAX.into(),
            I::U16 => u16::MAX.into(),
            I::U32 | I::Usize => u32::MAX.into(),
            I::U64 => u64::MAX.into(),
            I::I128 | I::U128 => i128::MAX,
        }
    }

    /// The parameter of this type, if there is one.
    fn param(self) -> Option<&'static str> {
        match self {
            I::I64 => Some("n"),
            I::U32 => Some("m"),
            I::I32 => Some("k"),
            I::U64 => Some("u"),
            I::U128 => Some("w"),
            _ => None,
        }
    }

    /// The types std has `From<S>` for into this one (`as` is refused
    /// there).
    fn from(self) -> &'static [I] {
        match self {
            I::I8 | I::U8 => &[],
            I::I16 => &[I::I8, I::U8],
            I::I32 => &[I::I8, I::I16, I::U8, I::U16],
            I::I64 => &[I::I8, I::I16, I::I32, I::U8, I::U16, I::U32],
            I::I128 => &[I::I8, I::I16, I::I32, I::I64, I::U8, I::U16, I::U32, I::U64],
            I::U16 => &[I::U8],
            I::U32 => &[I::U8, I::U16],
            I::U64 => &[I::U8, I::U16, I::U32],
            I::U128 => &[I::U8, I::U16, I::U32, I::U64],
            I::Usize => &[I::U8, I::U16],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum T {
    F64,
    F32,
    Int(I),
    Bool,
}

impl T {
    fn rust(self) -> &'static str {
        match self {
            T::F64 => "f64",
            T::F32 => "f32",
            T::Int(i) => i.rust(),
            T::Bool => "bool",
        }
    }
}

const PARAMS: &str = "x: f64, y: f64, z: f32, n: i64, m: u32, k: i32, u: u64, w: u128, b: bool";

const F64_LITS: &[&str] = &[
    "0.0",
    "-0.0",
    "0.5",
    "-0.5",
    "1.5",
    "2.5",
    "-2.5",
    "1.0",
    "3.0",
    "0.1",
    "1e5",
    "1e300",
    "-1e300",
    "1e-310",
    "0.49999999999999994",
    "4503599627370495.5",
    "65535.5",
    "-129.5",
    "2147483648.0",
    "-2147483649.0",
    "4294967296.0",
    "9007199254740993.0",
    "9223372036854775807.0",
    "18446744073709551616.0",
    "3.4028235e38",
    "3.5e38",
    "170141183460469231731687303715884105728.0",
];

const F32_LITS: &[&str] = &[
    "0.0",
    "-0.0",
    "0.5",
    "-0.5",
    "1.5",
    "-2.5",
    "1.0",
    "0.1",
    "16777217.0",
    "3.4028235e38",
    "1e-45",
    "1.17549435e-38",
    "2147483648.0",
    "-2147483904.0",
    "255.5",
    "1e10",
    "9223372036854775807.0",
];

struct G {
    rng: Rng,
    /// Locals in scope, with their types.
    scope: Vec<(String, T)>,
    next: usize,
    /// Whether a float literal may go without its suffix here: not as a
    /// method receiver, an `as` operand, or a comparison operand, where
    /// nothing else would give it a type.
    bare: bool,
}

impl G {
    fn typed<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let bare = std::mem::replace(&mut self.bare, false);
        let r = f(self);
        self.bare = bare;
        r
    }

    fn local(&mut self, t: T) -> Option<String> {
        let locals: Vec<String> = self.scope.iter().filter(|(_, v)| *v == t).map(|(n, _)| n.clone()).collect();
        if locals.is_empty() {
            None
        } else {
            Some(self.rng.pick(&locals))
        }
    }

    fn float_lit(&mut self, f32: bool) -> String {
        let ty = if f32 { "f32" } else { "f64" };
        if self.rng.below(6) == 0 {
            let c = self.rng.pick(&["NAN", "INFINITY", "NEG_INFINITY"]);
            return format!("{ty}::{c}");
        }
        let lit = self.rng.pick(if f32 { F32_LITS } else { F64_LITS });
        let lit = if self.bare && self.rng.below(2) == 0 { lit.to_string() } else { format!("{lit}{ty}") };
        if lit.starts_with('-') {
            format!("({lit})")
        } else {
            lit
        }
    }

    fn float(&mut self, f32: bool, d: u32) -> String {
        let t = if f32 { T::F32 } else { T::F64 };
        if d == 0 || self.rng.below(6) == 0 {
            return match self.rng.below(5) {
                0 | 1 => self.float_lit(f32),
                2 => self.local(t).unwrap_or_else(|| if f32 { "z" } else { "x" }.into()),
                _ if f32 => "z".into(),
                _ => self.rng.pick(&["x", "y"]).into(),
            };
        }
        let d = d - 1;
        match self.rng.below(16) {
            0..=3 => {
                let op = self.rng.pick(&["+", "-", "*", "/", "%"]);
                format!("({} {op} {})", self.float(f32, d), self.float(f32, d))
            }
            4 => format!("(-{})", self.float(f32, d)),
            5 | 6 => {
                let m = self.rng.pick(&["round", "floor", "ceil", "trunc", "abs"]);
                let recv = self.typed(|g| g.float(f32, d));
                format!("({recv}).{m}()")
            }
            7 => {
                let test = self.test(d);
                format!("(if {test} {{ {} }} else {{ {} }})", self.float(f32, d), self.float(f32, d))
            }
            8 => {
                let li = self.rng.pick(&INTS);
                let lt = self.rng.pick(&[T::F64, T::F32, T::F64, T::F32, T::Int(li), T::Bool]);
                self.next += 1;
                let v = format!("v{}", self.next);
                let value = self.value(lt, d);
                self.scope.push((v.clone(), lt));
                let body = self.float(f32, d);
                self.scope.pop();
                format!("({{ let {v}: {} = {value}; {body} }})", lt.rust())
            }
            // The other float.
            9 if f32 => {
                let e = self.typed(|g| g.float(false, d));
                format!("(({e}) as f32)")
            }
            9 => {
                let e = self.typed(|g| g.float(true, d));
                format!("f64::from({e})")
            }
            // From an integer: `From` where std has it, `as` otherwise.
            10 | 11 => {
                let from: &[I] =
                    if f32 { &[I::I8, I::I16, I::U8, I::U16] } else { &[I::I8, I::I16, I::I32, I::U8, I::U16, I::U32] };
                let s = self.rng.pick(&INTS);
                if from.contains(&s) {
                    format!("{}::from({})", if f32 { "f32" } else { "f64" }, self.int(s, d))
                } else {
                    let e = self.int(s, d);
                    format!("(({e}) as {})", if f32 { "f32" } else { "f64" })
                }
            }
            _ => self.float(f32, d),
        }
    }

    fn int_lit(&mut self, i: I) -> String {
        let max = i.max();
        let n = self.rng.pick(&[0i128, 1, 2, 7, 100, max, max - 1]).min(max);
        if i.signed() && self.rng.below(3) == 0 && n != 0 {
            format!("(-{n}{})", i.rust())
        } else {
            format!("{n}{}", i.rust())
        }
    }

    fn int(&mut self, i: I, d: u32) -> String {
        let t = T::Int(i);
        if d == 0 || self.rng.below(6) == 0 {
            return match self.rng.below(4) {
                0 => self.int_lit(i),
                1 => self.local(t).unwrap_or_else(|| self.int_lit(i)),
                _ => i.param().map(str::to_string).or_else(|| self.local(t)).unwrap_or_else(|| self.int_lit(i)),
            };
        }
        let d = d - 1;
        match self.rng.below(12) {
            // A float, toward zero and saturating; into `usize`, below
            // 2^40 (NaN fails the test, and is 0 anyway).
            0..=4 => {
                let f32 = self.rng.below(2) == 0;
                if i == I::Usize {
                    self.next += 1;
                    let v = format!("v{}", self.next);
                    let (ty, lim) = if f32 { ("f32", "1099511627776.0f32") } else { ("f64", "1099511627776.0f64") };
                    let e = self.float(f32, d);
                    format!("({{ let {v}: {ty} = {e}; (if {v} < {lim} {{ {v} }} else {{ 0.0 }}) as usize }})")
                } else {
                    let e = self.typed(|g| g.float(f32, d));
                    format!("(({e}) as {})", i.rust())
                }
            }
            5 => {
                let op = if i == I::Usize {
                    self.rng.pick(&["+", "-", "/", "%"])
                } else {
                    self.rng.pick(&["+", "-", "*", "/", "%"])
                };
                format!("({} {op} {})", self.int(i, d), self.int(i, d))
            }
            6 => {
                // `wrapping_*` on `usize` goes past 2^53, the documented gap.
                let m = if i == I::Usize {
                    self.rng.pick(&["min", "max"])
                } else {
                    self.rng.pick(&["min", "max", "wrapping_add", "wrapping_sub"])
                };
                format!("({}).{m}({})", self.int(i, d), self.int(i, d))
            }
            7 if !i.from().is_empty() => {
                let s = self.rng.pick(i.from());
                format!("{}::from({})", i.rust(), self.int(s, d))
            }
            // Another integer type, wrapping; into `usize` from `u32` only.
            8 => {
                let s = if i == I::Usize {
                    I::U32
                } else {
                    let others: Vec<I> = INTS.into_iter().filter(|o| *o != i && !i.from().contains(o)).collect();
                    self.rng.pick(&others)
                };
                format!("(({}) as {})", self.int(s, d), i.rust())
            }
            9 => {
                let test = self.test(d);
                format!("(if {test} {{ {} }} else {{ {} }})", self.int(i, d), self.int(i, d))
            }
            10 => {
                let li = self.rng.pick(&INTS);
                let lt = self.rng.pick(&[T::F64, T::F32, T::Int(li)]);
                self.next += 1;
                let v = format!("v{}", self.next);
                let value = self.value(lt, d);
                self.scope.push((v.clone(), lt));
                let body = self.int(i, d);
                self.scope.pop();
                format!("({{ let {v}: {} = {value}; {body} }})", lt.rust())
            }
            _ => self.int(i, d),
        }
    }

    /// A `bool`: a float test, a comparison, `b`, or a `bool` local.
    fn test(&mut self, d: u32) -> String {
        let t = match self.rng.below(8) {
            0..=2 => {
                let f32 = self.rng.below(2) == 0;
                let m = self.rng.pick(&["is_nan", "is_finite", "is_infinite"]);
                let recv = self.typed(|g| g.float(f32, d));
                format!("({recv}).{m}()")
            }
            3..=5 => {
                let op = self.rng.pick(&["<", "<=", ">", ">=", "==", "!="]);
                let f32 = self.rng.below(2) == 0;
                // The left side gives the right its type: a literal there may be
                // bare.
                let l = self.typed(|g| g.float(f32, d));
                let bare = std::mem::replace(&mut self.bare, true);
                let r = self.float(f32, d);
                self.bare = bare;
                format!("{l} {op} {r}")
            }
            6 => {
                let op = self.rng.pick(&["<", "<=", ">", ">=", "==", "!="]);
                let i = self.rng.pick(&INTS);
                format!("{} {op} {}", self.int(i, d), self.int(i, d))
            }
            _ => self.local(T::Bool).unwrap_or_else(|| "b".into()),
        };
        match self.rng.below(5) {
            0 => format!("(!({t}) || b)"),
            1 => format!("({t} && b)"),
            _ => t,
        }
    }

    /// A third of the time, statements before the result: `let mut`
    /// float locals updated by compound assignment (`+=` .. `%=`) and
    /// plain assignment, left in scope. An assignment is a statement of its
    /// own: inside a larger expression it is `[check/position]`.
    fn statements(&mut self) -> String {
        let mut out = String::new();
        if self.rng.below(3) != 0 {
            return out;
        }
        for _ in 0..1 + self.rng.below(2) {
            let f32 = self.rng.below(2) == 0;
            let t = if f32 { T::F32 } else { T::F64 };
            self.next += 1;
            let v = format!("a{}", self.next);
            let init = self.float(f32, 3);
            out.push_str(&format!("let mut {v}: {} = {init};\n    ", t.rust()));
            self.scope.push((v.clone(), t));
            for _ in 0..1 + self.rng.below(3) {
                let op = self.rng.pick(&["+=", "-=", "*=", "/=", "%=", "="]);
                let e = self.float(f32, 3);
                out.push_str(&format!("{v} {op} {e};\n    "));
            }
        }
        out
    }

    fn value(&mut self, t: T, d: u32) -> String {
        match t {
            T::F64 => self.float(false, d),
            T::F32 => self.float(true, d),
            T::Int(i) => self.int(i, d),
            T::Bool => self.test(d),
        }
    }
}

/// `Show` for the types beyond `i32` and `bool`; a float as its bits, as
/// `support::Show` and the driver's `bits` print it.
const SHOW_TYPES: &str = "impl Show for i8 { fn show(&self) -> String { self.to_string() } }
impl Show for i16 { fn show(&self) -> String { self.to_string() } }
impl Show for i64 { fn show(&self) -> String { self.to_string() } }
impl Show for i128 { fn show(&self) -> String { self.to_string() } }
impl Show for u8 { fn show(&self) -> String { self.to_string() } }
impl Show for u16 { fn show(&self) -> String { self.to_string() } }
impl Show for u32 { fn show(&self) -> String { self.to_string() } }
impl Show for u64 { fn show(&self) -> String { self.to_string() } }
impl Show for u128 { fn show(&self) -> String { self.to_string() } }
impl Show for usize { fn show(&self) -> String { self.to_string() } }
impl Show for f64 { fn show(&self) -> String { self.to_bits().to_string() } }
impl Show for f32 { fn show(&self) -> String { f64::from(*self).to_bits().to_string() } }
";

/// An `f64` argument: an edge, a value with a fraction of a half or a
/// quarter, or any non-NaN bit pattern.
fn f64_arg(rng: &mut Rng) -> f64 {
    const EDGES: &[f64] = &[
        f64::NAN,
        0.0,
        -0.0,
        0.5,
        -0.5,
        2.5,
        -2.5,
        1.5,
        f64::INFINITY,
        f64::NEG_INFINITY,
        2147483648.0,
        -2147483648.0,
        2147483647.5,
        4294967296.0,
        9007199254740991.0,
        9007199254740992.0,
        9007199254740994.0,
        9223372036854775808.0,
        -9223372036854775808.0,
        18446744073709551616.0,
        170141183460469231731687303715884105728.0,
        340282366920938463463374607431768211456.0,
        5e-324,
        2.2250738585072014e-308,
        3.4028235e38,
        1.401298464324817e-45,
        f64::MAX,
        0.49999999999999994,
        -128.5,
        255.5,
    ];
    match rng.below(3) {
        0 => rng.pick(EDGES),
        1 => (rng.below(4001) as f64 - 2000.0) / 4.0,
        _ => {
            let x = f64::from_bits(rng.next());
            if x.is_nan() {
                1.0
            } else {
                x
            }
        }
    }
}

fn f32_arg(rng: &mut Rng) -> f32 {
    const EDGES: &[f32] = &[
        f32::NAN,
        0.0,
        -0.0,
        0.5,
        -0.5,
        2.5,
        -2.5,
        f32::INFINITY,
        f32::NEG_INFINITY,
        16777216.0,
        16777218.0,
        2147483648.0,
        -2147483648.0,
        4294967296.0,
        f32::MAX,
        f32::MIN_POSITIVE,
        1e-45,
        1.1754942e-38,
        -128.5,
        255.5,
    ];
    match rng.below(3) {
        0 => rng.pick(EDGES),
        1 => (rng.below(4001) as f32 - 2000.0) / 4.0,
        _ => {
            let x = f32::from_bits(rng.next() as u32);
            if x.is_nan() {
                1.0
            } else {
                x
            }
        }
    }
}

fn u128_arg(rng: &mut Rng) -> u128 {
    match rng.below(3) {
        0 => rng.pick(&[0, 1, 2, u128::from(u64::MAX), 1 << 64, (1 << 104) + 1, (1 << 127) - 1, 1 << 127, u128::MAX]),
        1 => u128::from(rng.below(1000)),
        _ => (u128::from(rng.next()) << 64) | u128::from(rng.next()),
    }
}

#[test]
fn generated_floats_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let mut source = String::new();
    let mut fns = Vec::new();
    for &seed in &seeds {
        let mut g = G { rng: Rng::new(seed ^ 0xf10a7), scope: Vec::new(), next: 0, bare: true };
        for i in 0..count {
            let t = match g.rng.below(8) {
                0..=2 => T::F64,
                3 | 4 => T::F32,
                5 => T::Bool,
                _ => T::Int(g.rng.pick(&INTS)),
            };
            g.next = 0;
            g.bare = true;
            g.scope.clear();
            let prefix = g.statements();
            let body = match t {
                // A NaN's sign and payload are not fixed; print one NaN.
                T::F64 | T::F32 => {
                    let e = g.value(t, 4);
                    format!("let r: {0} = {e};\n    if r.is_nan() {{ {0}::NAN }} else {{ r }}", t.rust())
                }
                _ => g.value(t, 4),
            };
            let body = format!("{prefix}{body}");
            let name = format!("fl{seed}_f{i}");
            writeln!(source, "pub fn {name}({PARAMS}) -> {} {{\n    {body}\n}}\n", t.rust()).expect("write");
            fns.push(name);
        }
    }
    let mut rng = Rng::new(seeds[0] ^ 0x5eed);
    let rows: Vec<Args> = (0..14)
        .map(|_| {
            let x = f64_arg(&mut rng);
            let y = f64_arg(&mut rng);
            let z = f32_arg(&mut rng);
            let small = |rng: &mut Rng| rng.below(2) == 0;
            let n = if small(&mut rng) { rng.pick(&[-3, 0, 1, 7]) } else { rng.edgy(64, true) as i64 };
            let m = if small(&mut rng) { rng.pick(&[0, 1, 7]) } else { rng.edgy(32, false) as u32 };
            let k = if small(&mut rng) { rng.pick(&[-3, 0, 1, 7]) } else { rng.edgy(32, true) as i32 };
            let u = if small(&mut rng) { rng.pick(&[0, 1, 7]) } else { rng.edgy(64, false) as u64 };
            let w = u128_arg(&mut rng);
            let b = rng.below(2) == 0;
            Args {
                rust: format!(
                    "f64::from_bits({}), f64::from_bits({}), f32::from_bits({}), {n}, {m}, {k}, {u}, {w}, {b}",
                    x.to_bits(),
                    y.to_bits(),
                    z.to_bits()
                ),
                js: [x.js(), y.js(), z.js(), n.js(), m.js(), k.js(), u.js(), w.js(), b.js()].join(", "),
            }
        })
        .collect();
    compare_rows("generated_floats", &seeds, &source, SHOW_TYPES, &fns, &rows);
}
