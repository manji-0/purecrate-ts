//! Function bodies generated from a seed over the collection features of
//! 0.13.1: `let mut` `Vec`s of `i32`, `String`, a struct, and an enum,
//! built by `clone`, a move, `Vec::new()`, `vec![..]`, or `collect`, then
//! edited by `push`, `insert`, `remove`, `v[i] = x`, and `v[i] op= x`
//! (indices past the end and operands that overflow, so the order of the
//! panics is compared), sorted by `sort`, `sort_by` (an `Ordering`, chained
//! with `then` / `then_with`), and `sort_by_key` (stability over
//! duplicates), and read by `find`, `max` / `min`, `max_by_key` /
//! `min_by_key`, `max_by` / `min_by`, `position`, `any`, `all`, `count`,
//! and `sum`, inside loops that push to or remove from the `Vec` they walk
//! a copy of. `==` / `!=` compare `Vec`s, `Option`s, tuples, the struct,
//! and the enum, with `NaN` and `-0.0` in float fields. Inputs hold empty
//! vectors, duplicates, `i32::MIN` / `MAX`, strings ordered differently by
//! code point and by UTF-16 unit, and indices past the end.
//!
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.
//!
//! What it found (2026-10-11, against 0.13.1), each fixed with a fixture:
//! `v[i] op= x` read `v[i]` before an `x` that needs statements, and
//! `v[f()] = x` evaluated the index before `x` (the value comes first in
//! Rust); `Eq.deep` took one object on both sides as equal, so a `NaN`
//! field compared equal to itself; a variant literal bound to a temporary
//! lost its `kind` literal type, which tsc refuses.

use std::fmt::Write as _;

use crate::support::generated::{compare_rows, fn_count, seeds, Args, SEEDS};
use crate::support::{Js, Rng};

/// The crate's types, the same in every seed.
const TYPES: &str = "#[derive(Clone, PartialEq, Debug)]
pub struct P {
    pub a: i32,
    pub b: String,
    pub c: Option<u8>,
    pub f: f64,
}

#[derive(Clone, PartialEq, Debug)]
pub enum E {
    A,
    B(i32),
    C { s: String, v: Vec<i32> },
    D(f64),
}
";

/// `Show` for what the baseline prints beyond `SHOW_PRELUDE`'s, as
/// `purecrate_canon` prints it.
const SHOW_TYPES: &str = r#"
impl Show for usize { fn show(&self) -> String { self.to_string() } }
impl Show for u8 { fn show(&self) -> String { self.to_string() } }
impl Show for f64 { fn show(&self) -> String { self.to_bits().to_string() } }
impl Show for String {
    fn show(&self) -> String {
        let mut out = String::from("\"");
        for c in self.chars() {
            if (' '..='~').contains(&c) && c != '"' && c != '\\' { out.push(c); } else { out.push_str(&format!("\\u{{{:x}}}", u32::from(c))); }
        }
        out.push('"');
        out
    }
}
impl<T: Show> Show for Vec<T> {
    fn show(&self) -> String { format!("[{}]", self.iter().map(Show::show).collect::<Vec<_>>().join(", ")) }
}
impl<A: Show, B: Show> Show for (A, B) {
    fn show(&self) -> String { format!("({}, {})", self.0.show(), self.1.show()) }
}
impl<A: Show, B: Show, C: Show> Show for (A, B, C) {
    fn show(&self) -> String { format!("({}, {}, {})", self.0.show(), self.1.show(), self.2.show()) }
}
impl Show for P {
    fn show(&self) -> String {
        format!("P {{ a: {}, b: {}, c: {}, f: {} }}", self.a.show(), self.b.show(), self.c.show(), self.f.show())
    }
}
impl Show for E {
    fn show(&self) -> String {
        match self {
            E::A => "E::A".into(),
            E::B(x) => format!("E::B({})", x.show()),
            E::C { s, v } => format!("E::C {{ s: {}, v: {} }}", s.show(), v.show()),
            E::D(x) => format!("E::D({})", x.show()),
        }
    }
}
"#;

const PARAMS: &str = "xs: Vec<i32>, ys: Vec<String>, ps: Vec<P>, es: Vec<E>, i: usize, j: usize, k: i32";

#[derive(Clone, Copy, PartialEq, Debug)]
enum T {
    Int,
    Us,
    Bool,
    Str,
    P,
    E,
    VI,
    VS,
    VP,
    VE,
    OI,
    OP,
    OS,
    OU,
    OE,
}

const VECS: [T; 4] = [T::VI, T::VS, T::VP, T::VE];

impl T {
    fn rust(self) -> &'static str {
        match self {
            T::Int => "i32",
            T::Us => "usize",
            T::Bool => "bool",
            T::Str => "String",
            T::P => "P",
            T::E => "E",
            T::VI => "Vec<i32>",
            T::VS => "Vec<String>",
            T::VP => "Vec<P>",
            T::VE => "Vec<E>",
            T::OI => "Option<i32>",
            T::OP => "Option<P>",
            T::OS => "Option<String>",
            T::OU => "Option<usize>",
            T::OE => "Option<E>",
        }
    }

    fn elem(self) -> T {
        match self {
            T::VI => T::Int,
            T::VS => T::Str,
            T::VP => T::P,
            T::VE => T::E,
            _ => unreachable!("not a Vec"),
        }
    }

    fn param(self) -> &'static str {
        match self {
            T::VI => "xs",
            T::VS => "ys",
            T::VP => "ps",
            T::VE => "es",
            _ => unreachable!("not a Vec"),
        }
    }
}

