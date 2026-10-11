//! String methods from 0.13.1, generated from a seed and chained with the
//! older ones: `to_ascii_lowercase` / `to_ascii_uppercase`, `trim` /
//! `trim_start` / `trim_end`, `trim_matches` / `trim_start_matches` /
//! `trim_end_matches` with a `char`, a closure, or (one-sided) a `&str`,
//! `find` with each of those as a UTF-8 byte offset that then slices the
//! text, and `split` with a closure in a `for`, through `map` / `filter`, and
//! into `collect`, `count`, `position`, and `max`; beside `len`,
//! `is_empty`, `starts_with` / `ends_with` / `contains`, `strip_prefix`,
//! `split_once`, `eq_ignore_ascii_case`, `chars()`, `bytes()`,
//! `String::push` / `push_str`, slicing (which panics off a char boundary),
//! `==`, and `<`; `find` also drives an `if let`, a `match` with literal and
//! guarded arms, and a `while` tokenizer that slices past each match, and
//! the trims run on a `String` local as well as on a `&str`. Inputs hold every Unicode White_Space code point, the
//! near misses U+FEFF, U+180E, and U+200B, cased letters inside and outside
//! ASCII, characters of every UTF-8 width, the code points around the
//! surrogate gap, and patterns that are empty, repeated, or overlapping.
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
    Char,
    Str,
    String,
    OptUsize,
    VecString,
    VecUsize,
}

impl T {
    fn rust(self) -> &'static str {
        match self {
            T::Usize => "usize",
            T::Bool => "bool",
            T::Char => "char",
            T::Str => "&str",
            T::String => "String",
            T::OptUsize => "Option<usize>",
            T::VecString => "Vec<String>",
            T::VecUsize => "Vec<usize>",
        }
    }
}

const PARAMS: &str = "s: &str, t: &str, c: char, n: usize";

/// Characters a pattern or a comparison names: whitespace Rust and JS
/// disagree on, near misses, cased letters, and every UTF-8 width.
const CHARS: &[char] = &[
    ' ',
    '\t',
    '\n',
    'a',
    'A',
    'b',
    '-',
    ',',
    'é',
    'É',
    'ß',
    'İ',
    'Σ',
    'σ',
    '日',
    '😀',
    '\u{85}',
    '\u{a0}',
    '\u{1680}',
    '\u{2003}',
    '\u{2028}',
    '\u{3000}',
    '\u{feff}',
    '\u{180e}',
    '\u{200b}',
    '\u{d7ff}',
    '\u{e000}',
    '\u{ffff}',
    '\u{10000}',
    '\u{10ffff}',
];

/// `&str` patterns: empty, repeated, overlapping, and of every width.
const PIECES: &[&str] = &[
    "",
    "",
    " ",
    "a",
    "A",
    "aa",
    "aba",
    "ab",
    "-",
    "--",
    "é",
    "É",
    "ß",
    "日",
    "😀",
    "😀😀",
    "\u{85}",
    "\u{feff}",
    " \t",
    "\u{3000}",
    "\u{10ffff}",
];

struct G {
    rng: Rng,
    next: usize,
    /// Immutable locals and closure or loop parameters, with their types.
    scope: Vec<(String, T)>,
}

impl G {
    fn fresh(&mut self, p: &str) -> String {
        self.next += 1;
        format!("{p}{}", self.next)
    }

    fn local(&mut self, ty: T) -> Option<String> {
        let names: Vec<String> = self.scope.iter().filter(|(_, t)| *t == ty).map(|(n, _)| n.clone()).collect();
        (!names.is_empty() && self.rng.below(2) == 0).then(|| self.rng.pick(&names))
    }

    fn piece(&mut self) -> String {
        format!("{:?}", self.rng.pick(PIECES))
    }

    fn ch(&mut self) -> String {
        if let Some(v) = self.local(T::Char) {
            return v;
        }
        if self.rng.below(3) == 0 {
            "c".into()
        } else {
            format!("{:?}", self.rng.pick(CHARS))
        }
    }

