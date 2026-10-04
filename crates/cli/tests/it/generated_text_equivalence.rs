//! Text and lists generated from a seed: a `&str`, a `String`, and a
//! `Vec<i32>` read through the allowed methods (`len`, `is_empty`,
//! `starts_with` / `ends_with` / `contains`, `strip_prefix`, slicing at byte
//! positions, `split_once`, `parse`), `chars()`, `bytes()`, `split(c)`, and
//! `iter()` through `map` / `filter` / `copied` into `all`, `any`,
//! `position`, `count`, `sum`, and `collect`, indexing, `clone`, `vec!`, and
//! a local `Vec` grown with `push` in a `for`. Inputs hold multi-byte
//! characters, so a slice off a char boundary panics, as an index past the
//! end does; both are compared as panics, message included.
//!
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.

use std::fmt::Write as _;

use crate::support::generated::{compare_rows, fn_count, seeds, Args, SEEDS};
use crate::support::{Js, Rng};

#[derive(Clone, Copy, PartialEq)]
enum T {
    Usize,
    Bool,
    I32,
    Str,
    OptUsize,
    Vec,
    String,
}

impl T {
    fn rust(self) -> &'static str {
        match self {
            T::Usize => "usize",
            T::Bool => "bool",
            T::I32 => "i32",
            T::Str => "&str",
            T::OptUsize => "Option<usize>",
            T::Vec => "Vec<i32>",
            T::String => "String",
        }
    }
}

const PARAMS: &str = "s: &str, t: String, xs: Vec<i32>, a: i32";

/// Literals a test compares with: pieces of the inputs, and of nothing.
const PIECES: &[&str] = &["", "a", ",", "12", "é", "日", "ab", "-", "+1"];

struct G {
    rng: Rng,
    next: usize,
    /// Immutable locals, with their types.
    scope: Vec<(String, T)>,
}

impl G {
    fn piece(&mut self) -> String {
        format!("{:?}", self.rng.pick(PIECES))
    }

    fn local(&mut self, ty: T) -> Option<String> {
        let names: Vec<String> = self.scope.iter().filter(|(_, t)| *t == ty).map(|(n, _)| n.clone()).collect();
        (!names.is_empty() && self.rng.below(2) == 0).then(|| self.rng.pick(&names))
    }