/// A name in scope: its spelling, type, and whether it is a `let mut`.
/// Closures read only their items, `k`, and literals.
#[derive(Clone)]
struct Var {
    name: String,
    ty: T,
    mutable: bool,
}

struct G {
    rng: Rng,
    scope: Vec<Var>,
    next: usize,
    out: String,
    indent: usize,
    ret: T,
}

impl G {
    fn fresh(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}{}", self.next)
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn coin(&mut self, n: u64) -> bool {
        self.rng.below(n) == 0
    }

    /// Names of type `ty` readable here.
    fn vars(&self, ty: T) -> Vec<String> {
        self.scope.iter().filter(|v| v.ty == ty).map(|v| v.name.clone()).collect()
    }

    fn var(&mut self, ty: T) -> Option<String> {
        let vs = self.vars(ty);
        if vs.is_empty() {
            None
        } else {
            Some(self.rng.pick(&vs))
        }
    }

    /// A `let mut` Vec that may be edited here.
    fn mut_vec(&mut self, ty: Option<T>) -> Option<(String, T)> {
        let vs: Vec<(String, T)> = self
            .scope
            .iter()
            .filter(|v| v.mutable && VECS.contains(&v.ty) && ty.is_none_or(|t| t == v.ty))
            .map(|v| (v.name.clone(), v.ty))
            .collect();
        if vs.is_empty() {
            None
        } else {
            Some(self.rng.pick(&vs))
        }
    }

    /// A readable Vec of `ty` (a parameter not moved, or a local).
    fn vec_of(&mut self, ty: T) -> String {
        self.var(ty).unwrap_or_else(|| format!("{}.clone()", ty.param()))
    }

    fn int_lit(&mut self) -> String {
        self.rng
            .pick(&["0i32", "1i32", "2i32", "-1i32", "3i32", "7i32", "100i32", "2147483647i32", "(-2147483648i32)"])
            .to_string()
    }

    fn str_lit(&mut self) -> String {
        let s = self.rng.pick(&["", "a", "b", "aa", "\u{e9}", "\u{ff61}", "\u{1f600}"]);
        format!("String::from({s:?})")
    }

    fn float_lit(&mut self) -> String {
        self.rng.pick(&["0.0", "-0.0", "1.5", "f64::NAN", "f64::INFINITY"]).to_string()
    }

    fn idx(&mut self, d: u32) -> String {
        let vecs: Vec<String> = VECS.iter().flat_map(|t| self.vars(*t)).collect();
        match self.rng.below(9) {
            0 | 1 => "i".into(),
            2 | 3 => "j".into(),
            4 => "0".into(),
            5 => "1".into(),
            6 if !vecs.is_empty() => format!("{}.len()", self.rng.pick(&vecs)),
            7 if !vecs.is_empty() => format!("({}.len() - 1)", self.rng.pick(&vecs)),
            8 if d > 0 => {
                let pred = self.pred_int(1);
                let v = self.vec_of(T::VI);
                format!("{v}.iter().position(|x| {pred}).unwrap_or(j)")
            }
            _ => "(i + j) % 4".into(),
        }
    }

    /// A predicate on `x`, a `&i32` (`depth` 1) or a `&&i32` (2).
    fn pred_int(&mut self, depth: u32) -> String {
        let item = if depth == 1 { "*x" } else { "**x" };
        let p = match self.rng.below(5) {
            0 => format!("{item} > k"),
            1 => format!("{item} % 2 == 0"),
            2 => format!("{item} == {}", self.int_lit()),
            3 => format!("{item} * 2 > k"),
            _ => format!("{item} + k < 0"),
        };
        p
    }

    fn key_int(&mut self, item: &str) -> String {
        match self.rng.below(5) {
            0 => item.to_string(),
            1 => format!("{item} % 3"),
            2 => format!("{item} * 2"),
            3 => format!("{item} - k"),
            _ => format!("-{item}"),
        }
    }

    fn cmp_int(&mut self) -> String {
        match self.rng.below(4) {
            0 => "a.cmp(b)".into(),
            1 => "b.cmp(a)".into(),
            2 => "(a % 2).cmp(&(b % 2)).then(b.cmp(a))".into(),
            _ => "(a % 3).cmp(&(b % 3)).then_with(|| a.cmp(b))".into(),
        }
    }

    fn pred_p(&mut self) -> String {
        match self.rng.below(6) {
            0 => "p.a > k".into(),
            1 => "p.c == Some(3)".into(),
            2 => "p.c.is_none()".into(),
            3 => "p.b.len() > 1".into(),
            4 => "p.f.is_nan()".into(),
            _ => "p.a * 2 > k".into(),
        }
    }