    /// A test of the `char` named `x` (an expression of type `char`).
    fn pred(&mut self, x: &str) -> String {
        match self.rng.below(13) {
            0 | 1 => format!("{x} == {}", self.ch()),
            2 => format!("{x}.is_ascii_digit()"),
            3 => format!("{x}.is_ascii_alphabetic()"),
            4 => format!("{x}.is_ascii_uppercase()"),
            5 => format!("!{x}.is_ascii()"),
            6 => format!("{x}.is_ascii_whitespace()"),
            7 => format!("{x} == ' ' || {x} == '\\t'"),
            8 => format!("matches!({x}, 'a'..='z' | 'é' | '\\u{{85}}')"),
            9 => format!("{x} < {}", self.ch()),
            10 => format!("u32::from({x}) > 0x7f"),
            11 => format!("{x}.to_ascii_lowercase() == {}", self.ch()),
            _ => format!("{x}.is_ascii_punctuation() || {x} == {}", self.ch()),
        }
    }

    fn closure(&mut self) -> String {
        let p = self.pred("x");
        format!("|x: char| {p}")
    }

    /// A `char` or a closure: what `trim_matches` takes.
    fn cpat(&mut self) -> String {
        if self.rng.below(2) == 0 {
            self.ch()
        } else {
            self.closure()
        }
    }

    /// A `&str` needle.
    fn needle(&mut self) -> String {
        if let Some(v) = self.local(T::Str) {
            return v;
        }
        match self.rng.below(4) {
            0 => "t".into(),
            _ => self.piece(),
        }
    }

    /// A `char`, a closure, or a `&str`: what `find` and the one-sided
    /// trims take.
    fn spat(&mut self) -> String {
        match self.rng.below(3) {
            0 => self.ch(),
            1 => self.closure(),
            _ => self.needle(),
        }
    }

    fn text(&mut self, d: u32) -> String {
        if self.rng.below(3) == 0 {
            if let Some(v) = self.local(T::Str) {
                return v;
            }
            if let Some(v) = self.local(T::String) {
                return format!("{v}.as_str()");
            }
        }
        if d == 0 {
            return self.rng.pick(&["s", "t"]).into();
        }
        let d = d - 1;
        match self.rng.below(15) {
            0 => format!("{}.trim()", self.text(d)),
            1 => format!("{}.trim_start()", self.text(d)),
            2 => format!("{}.trim_end()", self.text(d)),
            3 => {
                let x = self.text(d);
                format!("{x}.trim_matches({})", self.cpat())
            }
            4 => {
                let x = self.text(d);
                format!("{x}.trim_start_matches({})", self.spat())
            }
            5 => {
                let x = self.text(d);
                format!("{x}.trim_end_matches({})", self.spat())
            }
            6 => {
                let x = self.text(d);
                let p = self.spat();
                let i = self.fresh("i");
                format!("(match {x}.find({p}) {{ Some({i}) => &{x}[..{i}], None => {x} }})")
            }
            7 => {
                let x = self.text(d);
                let i = self.fresh("i");
                let (p, skip) = if self.rng.below(2) == 0 {
                    let p = self.ch();
                    let skip = format!("{p}.len_utf8()");
                    (p, skip)
                } else {
                    let p = self.needle();
                    let skip = format!("{p}.len()");
                    (p, skip)
                };
                format!("(match {x}.find({p}) {{ Some({i}) => &{x}[{i} + {skip}..], None => \"\" }})")
            }
            8 => format!("(&{}[{}..])", self.text(d), self.size(d)),
            9 => format!("(&{}[..{}])", self.text(d), self.size(d)),
            10 => {
                let x = self.text(d);
                format!("{x}.strip_prefix({}).unwrap_or({x})", self.needle())
            }
            11 => {
                let x = self.text(d);
                let p = if self.rng.below(2) == 0 { self.ch() } else { self.needle() };
                let b = self.fresh("b");
                format!("(match {x}.split_once({p}) {{ Some((_, {b})) => {b}, None => {x} }})")
            }
            12 => match self.local(T::String) {
                Some(v) => {
                    let p = self.cpat();
                    self.rng.pick(&[
                        format!("{v}.trim()"),
                        format!("{v}.trim_matches({p})"),
                        format!("{v}.trim_end_matches({p})"),
                    ])
                }
                None => "s".into(),
            },
            _ => self.rng.pick(&["s", "t"]).into(),
        }
    }

