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
//! a copy of. From 0.21.0, `dedup` and `dedup_by_key` edit a `Vec` too, and
//! `filter_map`, `flat_map`, `flatten`, `windows`, `chunks` (sizes of 0
//! among them), and ranges stepped by `step_by` either side of `rev` feed
//! the consumers, added into `acc`; these draw from a second generator, so
//! each seed's other statements are the ones it drew before. From 0.23.0,
//! maps and sets keyed by integers, `char`s, and a newtype of `u64` are
//! built from literals and read in key order (a third generator, likewise).
//! `==` / `!=` compare `Vec`s, `Option`s, tuples, the struct,
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
const TYPES: &str = "use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Debug)]
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Id(pub u64);
";

/// `Show` for the crate's types, as `purecrate_canon` prints them.
const SHOW_TYPES: &str = r#"
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
    /// What draws the 0.21.0 shapes, apart from `rng`.
    extra: Rng,
    /// What draws the 0.23.0 maps and sets with keys other than `String`,
    /// apart from both.
    keyed: Rng,
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

    /// A statement, and now and then one of 0.21.0's after it: `dedup` on
    /// a `let mut` `Vec`, or an adaptor's `i32` added into `acc`. Each draws
    /// from `extra`, as `rng` in turn.
    fn stmt(&mut self, depth: u32) {
        self.drawn(depth);
        if self.keyed.below(5) == 0 {
            std::mem::swap(&mut self.rng, &mut self.keyed);
            self.keyed_stmt();
            std::mem::swap(&mut self.rng, &mut self.keyed);
        }
        let which = self.extra.below(8);
        if which > 1 {
            return;
        }
        std::mem::swap(&mut self.rng, &mut self.extra);
        match (which, self.mut_vec(None)) {
            (0, Some((v, ty))) => self.dedup(&v, ty),
            _ => {
                let x = self.adapted();
                self.line(&format!("acc = acc.wrapping_add({x});"));
            }
        }
        std::mem::swap(&mut self.rng, &mut self.extra);
    }

    /// `dedup`, or `dedup_by_key` with a key that may overflow.
    fn dedup(&mut self, v: &str, ty: T) {
        let text = match (ty, self.rng.below(2)) {
            (_, 0) => format!("{v}.dedup();"),
            (T::VI, _) => format!("{v}.dedup_by_key(|x| {});", self.key_int("*x")),
            (T::VS, _) => format!("{v}.dedup_by_key(|s| {});", self.key_s()),
            (T::VP, _) => format!("{v}.dedup_by_key(|p| {});", self.key_p()),
            _ => format!("{v}.dedup_by_key(|e| matches!(e, E::B(_)));"),
        };
        self.line(&text);
    }

    /// An `i32` from the iterator adaptors and ranges: `filter_map`,
    /// `flat_map`, `flatten`, `windows`, `chunks`, and `step_by` with `rev`
    /// on either side, before a consumer; from 0.23.0 `take`, `skip`,
    /// `take_while`, and `skip_while` after a `map` that may panic (on an
    /// item they pull or not), on ranges either side of `rev` and
    /// `step_by`, and `partition`. A size or a step of `i % 3` or `j % 3`
    /// may be 0, which panics. A closure reads no `let mut` local: the
    /// `Vec` is copied into a `let` first.
    fn adapted(&mut self) -> String {
        let v = self.vec_of(T::VI);
        let n = self.rng.pick(&["i % 3", "j % 3", "1 + j % 3", "2"]);
        match self.rng.below(15) {
            // `k / x` panics on 0 and on `i32::MIN / -1`: only on an item
            // `take` pulls.
            9 => format!("{v}.iter().map(|x| k / x).take({n}).fold(0i32, |a, x| a.wrapping_add(x))"),
            // `x + k` may overflow on an item `skip` drops, which it pulls.
            10 => format!("({v}.iter().map(|x| x + k).skip({n}).count() as i32)"),
            // The first item `take_while` refuses is pulled, `map` and all;
            // `skip_while` calls its predicate on none after it.
            11 => format!("{v}.iter().map(|x| 100 / x).take_while(|q| *q > k).fold(1i32, |a, x| a.wrapping_mul(x))"),
            12 => format!("{v}.iter().copied().skip_while(|x| 50 / x > k).fold(0i32, |a, x| a.wrapping_mul(3).wrapping_add(x))"),
            13 => format!(
                "{{ let (a, b): (Vec<i32>, Vec<i32>) = {v}.iter().map(|x| x.wrapping_sub(k)).take(1 + {n}).partition(|x| *x > k); (a.len() as i32).wrapping_mul(7).wrapping_add(b.iter().fold(0i32, |s, x| s.wrapping_add(*x))) }}"
            ),
            14 => {
                let lo = self.int(1);
                match self.rng.below(3) {
                    0 => format!(
                        "{{ let w: Vec<i32> = {v}.clone(); (0..w.len()).skip({n}).take(2).map(|n| w.get(n).copied().unwrap_or(k)).sum::<i32>() }}"
                    ),
                    1 => format!("(({lo}) % 4..=9).rev().skip({n}).step_by(2).fold(k, |a, x| a.wrapping_sub(x))"),
                    _ => format!("(({lo}) % 5..7).take({n}).rev().fold(k, |a, x| a.wrapping_mul(2).wrapping_sub(x))"),
                }
            }
            0 => format!("{v}.iter().filter_map(|x| x.checked_mul(k)).sum::<i32>()"),
            1 => {
                let key = self.key_int("*x");
                format!("{v}.iter().flat_map(|x| vec![*x, {key}]).fold(0i32, |a, x| a.wrapping_mul(3).wrapping_add(x))")
            }
            2 => format!("({v}.iter().flat_map(|x| x.checked_add(k)).count() as i32)"),
            3 => {
                let w = self.vec_of(T::VI);
                format!("vec![{v}.clone(), {w}.clone()].into_iter().flatten().sum::<i32>()")
            }
            4 => {
                let p = self.vec_of(T::VP);
                format!("{p}.iter().map(|p| p.c).flatten().map(|c| i32::from(c)).sum::<i32>()")
            }
            5 => format!("({v}.windows({n}).filter(|w| w[0] < w[w.len() - 1]).count() as i32)"),
            6 => format!("{v}.chunks({n}).map(|c| c[c.len() - 1]).sum::<i32>()"),
            7 => {
                let range = match self.rng.below(4) {
                    0 => format!("(0..w.len()).step_by({n})"),
                    1 => format!("(0..=w.len()).rev().step_by({n})"),
                    2 => format!("(0..w.len()).step_by({n}).rev()"),
                    _ => format!("(i..j + 3).rev().step_by({n}).rev()"),
                };
                format!(
                    "{{ let w: Vec<i32> = {v}.clone(); {range}.map(|n| w.get(n).copied().unwrap_or(k)).sum::<i32>() }}"
                )
            }
            _ => {
                let lo = self.int(1);
                format!("(({lo}) % 4..=3).step_by({n}).fold(k, |a, x| a.wrapping_sub(x))")
            }
        }
    }

    fn drawn(&mut self, depth: u32) {
        let pick = if depth == 0 { self.rng.below(9) } else { self.rng.below(21) };
        if pick >= 19 {
            return self.tree_stmt();
        }
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

    /// A `BTreeMap` or `BTreeSet` built from literal strings (ordered
    /// differently by code point and by UTF-16 unit) and read in key order
    /// into `acc`, so an order that differs from Rust's changes the result;
    /// `insert`'s old value and `acc +=` may overflow.
    fn tree_stmt(&mut self) {
        let m = self.fresh("m");
        let (a, b) = (self.str_lit(), self.str_lit());
        let x = self.fresh("x");
        // `xs` and `ys` may have been moved by now: literals, in any order.
        let n = 1 + self.rng.below(5);
        let words = (0..n).map(|_| self.str_lit()).collect::<Vec<_>>().join(", ");
        let ints = (0..n).map(|_| self.int_lit()).collect::<Vec<_>>().join(", ");
        let ys = format!("vec![{words}]");
        let xs = format!("vec![{ints}]");
        match self.rng.below(3) {
            0 => {
                self.line(&format!("let mut {m}: BTreeMap<String, i32> = BTreeMap::new();"));
                self.line(&format!("for ({x}, n) in {ys}.iter().zip({xs}.iter()) {{"));
                self.line(&format!("    if let Some(o) = {m}.insert({x}.clone(), *n) {{"));
                self.line("        acc += o;");
                self.line("    }");
                self.line("}");
                self.line(&format!("if let Some(o) = {m}.remove(&{a}) {{"));
                self.line("    acc -= o;");
                self.line("}");
                self.line(&format!("for ({x}, n) in &{m} {{"));
                self.line(&format!("    acc = acc.wrapping_mul(31).wrapping_add(*n).wrapping_add({x}.len() as i32);"));
                self.line("}");
                self.line(&format!("flag = flag != {m}.contains_key(&{b});"));
            }
            1 => {
                self.line(&format!("let mut {m}: BTreeSet<String> = {ys}.into_iter().collect();"));
                self.line(&format!("flag = {m}.insert({a}) != flag;"));
                self.line(&format!("if {m}.remove(&{b}) {{"));
                self.line("    acc += 1;");
                self.line("}");
                self.line(&format!("for {x} in &{m} {{"));
                self.line(&format!("    acc = acc.wrapping_mul(7).wrapping_add({x}.chars().count() as i32);"));
                self.line("}");
            }
            _ => {
                self.line(&format!(
                    "let {m}: BTreeMap<String, usize> = {ys}.iter().map(|y| (y.clone(), y.len())).collect();"
                ));
                self.line(&format!("acc += {m}.get(&{a}).copied().unwrap_or(0) as i32;"));
                self.line(&format!(
                    "acc = {m}.keys().fold(acc, |t, key| t.wrapping_mul(3).wrapping_add(key.len() as i32));"
                ));
                self.line(&format!("acc += {m}.values().map(|n| *n as i32).sum::<i32>();"));
            }
        }
    }

    /// A `BTreeMap` or `BTreeSet` keyed by an integer (`9` and `10`,
    /// negatives, `bigint`s past 2^53, each type's bounds), a `char`
    /// outside the BMP, or a newtype of `u64` (design/01 §7.26), built from
    /// literals in any order and read in key order into `acc`, where JS's
    /// `sort()` would give another order and so another result.
    fn keyed_stmt(&mut self) {
        let m = self.fresh("m");
        let x = self.fresh("x");
        let n = 1 + self.rng.below(6);
        match self.rng.below(4) {
            0 => {
                let (ty, lits): (&str, &[&str]) = match self.rng.below(3) {
                    0 => (
                        "i64",
                        &[
                            "9i64",
                            "10i64",
                            "-1i64",
                            "-2i64",
                            "0i64",
                            "9007199254740993i64",
                            "(-9007199254740993i64)",
                            "9223372036854775807i64",
                            "(-9223372036854775808i64)",
                        ],
                    ),
                    1 => (
                        "u128",
                        &[
                            "0u128",
                            "9u128",
                            "10u128",
                            "9007199254740993u128",
                            "18446744073709551616u128",
                            "170141183460469231731687303715884105727u128",
                        ],
                    ),
                    _ => ("i8", &["9i8", "10i8", "-1i8", "-2i8", "0i8", "127i8", "(-128i8)"]),
                };
                let keys = (0..n).map(|_| self.rng.pick(lits)).collect::<Vec<_>>().join(", ");
                let ints = (0..n).map(|_| self.int_lit()).collect::<Vec<_>>().join(", ");
                let (a, b) = (self.rng.pick(lits), self.rng.pick(lits));
                self.line(&format!("let mut {m}: BTreeMap<{ty}, i32> = BTreeMap::new();"));
                self.line(&format!("for ({x}, n) in vec![{keys}].iter().zip(vec![{ints}].iter()) {{"));
                self.line(&format!("    if let Some(o) = {m}.insert(*{x}, *n) {{"));
                self.line("        acc = acc.wrapping_add(o);");
                self.line("    }");
                self.line("}");
                self.line(&format!("if let Some(o) = {m}.remove(&{a}) {{"));
                self.line("    acc = acc.wrapping_sub(o);");
                self.line("}");
                self.line(&format!("for ({x}, n) in &{m} {{"));
                // `as` narrows; an `i8` widens by `from`.
                let low = if ty == "i8" { format!("i32::from(*{x} % 100)") } else { format!("(*{x} % 100) as i32") };
                self.line(&format!("    acc = acc.wrapping_mul(31).wrapping_add(*n).wrapping_add({low});"));
                self.line("}");
                self.line(&format!("flag = flag != {m}.contains_key(&{b});"));
            }
            1 => {
                let ints = (0..n).map(|_| self.int_lit()).collect::<Vec<_>>().join(", ");
                let (a, b) = (self.int_lit(), self.int_lit());
                self.line(&format!("let mut {m}: BTreeSet<i32> = vec![{ints}].into_iter().collect();"));
                self.line(&format!("flag = {m}.insert({a}) != flag;"));
                self.line(&format!("if {m}.remove(&{b}) {{"));
                self.line("    acc = acc.wrapping_add(1);");
                self.line("}");
                self.line(&format!("for {x} in &{m} {{"));
                self.line(&format!("    acc = acc.wrapping_mul(7).wrapping_add(*{x});"));
                self.line("}");
            }
            2 => {
                let lits =
                    ["9u64", "10u64", "0u64", "9007199254740993u64", "9007199254740992u64", "18446744073709551615u64"];
                let keys = (0..n).map(|_| self.rng.pick(&lits)).collect::<Vec<_>>().join(", ");
                let a = self.rng.pick(&lits);
                self.line(&format!(
                    "let {m}: BTreeMap<Id, usize> = vec![{keys}].iter().map(|n| (Id(*n), (*n % 7) as usize)).collect();"
                ));
                self.line(&format!("acc = acc.wrapping_add({m}.get(&Id({a})).copied().unwrap_or(9) as i32);"));
                self.line(&format!(
                    "acc = {m}.keys().fold(acc, |t, key| t.wrapping_mul(3).wrapping_add((key.0 % 1000) as i32));"
                ));
                self.line(&format!("acc = acc.wrapping_add({m}.values().map(|n| *n as i32).sum::<i32>());"));
            }
            _ => {
                let chars = ["a", "b", "\\u{e9}", "\\u{fffd}", "\\u{ff61}", "\\u{1f600}", "\\u{10ffff}", "\\u{10000}"];
                let text = (0..n).map(|_| self.rng.pick(&chars)).collect::<String>();
                let c = self.rng.pick(&chars);
                // Each `char` in key order, by code point.
                self.line(&format!("let {m}: BTreeSet<char> = \"{text}\".chars().collect();"));
                self.line(&format!("for {x} in &{m} {{"));
                self.line(&format!("    acc = acc.wrapping_mul(5).wrapping_add(u32::from(*{x}) as i32);"));
                self.line("}");
                self.line(&format!("flag = flag != {m}.contains(&'{c}');"));
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
    let mut g = G {
        rng: Rng::new(seed ^ 0xc011),
        extra: Rng::new(seed ^ 0x0221),
        keyed: Rng::new(seed ^ 0x0230),
        scope: Vec::new(),
        next: 0,
        out: String::new(),
        indent: 1,
        ret: T::Int,
    };
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