    fn key_p(&mut self) -> String {
        self.rng.pick(&["p.a", "p.b.len()", "p.b.clone()", "p.a * 2", "p.c.unwrap_or(0)"]).into()
    }

    fn cmp_p(&mut self) -> String {
        self.rng
            .pick(&[
                "a.a.cmp(&b.a)",
                "b.a.cmp(&a.a)",
                "a.b.cmp(&b.b)",
                "a.a.cmp(&b.a).then(b.b.cmp(&a.b))",
                "a.b.len().cmp(&b.b.len()).then_with(|| a.a.cmp(&b.a))",
                "a.c.unwrap_or(0).cmp(&b.c.unwrap_or(0))",
            ])
            .into()
    }

    fn pred_s(&mut self) -> String {
        self.rng.pick(&["s.len() > 1", "s.is_empty()", "s.starts_with(\"a\")", "s.as_str() == \"b\""]).into()
    }

    fn key_s(&mut self) -> String {
        self.rng.pick(&["s.len()", "s.clone()"]).into()
    }

    fn cmp_s(&mut self) -> String {
        self.rng.pick(&["a.cmp(b)", "b.cmp(a)", "a.len().cmp(&b.len())", "a.len().cmp(&b.len()).then(b.cmp(a))"]).into()
    }

    fn pred_e(&mut self) -> String {
        self.rng
            .pick(&[
                "matches!(e, E::A)",
                "matches!(e, E::B(_))",
                "matches!(e, E::C { .. })",
                "matches!(e, E::B(n) if *n > k)",
                "matches!(e, E::D(x) if x.is_nan())",
            ])
            .into()
    }

    fn int(&mut self, d: u32) -> String {
        if d == 0 || self.coin(4) {
            let locals = self.vars(T::Int);
            return match self.rng.below(4) {
                0 => self.int_lit(),
                1 if !locals.is_empty() => self.rng.pick(&locals),
                _ => "k".into(),
            };
        }
        let d = d - 1;
        match self.rng.below(14) {
            0 | 1 => {
                let op = self.rng.pick(&["+", "-", "*"]);
                format!("({} {op} {})", self.int(d), self.int(d))
            }
            2 | 3 => {
                let v = self.vec_of(T::VI);
                let at = self.idx(d);
                format!("{v}[{at}]")
            }
            4 => {
                let v = self.vec_of(T::VI);
                format!("({v}.len() as i32)")
            }
            5 => {
                let v = self.vec_of(T::VP);
                let at = self.idx(d);
                format!("{v}[{at}].a")
            }
            6 => {
                let v = self.vec_of(T::VI);
                format!("{v}.iter().sum::<i32>()")
            }
            7 => {
                let v = self.vec_of(T::VI);
                let m = self.rng.pick(&["max", "min"]);
                format!("{v}.iter().copied().{m}().unwrap_or({})", self.int(d))
            }
            8 => {
                let v = self.vec_of(T::VI);
                let p = self.pred_int(2);
                format!("({v}.iter().filter(|x| {p}).count() as i32)")
            }
            9 => {
                let v = self.vec_of(T::VI);
                let key = self.key_int("*x");
                format!("{v}.iter().map(|x| {key}).sum::<i32>()")
            }
            10 => {
                // A literal `None` would leave the receiver's type unknown.
                let o = loop {
                    let o = self.expr(T::OI, d);
                    if !o.starts_with("None") && !o.starts_with("Some(") {
                        break o;
                    }
                };
                format!("{}.unwrap_or({})", o, self.int(d))
            }
            11 => {
                let v = self.vec_of(T::VE);
                let p = self.pred_e();
                format!("({v}.iter().filter(|e| {p}).count() as i32)")
            }
            _ => self.int(d),
        }
    }

    fn boolean(&mut self, d: u32) -> String {
        if d == 0 {
            let v = self.vec_of(T::VI);
            return format!("{v}.is_empty()");
        }
        let d = d - 1;
        match self.rng.below(12) {
            0 | 1 => {
                let op = self.rng.pick(&["<", "==", ">=", "!="]);
                format!("{} {op} {}", self.int(d), self.int(d))
            }
            2 | 3 => {
                let ty = self.rng.pick(&[T::VI, T::VS, T::VP, T::VE, T::OI, T::OP, T::OS, T::OE, T::P, T::E]);
                let op = self.rng.pick(&["==", "!="]);
                format!("{} {op} {}", self.typed(ty, d), self.expr(ty, d))
            }
            4 => {
                let op = self.rng.pick(&["==", "!="]);
                format!("({}, {}) {op} ({}, {})", self.int(d), self.typed(T::OS, d), self.int(d), self.expr(T::OS, d))
            }
            5 => {
                let ty = self.rng.pick(&VECS);
                let v = self.vec_of(ty);
                format!("{v}.is_empty()")
            }
            6 => {
                let m = self.rng.pick(&["any", "all"]);
                match self.rng.pick(&VECS) {
                    T::VI => {
                        let v = self.vec_of(T::VI);
                        let p = self.pred_int(1);
                        format!("{v}.iter().{m}(|x| {p})")
                    }
                    T::VS => {
                        let v = self.vec_of(T::VS);
                        let p = self.pred_s();
                        format!("{v}.iter().{m}(|s| {p})")
                    }
                    T::VP => {
                        let v = self.vec_of(T::VP);
                        let p = self.pred_p();
                        format!("{v}.iter().{m}(|p| {p})")
                    }
                    _ => {
                        let v = self.vec_of(T::VE);
                        let p = self.pred_e();
                        format!("{v}.iter().{m}(|e| {p})")
                    }
                }
            }
            7 => {
                let p = self.expr(T::P, d);
                let q = self.expr(T::P, d);
                format!("{p} == {q}")
            }
            _ => {
                let locals = self.vars(T::Bool);
                if locals.is_empty() {
                    self.boolean(d)
                } else {
                    let b = self.rng.pick(&locals);
                    format!("!{b}")
                }
            }
        }
    }