    fn size(&mut self, d: u32) -> String {
        if let Some(v) = self.local(T::Usize) {
            return v;
        }
        if d == 0 || self.rng.below(4) == 0 {
            return self.rng.pick(&["n", "0usize", "1usize", "2usize", "3usize", "4usize"]).into();
        }
        let d = d - 1;
        match self.rng.below(13) {
            0 => format!("{}.len()", self.text(d)),
            1 => format!("{}.chars().count()", self.text(d)),
            2 => {
                let x = self.text(d);
                format!("{x}.find({}).unwrap_or({})", self.spat(), self.size(d))
            }
            3 => {
                let x = self.text(d);
                format!("{x}.split({}).count()", self.closure())
            }
            4 => format!("{}.len()", self.owned(d)),
            5 => {
                let x = self.text(d);
                let p = self.pred("(*y)");
                format!("{x}.chars().filter(|y| {p}).count()")
            }
            6 => {
                let x = self.text(d);
                format!("{x}.split({}).collect::<Vec<&str>>().len()", self.closure())
            }
            7 => format!("({} + {})", self.size(d), self.size(d)),
            8 => format!("{}.min({})", self.size(d), self.size(d)),
            9 => format!("{}.bytes().filter(|b| *b > 0x7f).count()", self.text(d)),
            10 => {
                let x = self.text(d);
                let cl = self.closure();
                format!("{x}.split({cl}).map(|y| y.len()).max().unwrap_or({})", self.size(d))
            }
            11 => format!("({}).unwrap_or({})", self.opt(d), self.size(d)),
            _ => {
                let o = self.opt(d);
                let j = self.fresh("j");
                let a = self.size(d);
                let b = self.size(d);
                format!("(match {o} {{ Some(0) => {a}, Some({j}) if {j} > n => {j} - n, Some({j}) => {j} + {b}, None => n }})")
            }
        }
    }

    fn opt(&mut self, d: u32) -> String {
        match self.rng.below(8) {
            0 => format!("{}.find({})", self.text(d), self.ch()),
            1 => format!("{}.find({})", self.text(d), self.needle()),
            2 => format!("{}.find({})", self.text(d), self.closure()),
            3 => format!("{}.chars().position({})", self.text(d), self.closure()),
            4 => format!("{}.find({})", self.owned(d), self.spat()),
            5 => format!("{}.find({}).map(|i| i + 1)", self.text(d), self.spat()),
            6 => format!("{}.split({}).position(|y| y.is_empty())", self.text(d), self.closure()),
            _ => {
                let x = self.text(d);
                let p = self.spat();
                format!("{x}.trim().find({p})")
            }
        }
    }

    fn boolean(&mut self, d: u32) -> String {
        if d == 0 {
            return format!("{}.is_empty()", self.text(0));
        }
        let d = d - 1;
        match self.rng.below(16) {
            0 => format!("{}.starts_with({})", self.text(d), self.needle()),
            1 => format!("{}.ends_with({})", self.text(d), self.needle()),
            2 => format!("{}.contains({})", self.text(d), self.needle()),
            3 => format!("({} == {})", self.text(d), self.text(d)),
            4 => format!("({} < {})", self.text(d), self.text(d)),
            5 => format!("{}.eq_ignore_ascii_case({})", self.text(d), self.text(d)),
            6 => format!("{}.find({}).is_some()", self.text(d), self.spat()),
            7 => format!("({} == {})", self.opt(d), self.opt(d)),
            8 => format!("({} == {})", self.owned(d), self.owned(d)),
            9 => format!("({} < {})", self.size(d), self.size(d)),
            10 => format!("{}.chars().any({})", self.text(d), self.closure()),
            11 => format!("{}.chars().all({})", self.text(d), self.closure()),
            12 => format!("({} && {})", self.boolean(d), self.boolean(d)),
            13 => format!("({} == {})", self.owned(d), self.text(d)),
            14 => format!("({} < {})", self.owned(d), self.owned(d)),
            _ => format!("{}.trim().is_empty()", self.text(d)),
        }
    }