    fn sep(&mut self) -> &'static str {
        self.rng.pick(&["','", "'é'", "'a'"])
    }

    fn text(&mut self, d: u32) -> String {
        if let Some(v) = self.local(T::Str) {
            return v;
        }
        if d == 0 {
            return self.rng.pick(&["s", "t.as_str()"]).into();
        }
        let d = d - 1;
        match self.rng.below(7) {
            0 => format!("(&{}[{}..{}])", self.text(d), self.size(d), self.size(d)),
            1 => format!("(&{}[{}..])", self.text(d), self.size(d)),
            2 => {
                let p = self.piece();
                let x = self.text(d);
                format!("{x}.strip_prefix({p}).unwrap_or({x})")
            }
            3 => {
                let p = self.piece();
                let x = self.text(d);
                format!("{x}.strip_suffix({p}).unwrap_or(\"\")")
            }
            _ => self.rng.pick(&["s", "t.as_str()"]).into(),
        }
    }

    fn size(&mut self, d: u32) -> String {
        if let Some(v) = self.local(T::Usize) {
            return v;
        }
        if d == 0 || self.rng.below(3) == 0 {
            return format!("{}usize", self.rng.pick(&[0, 1, 2, 3, 5]));
        }
        let d = d - 1;
        match self.rng.below(9) {
            0 => format!("{}.len()", self.text(d)),
            1 => format!("{}.chars().count()", self.text(d)),
            2 => {
                let sep = self.sep();
                format!("{}.split({sep}).count()", self.text(d))
            }
            3 => "xs.len()".into(),
            4 => format!("{}.bytes().filter(|b| *b == b',').count()", self.text(d)),
            5 => format!("({}).unwrap_or({})", self.opt(d), self.size(d)),
            6 => format!("xs.iter().filter(|x| **x > {}).count()", self.int(d)),
            7 => format!("({} + {})", self.size(d), self.size(d)),
            _ => format!("{}.min({})", self.size(d), self.size(d)),
        }
    }

    fn opt(&mut self, d: u32) -> String {
        match self.rng.below(3) {
            0 => format!("{}.chars().position(|c| c == {})", self.text(d), self.sep()),
            1 => format!("xs.iter().position(|x| *x == {})", self.rng.pick(&[0, 1, 3, -1])),
            _ => {
                let sep = self.sep();
                format!("{}.chars().position(|c| c.is_ascii_digit() || c == {sep})", self.text(d))
            }
        }
    }

    fn int(&mut self, d: u32) -> String {
        if let Some(v) = self.local(T::I32) {
            return v;
        }
        if d == 0 || self.rng.below(3) == 0 {
            return self.rng.pick(&["a", "0", "1", "-3", "7"]).into();
        }
        let d = d - 1;
        match self.rng.below(7) {
            0 => format!("{}.parse::<i32>().ok().unwrap_or({})", self.text(d), self.int(d)),
            1 => "xs.iter().sum::<i32>()".into(),
            2 => format!("xs[{}]", self.size(d)),
            3 => format!("({} + {})", self.int(d), self.int(d)),
            4 => format!("(if {} {{ {} }} else {{ {} }})", self.boolean(d), self.int(d), self.int(d)),
            5 => format!("xs.iter().map(|x| x * {}).sum::<i32>()", self.rng.pick(&[2, -1, 3])),
            _ => {
                let x = self.text(d);
                let sep = self.sep();
                let none = self.int(d);
                format!("(match {x}.split_once({sep}) {{ Some((p, _)) => p.parse::<i32>().ok().unwrap_or(-1), None => {none} }})")
            }
        }
    }

    fn boolean(&mut self, d: u32) -> String {
        if d == 0 {
            return format!("{}.is_empty()", self.text(0));
        }
        let d = d - 1;
        match self.rng.below(10) {
            0 => format!("{}.starts_with({})", self.text(d), self.piece()),
            1 => format!("{}.ends_with({})", self.text(d), self.piece()),
            2 => format!("{}.contains({})", self.text(d), self.piece()),
            3 => format!("({} == {})", self.text(d), self.piece()),
            4 => format!("{}.chars().all(|c| c.is_ascii_digit())", self.text(d)),
            5 => format!("{}.chars().any(|c| matches!(c, 'a'..='z' | 'é'))", self.text(d)),
            6 => format!("xs.iter().any(|x| *x > {})", self.int(d)),
            7 => format!("({} < {})", self.size(d), self.size(d)),
            8 => format!("(t == {})", self.piece()),
            _ => format!("({} && {})", self.boolean(d), self.boolean(d)),
        }
    }

    fn list(&mut self, d: u32) -> String {
        match self.rng.below(6) {
            0 => "xs.clone()".into(),
            1 => format!("xs.iter().map(|x| x * {}).collect::<Vec<i32>>()", self.rng.pick(&[2, -1, 0])),
            2 => format!("xs.iter().filter(|x| **x > {}).copied().collect::<Vec<i32>>()", self.rng.pick(&[0, 1, -2])),
            3 => format!("vec![{}, {}]", self.int(d), self.int(d)),
            4 => {
                let sep = self.sep();
                format!(
                    "{}.split({sep}).map(|p| p.parse::<i32>().ok().unwrap_or(0)).collect::<Vec<i32>>()",
                    self.text(d)
                )
            }
            _ => {
                let test = self.rng.pick(&["*x > 0", "*x % 2 == 0", "*x != a"]);
                let push = self.rng.pick(&["*x", "*x + 1", "a"]);
                format!("{{\n        let mut v: Vec<i32> = Vec::new();\n        for x in &xs {{\n            if {test} {{\n                v.push({push});\n            }}\n        }}\n        v\n    }}")
            }
        }
    }

    /// A body of `ty`, after a `let` or two.
    fn body(&mut self, ty: T) -> String {
        let mut out = String::new();
        self.scope.clear();
        for _ in 0..self.rng.below(3) {
            let lt = self.rng.pick(&[T::Usize, T::I32, T::Str]);
            let value = match lt {
                T::Usize => self.size(2),
                T::I32 => self.int(2),
                _ => self.text(2),
            };
            self.next += 1;
            let v = format!("v{}", self.next);
            writeln!(out, "    let {v}: {} = {value};", lt.rust()).expect("write");
            self.scope.push((v, lt));
        }
        let tail = match ty {
            T::Usize => self.size(3),
            T::Bool => self.boolean(3),
            T::I32 => self.int(3),
            T::OptUsize => self.opt(3),
            T::Vec => self.list(2),
            T::String => format!("String::from({})", self.text(3)),
            T::Str => unreachable!("not returned"),
        };
        writeln!(out, "    {tail}").expect("write");
        out
    }
}

/// `Show` for the types these functions return, as `support::Show` prints
/// them (a string quoted, printable ASCII but `"` and `\` as is).
const SHOW_TEXT: &str = r#"
impl Show for usize { fn show(&self) -> String { self.to_string() } }
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
"#;

#[test]
fn generated_text_matches_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let mut source = String::new();
    let mut fns = Vec::new();
    for &seed in &seeds {
        let mut g = G { rng: Rng::new(seed ^ 0x7e47), next: 0, scope: Vec::new() };
        for i in 0..count {
            let ty = g.rng.pick(&[T::Usize, T::Bool, T::I32, T::OptUsize, T::Vec, T::String]);
            g.next = 0;
            let body = g.body(ty);
            let name = format!("x{seed}_f{i}");
            writeln!(source, "pub fn {name}({PARAMS}) -> {} {{\n{body}}}\n", ty.rust()).expect("write");
            fns.push(name);
        }
    }
    let strings = ["", "abc", "12", "-7", "+12", "a,b,,c", "é,a", "日本,12", "x", ",", "2147483648", "aé1"];
    let lists: [&[i32]; 5] = [&[], &[1], &[0, -3, 5], &[i32::MAX, 1], &[2, 2, 7, -1]];
    let mut rng = Rng::new(seeds[0] ^ 0x7e7);
    let rows: Vec<(&str, String, Vec<i32>, i32)> = (0..12)
        .map(|_| {
            let s = rng.pick(&strings);
            let t = rng.pick(&strings).to_string();
            let xs = rng.pick(&lists).to_vec();
            (s, t, xs, rng.pick(&[0, 1, -1, 3, i32::MAX]))
        })
        .collect();

    let rows: Vec<Args> = rows
        .iter()
        .map(|(s, t, xs, a)| Args {
            rust: format!("{s:?}, String::from({t:?}), vec!{xs:?}, {a}"),
            js: [s.js(), t.js(), xs.js(), a.js()].join(", "),
        })
        .collect();
    compare_rows("generated_text", &seeds, &source, SHOW_TEXT, &fns, &rows);
}