    /// A value whose type its own spelling fixes, for the left of `==`
    /// (not `None`, `Some(..)`, `Vec::new()`, or `vec![..]`), so the
    /// literals on the right take it.
    fn typed(&mut self, ty: T, d: u32) -> String {
        loop {
            let e = self.expr(ty, d);
            if !["None", "Some(", "Vec::", "vec!["].iter().any(|p| e.starts_with(p)) {
                break e;
            }
        }
    }

    fn elem(&mut self, ty: T, d: u32) -> String {
        self.expr(ty.elem(), d)
    }

    fn expr(&mut self, ty: T, d: u32) -> String {
        let d1 = d.saturating_sub(1);
        match ty {
            T::Int => self.int(d),
            T::Bool => self.boolean(d),
            T::Us => self.idx(d),
            T::Str => match self.rng.below(4) {
                0 | 1 => self.str_lit(),
                2 => {
                    let v = self.vec_of(T::VS);
                    let at = self.idx(d1);
                    format!("{v}[{at}].clone()")
                }
                _ => {
                    let v = self.vec_of(T::VP);
                    let at = self.idx(d1);
                    format!("{v}[{at}].b.clone()")
                }
            },
            T::P => {
                if self.coin(2) {
                    let v = self.vec_of(T::VP);
                    let at = self.idx(d1);
                    format!("{v}[{at}].clone()")
                } else {
                    let c = if self.coin(3) {
                        "None".to_string()
                    } else {
                        format!("Some({})", self.rng.pick(&[0, 3, 255]))
                    };
                    format!(
                        "(P {{ a: {}, b: {}, c: {c}, f: {} }})",
                        self.int(d1),
                        self.expr(T::Str, d1),
                        self.float_lit()
                    )
                }
            }
            T::E => match self.rng.below(6) {
                0 => "E::A".into(),
                1 => format!("E::B({})", self.int(d1)),
                2 => format!("(E::C {{ s: {}, v: {} }})", self.expr(T::Str, d1), self.expr(T::VI, d1)),
                3 => format!("E::D({})", self.float_lit()),
                _ => {
                    let v = self.vec_of(T::VE);
                    let at = self.idx(d1);
                    format!("{v}[{at}].clone()")
                }
            },
            T::VI | T::VS | T::VP | T::VE => self.vec_expr(ty, d),
            T::OI => {
                let v = self.vec_of(T::VI);
                match self.rng.below(10) {
                    0 => format!("Some({})", self.int(d1)),
                    1 => "None".into(),
                    2 => {
                        let p = self.pred_int(2);
                        format!("{v}.iter().find(|x| {p}).copied()")
                    }
                    3 => {
                        let p = self.pred_int(1);
                        format!("{v}.iter().copied().find(|x| {p})")
                    }
                    4 => {
                        let m = self.rng.pick(&["max", "min"]);
                        format!("{v}.iter().copied().{m}()")
                    }
                    5 => {
                        let m = self.rng.pick(&["max", "min"]);
                        format!("{v}.iter().{m}().copied()")
                    }
                    6 => {
                        let m = self.rng.pick(&["max_by_key", "min_by_key"]);
                        let key = self.key_int("*x");
                        format!("{v}.iter().copied().{m}(|x| {key})")
                    }
                    7 => {
                        let m = self.rng.pick(&["max_by_key", "min_by_key"]);
                        let key = self.key_int("**x");
                        format!("{v}.iter().{m}(|x| {key}).copied()")
                    }
                    _ => {
                        let m = self.rng.pick(&["max_by", "min_by"]);
                        let c = self.cmp_int();
                        format!("{v}.iter().copied().{m}(|a, b| {c})")
                    }
                }
            }
            T::OP => {
                let v = self.vec_of(T::VP);
                match self.rng.below(5) {
                    0 => {
                        let p = self.pred_p();
                        format!("{v}.iter().find(|p| {p}).cloned()")
                    }
                    1 | 2 => {
                        let m = self.rng.pick(&["max_by_key", "min_by_key"]);
                        let key = self.key_p();
                        format!("{v}.iter().{m}(|p| {key}).cloned()")
                    }
                    3 => {
                        let m = self.rng.pick(&["max_by", "min_by"]);
                        let c = self.cmp_p();
                        format!("{v}.iter().{m}(|a, b| {c}).cloned()")
                    }
                    _ => {
                        if self.coin(2) {
                            "None".into()
                        } else {
                            format!("Some({})", self.expr(T::P, d1))
                        }
                    }
                }
            }
            T::OS => {
                let v = self.vec_of(T::VS);
                match self.rng.below(6) {
                    0 => {
                        let m = self.rng.pick(&["max", "min"]);
                        format!("{v}.iter().{m}().cloned()")
                    }
                    1 => {
                        let p = self.pred_s();
                        format!("{v}.iter().find(|s| {p}).cloned()")
                    }
                    2 => {
                        let m = self.rng.pick(&["max_by_key", "min_by_key"]);
                        let key = self.key_s();
                        format!("{v}.iter().{m}(|s| {key}).cloned()")
                    }
                    3 => {
                        let m = self.rng.pick(&["max_by", "min_by"]);
                        let c = self.cmp_s();
                        format!("{v}.iter().{m}(|a, b| {c}).cloned()")
                    }
                    4 => {
                        let w = self.vec_of(T::VP);
                        let m = self.rng.pick(&["max_by_key", "min_by_key"]);
                        let key = self.key_p();
                        format!("{w}.iter().{m}(|p| {key}).map(|p| p.b.clone())")
                    }
                    _ => {
                        if self.coin(2) {
                            "None".into()
                        } else {
                            format!("Some({})", self.expr(T::Str, d1))
                        }
                    }
                }
            }
            T::OU => match self.rng.pick(&VECS) {
                T::VI => {
                    let v = self.vec_of(T::VI);
                    let p = self.pred_int(1);
                    format!("{v}.iter().position(|x| {p})")
                }
                T::VS => {
                    let v = self.vec_of(T::VS);
                    let p = self.pred_s();
                    format!("{v}.iter().position(|s| {p})")
                }
                T::VP => {
                    let v = self.vec_of(T::VP);
                    let p = self.pred_p();
                    format!("{v}.iter().position(|p| {p})")
                }
                _ => {
                    let v = self.vec_of(T::VE);
                    let p = self.pred_e();
                    format!("{v}.iter().position(|e| {p})")
                }
            },
            T::OE => {
                let v = self.vec_of(T::VE);
                match self.rng.below(3) {
                    0 | 1 => {
                        let p = self.pred_e();
                        format!("{v}.iter().find(|e| {p}).cloned()")
                    }
                    _ => {
                        if self.coin(2) {
                            "None".into()
                        } else {
                            format!("Some({})", self.expr(T::E, d1))
                        }
                    }
                }
            }
        }
    }