    /// A `String`, without a block.
    fn owned(&mut self, d: u32) -> String {
        if let Some(v) = self.local(T::String) {
            return format!("{v}.clone()");
        }
        match self.rng.below(8) {
            0 | 1 => format!("{}.to_ascii_lowercase()", self.text(d)),
            2 | 3 => format!("{}.to_ascii_uppercase()", self.text(d)),
            4 => format!("String::from({})", self.text(d)),
            5 => format!("{}.chars().map(|y| y.to_ascii_lowercase()).collect::<String>()", self.text(d)),
            6 => {
                let x = self.text(d);
                let p = self.pred("(*y)");
                format!("{x}.chars().filter(|y| {p}).collect::<String>()")
            }
            _ => {
                let x = self.text(d);
                format!("{x}.trim().to_ascii_uppercase()")
            }
        }
    }

    /// A `String`, possibly a block that builds one; only where a block
    /// may stand (a `let` or the tail).
    fn owned_top(&mut self, d: u32) -> String {
        match self.rng.below(7) {
            4 => {
                // A tokenizer: `find` a separator, take what precedes it,
                // and go on past it, so a wrong offset shows.
                let x = self.text(d);
                let w = self.fresh("w");
                let r = self.fresh("r");
                let i = self.fresh("i");
                let (p, skip) = if self.rng.below(2) == 0 {
                    let p = self.ch();
                    let skip = format!("{p}.len_utf8()");
                    (p, skip)
                } else {
                    let p = format!("{:?}", self.rng.pick(&PIECES[2..]));
                    let skip = format!("{p}.len()");
                    (p, skip)
                };
                self.scope.push((r.clone(), T::Str));
                let piece = format!("(&{r}[..{i}])");
                self.scope.push((piece, T::Str));
                let item = self.text(1);
                self.scope.pop();
                self.scope.pop();
                let sep = self.ch();
                format!(
                    "{{\n        let mut {w} = String::new();\n        let mut {r}: &str = {x};\n        while !{r}.is_empty() {{\n            match {r}.find({p}) {{\n                Some({i}) => {{\n                    {w}.push_str({item});\n                    {w}.push({sep});\n                    {r} = &{r}[{i} + {skip}..];\n                }}\n                None => {{\n                    {w}.push_str({r}.trim_end());\n                    {r} = \"\";\n                }}\n            }}\n        }}\n        {w}\n    }}"
                )
            }
            5 => {
                // `if let` on `find`, slicing after the match.
                let x = self.text(d);
                let p = self.spat();
                let i = self.fresh("i");
                let v = self.fresh("v");
                let other = self.owned(d);
                format!(
                    "{{\n        let {v}: &str = {x};\n        if let Some({i}) = {v}.find({p}) {{\n            (&{v}[{i}..]).to_ascii_uppercase()\n        }} else {{\n            {other}\n        }}\n    }}"
                )
            }
            0 => {
                let x = self.text(d);
                let cl = self.closure();
                let w = self.fresh("w");
                let p = self.fresh("p");
                self.scope.push((p.clone(), T::Str));
                let piece = self.text(1);
                self.scope.pop();
                let sep = self.ch();
                format!(
                    "{{\n        let mut {w} = String::new();\n        for {p} in {x}.split({cl}) {{\n            {w}.push_str({piece});\n            {w}.push({sep});\n        }}\n        {w}\n    }}"
                )
            }
            1 => {
                let x = self.text(d);
                let w = self.fresh("w");
                let k = self.fresh("k");
                self.scope.push((k.clone(), T::Char));
                let test = self.pred(&k);
                let other = self.ch();
                self.scope.pop();
                format!(
                    "{{\n        let mut {w} = String::new();\n        for {k} in {x}.chars() {{\n            if {test} {{\n                {w}.push({k}.to_ascii_uppercase());\n            }} else {{\n                {w}.push({other});\n            }}\n        }}\n        {w}\n    }}"
                )
            }
            _ => self.owned(d),
        }
    }