    /// A new Vec value of `ty`.
    fn vec_expr(&mut self, ty: T, d: u32) -> String {
        let d1 = d.saturating_sub(1);
        match self.rng.below(6) {
            0 | 1 => format!("{}.clone()", self.vec_of(ty)),
            2 => {
                let n = self.rng.below(4);
                let es: Vec<String> = (0..n).map(|_| self.elem(ty, d1)).collect();
                if n == 0 {
                    format!("{}::new()", ty.rust().replace("Vec<", "Vec::<"))
                } else {
                    format!("vec![{}]", es.join(", "))
                }
            }
            _ => {
                let v = self.vec_of(ty);
                match ty {
                    T::VI => match self.rng.below(3) {
                        0 => {
                            let key = self.key_int("*x");
                            format!("{v}.iter().map(|x| {key}).collect::<Vec<i32>>()")
                        }
                        1 => {
                            let p = self.pred_int(1);
                            format!("{v}.iter().copied().filter(|x| {p}).collect::<Vec<i32>>()")
                        }
                        _ => {
                            let w = self.vec_of(T::VP);
                            format!("{w}.iter().map(|p| p.a).collect::<Vec<i32>>()")
                        }
                    },
                    T::VS => {
                        if self.coin(2) {
                            let p = self.pred_s();
                            format!("{v}.iter().filter(|s| {p}).cloned().collect::<Vec<String>>()")
                        } else {
                            let w = self.vec_of(T::VP);
                            format!("{w}.iter().map(|p| p.b.clone()).collect::<Vec<String>>()")
                        }
                    }
                    T::VP => {
                        let p = self.pred_p();
                        format!("{v}.iter().filter(|p| {p}).cloned().collect::<Vec<P>>()")
                    }
                    _ => {
                        let p = self.pred_e();
                        format!("{v}.iter().filter(|e| {p}).cloned().collect::<Vec<E>>()")
                    }
                }
            }
        }
    }

    fn block(&mut self, head: &str, depth: u32, count: u64) {
        self.line(&format!("{head} {{"));
        let scope = self.scope.len();
        self.indent += 1;
        let n = 1 + self.rng.below(count);
        for _ in 0..n {
            self.stmt(depth);
        }
        self.indent -= 1;
        self.scope.truncate(scope);
        self.line("}");
    }

    fn new_vec(&mut self) {
        let ty = self.rng.pick(&VECS);
        let p = ty.param();
        let moved = !self.scope.iter().any(|v| v.name == p);
        let value = if !moved && self.indent == 1 && self.coin(5) {
            // A move: the parameter is gone from here on.
            self.scope.retain(|v| v.name != p);
            p.to_string()
        } else {
            self.vec_expr(ty, 2)
        };
        let v = self.fresh("v");
        self.line(&format!("let mut {v}: {} = {value};", ty.rust()));
        self.scope.push(Var { name: v, ty, mutable: true });
    }

    fn sort(&mut self, v: &str, ty: T) {
        let text = match (ty, self.rng.below(3)) {
            (T::VI | T::VS, 0) => format!("{v}.sort();"),
            (T::VI, 1) => format!("{v}.sort_by(|a, b| {});", self.cmp_int()),
            (T::VI, _) => format!("{v}.sort_by_key(|x| {});", self.key_int("*x")),
            (T::VS, 1) => format!("{v}.sort_by(|a, b| {});", self.cmp_s()),
            (T::VS, _) => format!("{v}.sort_by_key(|s| {});", self.key_s()),
            (T::VP, 0 | 1) => format!("{v}.sort_by(|a, b| {});", self.cmp_p()),
            (T::VP, _) => format!("{v}.sort_by_key(|p| {});", self.key_p()),
            _ => {
                // An enum is not ordered: a key read from it.
                format!("{v}.sort_by_key(|e| match e {{ E::A => 0i32, E::B(n) => *n, E::C {{ v, .. }} => v.len() as i32, E::D(_) => -1 }});")
            }
        };
        self.line(&text);
    }

    fn stmt(&mut self, depth: u32) {
        let pick = if depth == 0 { self.rng.below(9) } else { self.rng.below(19) };
        let target = self.mut_vec(None);
        match (pick, target) {
            (0, _) | (_, None) => self.new_vec(),
            (1 | 2, Some((v, ty))) => {
                let x = self.elem(ty, 2);
                self.line(&format!("{v}.push({x});"));
            }
            (3, Some((v, ty))) => {
                let at = self.idx(1);
                let x = self.elem(ty, 2);
                self.line(&format!("{v}.insert({at}, {x});"));
            }
            (4, Some((v, ty))) => {
                let at = self.idx(1);
                let text = match ty {
                    T::VI if self.coin(2) => format!("acc += {v}.remove({at});"),
                    T::VP if self.coin(2) => format!("acc -= {v}.remove({at}).a;"),
                    _ => format!("{v}.remove({at});"),
                };
                self.line(&text);
            }
            (5 | 6, Some((v, ty))) => {
                // The place's index may not read the Vec it indexes
                // (rustc's borrow of the place comes first).
                let at = loop {
                    let at = self.idx(1);
                    if !at.contains(&format!("{v}.")) && !at.contains(&format!("{v}[")) {
                        break at;
                    }
                };
                if ty == T::VI && self.coin(2) {
                    let op = self.rng.pick(&["+=", "-=", "*="]);
                    let x = self.int(2);
                    self.line(&format!("{v}[{at}] {op} {x};"));
                } else {
                    let x = self.elem(ty, 2);
                    self.line(&format!("{v}[{at}] = {x};"));
                }
            }
            (7 | 8, Some((v, ty))) => self.sort(&v, ty),
            (9, Some((v, ty))) => {
                // Walk a copy while editing the original.
                let snap = self.fresh("s");
                self.line(&format!("let {snap}: {} = {v}.clone();", ty.rust()));
                let x = self.fresh("x");
                self.line(&format!("for {x} in &{snap} {{"));
                self.indent += 1;
                let item = match ty {
                    T::VI => format!("*{x}"),
                    _ => format!("{x}.clone()"),
                };
                if self.coin(2) {
                    self.line(&format!("{v}.push({item});"));
                } else {
                    self.line(&format!("{v}.insert(0, {item});"));
                }
                if self.coin(2) {
                    let test = self.boolean(1);
                    self.line(&format!("if {test} {{"));
                    self.line(&format!("    {v}.remove({v}.len() - 1);"));
                    self.line("}");
                }
                self.indent -= 1;
                self.line("}");
            }
            (10, Some((v, ty))) => {
                // Remove in place while walking by index.
                let c = self.fresh("c");
                self.line(&format!("let mut {c}: usize = 0;"));
                self.line(&format!("while {c} < {v}.len() {{"));
                let test = match ty {
                    T::VI => {
                        let p = self.rng.pick(&["> k", "% 2 == 0", "== 0", "* 2 > k"]);
                        format!("{v}[{c}] {p}")
                    }
                    T::VS => format!("{v}[{c}].len() {} 1", self.rng.pick(&["<", ">", "=="])),
                    T::VP => format!("{v}[{c}].a {} k", self.rng.pick(&["<", ">", "=="])),
                    _ => format!("matches!({v}[{c}], E::{})", self.rng.pick(&["A", "B(_)", "D(_)"])),
                };
                self.line(&format!("    if {test} {{"));
                self.line(&format!("        {v}.remove({c});"));
                self.line("    } else {");
                self.line(&format!("        {c} += 1;"));
                self.line("    }");
                self.line("}");
            }
            (11, Some((v, ty))) => {
                // A range fixed before the loop edits the Vec.
                let n = self.fresh("n");
                self.line(&format!("for {n} in 0..{v}.len() {{"));
                let text = match (ty, self.rng.below(3)) {
                    (T::VI, 0) => format!("{v}[{n}] *= 2;"),
                    (T::VI, 1) => format!("{v}.push({v}[{n}]);"),
                    (_, 2) => format!("{v}.insert({n}, {v}[{n}].clone());"),
                    _ => format!("{v}.push({v}[{n}].clone());"),
                };
                self.line(&format!("    {text}"));
                self.line("}");
            }
            (12, _) => {
                let test = self.boolean(2);
                self.block(&format!("if {test}"), depth - 1, 3);
                if self.coin(2) {
                    self.block("else", depth - 1, 3);
                }
            }
            (13, _) => {
                let test = self.boolean(2);
                let value = self.tail(1);
                self.line(&format!("if {test} {{"));
                self.line(&format!("    return {value};"));
                self.line("}");
            }
            (16, Some((v, ty))) => {
                // A new value for the whole Vec: copied when bound.
                let value = self.vec_expr(ty, 2);
                self.line(&format!("{v} = {value};"));
            }
            (17, Some((v, ty))) => {
                // An owned walk over a copy, while editing the original.
                let src =
                    if self.coin(2) { v.clone() } else { self.vec_of(ty).trim_end_matches(".clone()").to_string() };
                let x = self.fresh("x");
                self.line(&format!("for {x} in {src}.clone() {{"));
                self.indent += 1;
                match self.rng.below(3) {
                    0 if ty == T::VI => self.line(&format!("acc += {x};")),
                    0 => self.line(&format!("{v}.insert(0, {x});")),
                    1 => self.line(&format!("{v}.push({x});")),
                    _ => {
                        let at = self.idx(1);
                        self.line(&format!("{v}.insert({at}, {x});"));
                    }
                }
                if self.coin(2) {
                    let test = self.boolean(1);
                    self.line(&format!("if {test} {{"));
                    self.line(&format!("    {v}.remove(0);"));
                    self.line("}");
                }
                self.indent -= 1;
                self.line("}");
            }
            (14, _) => {
                let op = self.rng.pick(&["+=", "-=", "="]);
                let x = self.int(2);
                self.line(&format!("acc {op} {x};"));
            }
            _ => {
                let x = self.boolean(2);
                self.line(&format!("flag = flag || {x};"));
            }
        }
    }