    fn vec_string(&mut self, d: u32) -> String {
        let x = self.text(d);
        let cl = self.closure();
        match self.rng.below(5) {
            0 => format!("{x}.split({cl}).map(|y| String::from(y.trim())).collect::<Vec<String>>()"),
            1 => format!("{x}.split({cl}).map(|y| y.to_ascii_uppercase()).collect::<Vec<String>>()"),
            2 => {
                let v = self.fresh("v");
                let o = self.fresh("o");
                let y = self.fresh("y");
                let p = self.cpat();
                format!(
                    "{{\n        let {v}: Vec<&str> = {x}.split({cl}).collect();\n        let mut {o}: Vec<String> = Vec::new();\n        for {y} in &{v} {{\n            {o}.push(String::from({y}.trim_matches({p})));\n        }}\n        {o}\n    }}"
                )
            }
            3 => {
                let o = self.fresh("o");
                let y = self.fresh("y");
                self.scope.push((y.clone(), T::Str));
                let item = self.owned(1);
                self.scope.pop();
                format!(
                    "{{\n        let mut {o}: Vec<String> = Vec::new();\n        for {y} in {x}.split({cl}) {{\n            if !{y}.is_empty() {{\n                {o}.push({item});\n            }}\n        }}\n        {o}\n    }}"
                )
            }
            _ => format!("{x}.split({cl}).filter(|y| !y.is_empty()).map(|y| String::from(y)).collect::<Vec<String>>()"),
        }
    }

    fn vec_usize(&mut self, d: u32) -> String {
        let x = self.text(d);
        let cl = self.closure();
        match self.rng.below(3) {
            0 => format!("{x}.split({cl}).map(|y| y.len()).collect::<Vec<usize>>()"),
            1 => {
                let p = self.spat();
                format!("{x}.split({cl}).map(|y| y.find({p}).unwrap_or(n)).collect::<Vec<usize>>()")
            }
            _ => {
                let o = self.fresh("o");
                let y = self.fresh("y");
                self.scope.push((y.clone(), T::Str));
                let item = self.size(1);
                self.scope.pop();
                format!(
                    "{{\n        let mut {o}: Vec<usize> = Vec::new();\n        for {y} in {x}.split({cl}) {{\n            {o}.push({item});\n        }}\n        {o}\n    }}"
                )
            }
        }
    }

    /// A body of `ty`, after a `let` or three.
    fn body(&mut self, ty: T) -> String {
        let mut out = String::new();
        self.scope.clear();
        for _ in 0..self.rng.below(4) {
            let lt = self.rng.pick(&[T::Usize, T::Str, T::Str, T::String, T::Char, T::OptUsize]);
            let v = self.fresh("v");
            match lt {
                T::Usize => {
                    let value = self.size(2);
                    writeln!(out, "    let {v}: usize = {value};").expect("write");
                }
                T::Str => {
                    let value = self.text(2);
                    writeln!(out, "    let {v}: &str = {value};").expect("write");
                }
                T::Char => {
                    let x = self.text(1);
                    let d = self.ch();
                    // The greatest `char` of a text, or `d`.
                    let k = self.fresh("k");
                    let m = self.fresh("m");
                    writeln!(
                        out,
                        "    let mut {m}: char = {d};\n    for {k} in {x}.chars() {{\n        if {k} > {m} {{\n            {m} = {k};\n        }}\n    }}\n    let {v}: char = {m};"
                    )
                    .expect("write");
                }
                T::String => {
                    if self.rng.below(3) == 0 {
                        let value = self.owned(2);
                        let push = self.text(1);
                        let pc = self.ch();
                        let m = self.fresh("m");
                        writeln!(out, "    let mut {m}: String = {value};\n    {m}.push_str({push});\n    {m}.push({pc});\n    let {v}: String = {m};")
                            .expect("write");
                    } else {
                        let value = self.owned_top(2);
                        writeln!(out, "    let {v}: String = {value};").expect("write");
                    }
                }
                _ => {
                    if ty == T::OptUsize {
                        let x = self.text(1);
                        let p = self.spat();
                        writeln!(out, "    let {v}: usize = {x}.find({p})?;").expect("write");
                        self.scope.push((v, T::Usize));
                    } else {
                        let value = self.opt(2);
                        let w = self.fresh("v");
                        let dflt = self.size(1);
                        writeln!(
                            out,
                            "    let {v}: Option<usize> = {value};\n    let {w}: usize = {v}.unwrap_or({dflt});"
                        )
                        .expect("write");
                        self.scope.push((w, T::Usize));
                    }
                    continue;
                }
            }
            self.scope.push((v, lt));
        }
        let tail = match ty {
            T::Usize => self.size(3),
            T::Bool => self.boolean(3),
            T::OptUsize => {
                if self.rng.below(3) == 0 {
                    format!("Some({})", self.size(2))
                } else {
                    self.opt(3)
                }
            }
            T::String => {
                if self.rng.below(3) == 0 {
                    format!("String::from({})", self.text(3))
                } else {
                    self.owned_top(3)
                }
            }
            T::VecString => self.vec_string(2),
            T::VecUsize => self.vec_usize(2),
            T::Char | T::Str => unreachable!("not returned"),
        };
        writeln!(out, "    {tail}").expect("write");
        out
    }
}

/// `Show` for the types these functions return, as `support::Show` prints
/// them (a string quoted, printable ASCII but `"` and `\` as is).
const SHOW_STRINGS: &str = r#"
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

/// Texts the parameters take: empty, every White_Space code point, the
/// near misses, cased letters, every UTF-8 width, the surrogate gap's
/// neighbors, and repeats that overlap.
const TEXTS: &[&str] = &[
    "",
    " ",
    "a",
    "Ab",
    "  a b  ",
    "\t\n\u{b}\u{c}\r x \u{20}",
    "\u{85}a\u{85}",
    "\u{a0}\u{1680}x\u{2000}\u{2001}",
    "\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}y",
    "\u{2028}z\u{2029}\u{202f}\u{205f}\u{3000}",
    "\u{feff}z\u{180e}\u{200b}",
    "\u{200b} \u{feff}",
    "ÉCOLE école ß İ Σσς",
    "日本語",
    "😀a😀",
    "\u{d7ff}\u{e000}\u{ffff}\u{10000}\u{10ffff}",
    "aaaa",
    "ababa",
    "--a--b--",
    "a-b,c;d",
    "AbC-déF",
    "x😀\u{85}é",
    "😀😀😀",
    " é ",
];

#[test]
fn generated_strings_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let mut source = String::new();
    let mut fns = Vec::new();
    for &seed in &seeds {
        let mut g = G { rng: Rng::new(seed ^ 0x5717), next: 0, scope: Vec::new() };
        for i in 0..count {
            let ty = g.rng.pick(&[T::Usize, T::Bool, T::OptUsize, T::String, T::String, T::VecString, T::VecUsize]);
            g.next = 0;
            let body = g.body(ty);
            let name = format!("x{seed}_f{i}");
            writeln!(source, "pub fn {name}({PARAMS}) -> {} {{\n{body}}}\n", ty.rust()).expect("write");
            fns.push(name);
        }
    }
    let mut rng = Rng::new(seeds[0] ^ 0x57e);
    // Every text as `s`, against a drawn `t`, `c`, and `n`.
    let rows: Vec<Args> = TEXTS
        .iter()
        .map(|&s| {
            let t = rng.pick(TEXTS);
            let c = rng.pick(CHARS);
            let n = rng.pick(&[0usize, 1, 2, 3, 5, 8]);
            Args { rust: format!("{s:?}, {t:?}, {c:?}, {n}"), js: [s.js(), t.js(), c.js(), n.js()].join(", ") }
        })
        .collect();
    compare_rows("generated_strings", &seeds, &source, SHOW_STRINGS, &fns, &rows);
}