    /// The function's result: a value of its return type, with `clone`
    /// of every local so nothing is moved before it is read again.
    fn tail(&mut self, d: u32) -> String {
        let ret = self.ret;
        match ret {
            T::Int if self.coin(2) => "acc".into(),
            T::Bool if self.coin(2) => "flag".into(),
            ret => self.expr(ret, d),
        }
    }
}

const RETS: [T; 13] = [T::VI, T::VI, T::VS, T::VP, T::VE, T::OI, T::OP, T::OS, T::OU, T::OE, T::Int, T::Bool, T::Us];

fn generate(seed: u64, count: usize, source: &mut String, fns: &mut Vec<String>) {
    let mut g =
        G { rng: Rng::new(seed ^ 0xc011), scope: Vec::new(), next: 0, out: String::new(), indent: 1, ret: T::Int };
    for n in 0..count {
        g.next = 0;
        g.out.clear();
        g.scope = vec![
            Var { name: "xs".into(), ty: T::VI, mutable: false },
            Var { name: "ys".into(), ty: T::VS, mutable: false },
            Var { name: "ps".into(), ty: T::VP, mutable: false },
            Var { name: "es".into(), ty: T::VE, mutable: false },
            Var { name: "k".into(), ty: T::Int, mutable: false },
            Var { name: "acc".into(), ty: T::Int, mutable: true },
            Var { name: "flag".into(), ty: T::Bool, mutable: true },
        ];
        // A tuple of two results, or one.
        let pair = g.coin(4);
        let (r1, r2) = (g.rng.pick(&RETS), g.rng.pick(&RETS));
        g.line("let mut acc: i32 = 0;");
        g.line("let mut flag: bool = false;");
        let stmts = 3 + g.rng.below(6);
        let rt = if pair { format!("({}, {})", r1.rust(), r2.rust()) } else { r1.rust().to_string() };
        g.ret = r1;
        for _ in 0..stmts {
            let before = g.out.len();
            g.stmt(2);
            // A pair's early return would need both halves: dropped.
            if pair && g.out[before..].contains("return ") {
                g.out.truncate(before);
            }
        }
        let tail = if pair {
            g.ret = r1;
            let a = g.tail(2);
            g.ret = r2;
            let b = g.tail(2);
            format!("({a}, {b})")
        } else {
            g.ret = r1;
            g.tail(2)
        };
        g.line(&tail);
        let name = format!("c{seed}_f{n}");
        writeln!(source, "pub fn {name}({PARAMS}) -> {rt} {{\n{}}}\n", g.out).expect("write");
        fns.push(name);
    }
}

/// A `P` as Rust and as TS.
fn p_value(a: i32, b: &str, c: Option<u8>, f: f64) -> (String, String) {
    let rf = if f.is_nan() {
        "f64::NAN".to_string()
    } else if f.is_infinite() {
        "f64::INFINITY".to_string()
    } else {
        format!("{f:?}")
    };
    (
        format!("P {{ a: {a}, b: String::from({b:?}), c: {c:?}, f: {rf} }}"),
        format!("{{ a: {}, b: {}, c: {}, f: {} }}", a.js(), b.js(), c.js(), f.js()),
    )
}

fn e_value(rng: &mut Rng) -> (String, String) {
    match rng.below(4) {
        0 => ("E::A".into(), "{ kind: \"A\" }".into()),
        1 => {
            let x = rng.pick(&[0, 1, -1, 5, i32::MAX, i32::MIN]);
            (format!("E::B({x})"), format!("{{ kind: \"B\", value: {} }}", x.js()))
        }
        2 => {
            let s = rng.pick(&["", "a", "\u{1f600}"]);
            let v: Vec<i32> = rng.pick(&[vec![], vec![1], vec![2, 2]]);
            (
                format!("E::C {{ s: String::from({s:?}), v: vec!{v:?} }}"),
                format!("{{ kind: \"C\", s: {}, v: {} }}", s.js(), v.js()),
            )
        }
        _ => {
            let f = rng.pick(&[0.0, -0.0, f64::NAN, 2.5]);
            let rf = if f.is_nan() { "f64::NAN".to_string() } else { format!("{f:?}") };
            (format!("E::D({rf})"), format!("{{ kind: \"D\", value: {} }}", f.js()))
        }
    }
}

fn row(rng: &mut Rng) -> Args {
    let nan = f64::NAN;
    let xs: Vec<i32> = rng.pick(&[
        vec![],
        vec![1],
        vec![3, 1, 2],
        vec![5, 5, -1, 5],
        vec![i32::MAX, 1, i32::MIN],
        vec![2, 2, 1, 1, 0],
        vec![-3, 4, -3, 4, 9, 0],
        vec![i32::MIN, i32::MIN],
        // Past the length std's sort switches algorithm at, with ties.
        vec![4, 1, 9, 1, 0, 4, -2, 7, 3, 3, 8, -5, 6, 2, 1, 0, 9, 4, -1, 5, 2, 2, 7, 6, 0],
    ]);
    let ys: Vec<&str> = rng.pick(&[
        vec![],
        vec!["b", "a", "b"],
        vec!["", "aa", "a"],
        vec!["\u{ff61}", "\u{1f600}", "a"],
        vec!["x"],
        vec!["\u{1f600}", "\u{e9}", "\u{ff61}", "\u{1f600}"],
    ]);
    let n = rng.below(5);
    let ps: Vec<(String, String)> = (0..n)
        .map(|_| {
            let a = rng.pick(&[0, 1, 1, -2, i32::MAX, i32::MIN]);
            let b = rng.pick(&["", "a", "b", "aa", "\u{1f600}", "\u{ff61}"]);
            let c = rng.pick(&[None, Some(0), Some(3), Some(255)]);
            let f = rng.pick(&[0.0, -0.0, nan, 1.5]);
            p_value(a, b, c, f)
        })
        .collect();
    let n = rng.below(5);
    let es: Vec<(String, String)> = (0..n).map(|_| e_value(rng)).collect();
    let i = rng.pick(&[0usize, 0, 1, 1, 2, 3, 7]);
    let j = rng.pick(&[0usize, 1, 2, 4]);
    let k = rng.pick(&[0, 1, -1, 2, i32::MAX, i32::MIN]);
    let ys_rust: Vec<String> = ys.iter().map(|s| format!("String::from({s:?})")).collect();
    let ps_rust: Vec<String> = ps.iter().map(|p| p.0.clone()).collect();
    let ps_js: Vec<String> = ps.iter().map(|p| p.1.clone()).collect();
    let es_rust: Vec<String> = es.iter().map(|e| e.0.clone()).collect();
    let es_js: Vec<String> = es.iter().map(|e| e.1.clone()).collect();
    Args {
        rust: format!(
            "vec!{xs:?}, vec![{}], vec![{}], vec![{}], {i}, {j}, {k}",
            ys_rust.join(", "),
            ps_rust.join(", "),
            es_rust.join(", ")
        ),
        js: [
            xs.js(),
            ys.js(),
            format!("[{}]", ps_js.join(", ")),
            format!("[{}]", es_js.join(", ")),
            i.js(),
            j.js(),
            k.js(),
        ]
        .join(", "),
    }
}

#[test]
fn generated_collections_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(30);
    let mut source = String::from(TYPES);
    source.push('\n');
    let mut fns = Vec::new();
    for &seed in &seeds {
        generate(seed, count, &mut source, &mut fns);
    }
    let mut rng = Rng::new(seeds[0] ^ 0xc0c0);
    let rows: Vec<Args> = (0..12).map(|_| row(&mut rng)).collect();
    compare_rows("generated_collections", &seeds, &source, SHOW_TYPES, &fns, &rows);
}
