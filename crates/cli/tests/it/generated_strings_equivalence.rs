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
//! the trims run on a `String` local as well as on a `&str`. From 0.21.0:
//! `to_lowercase` / `to_uppercase`, `split` with a `&str` (a literal, or a
//! parameter that may be empty), `split_whitespace`, and `lines` in a `for`,
//! into `map` and `collect`, and into `count`; `join`, `repeat`, and
//! `to_string`. From 0.22.0: a struct and an enum per seed with `impl
//! Display` made of writes (arguments that panic, specs, `?` and `Ok(())`,
//! a `let`, the struct printed inside the enum, `write_str` of a literal or
//! a computed text, a bare arm, an arm block with a `let`, the `match`
//! followed by a write), and an enum with §7.11's fixed text in one of its
//! shapes, printed by `format!` and `to_string`, and each once by
//! `x<seed>_types`. Inputs hold every Unicode White_Space code point, the
//! near misses U+FEFF, U+180E, and U+200B, cased letters inside and outside
//! ASCII (final sigma, `ß`, `İ`, `ﬁ`), `\r`, `\n`, and `\r\n`, characters
//! of every UTF-8 width, the code points around the surrogate gap, and
//! patterns that are empty, repeated, or overlapping.
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
    'ς',
    'ﬁ',
    '\r',
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
    "\n",
    "\r\n",
    "ΟΣ",
];

struct G {
    rng: Rng,
    next: usize,
    /// Immutable locals and closure or loop parameters, with their types.
    scope: Vec<(String, T)>,
    /// The seed, which names the seed's struct `D<seed>` and enum
    /// `E<seed>` with `Display` (`display_types`).
    seed: u64,
}

/// Text between the arguments of a `write!`, escapes included.
const FMT_TEXT: &[&str] = &["", "", "-", " ", "é", "😀", "{{", "}}", "`", "${{x}}", "\\\\", "\u{85}"];

impl G {
    /// A spec for a value of a primitive type; zero padding on an integer.
    fn spec(&mut self, int: bool) -> &'static str {
        match self.rng.below(9) {
            0 if int => ":05",
            1 => ":5",
            2 => ":<4",
            3 => ":^5",
            4 => ":>3",
            5 => ":*^6",
            6 => ":😀>4",
            _ => "",
        }
    }

    /// One `write!(f, ..)` whose arguments are drawn from `values` (each an
    /// expression and whether it is an integer), placed once each in the
    /// order written, each with a drawn spec; `shown` (crate types with
    /// `Display`, no spec) come last, so no argument that computes runs
    /// after one.
    fn write(&mut self, values: &[(&str, bool)], shown: &[&str]) -> String {
        let mut template = String::new();
        let mut args = Vec::new();
        for _ in 0..self.rng.below(3) {
            template.push_str(self.rng.pick(FMT_TEXT));
            let (value, int) = self.rng.pick(values);
            let spec = self.spec(int);
            write!(template, "{{{spec}}}").expect("write");
            args.push(value.to_string());
        }
        for s in shown {
            template.push_str(self.rng.pick(FMT_TEXT));
            write!(template, "{{{s}}}").expect("write");
        }
        template.push_str(self.rng.pick(FMT_TEXT));
        let args: String = args.iter().map(|a| format!(", {a}")).collect();
        format!("write!(f, \"{template}\"{args})")
    }

    /// One write that holds no crate type: a `write!`, or a third of the
    /// time `f.write_str` of one of `texts` (each a `&str`, a literal or
    /// computed).
    fn plain_write(&mut self, values: &[(&str, bool)], texts: &[&str]) -> String {
        if self.rng.below(3) == 0 {
            format!("f.write_str({})", self.rng.pick(texts))
        } else {
            self.write(values, &[])
        }
    }

    /// One to three writes joined with `?`, the last bare or followed by
    /// `Ok(())`; `shown` go in the last, which is a `write!`.
    fn writes(&mut self, values: &[(&str, bool)], texts: &[&str], shown: &[&str], indent: &str) -> String {
        let mut out = String::new();
        let n = 1 + self.rng.below(3);
        for i in 0..n {
            let w = if i + 1 == n && !shown.is_empty() {
                self.write(values, shown)
            } else {
                self.plain_write(values, texts)
            };
            if i + 1 < n {
                writeln!(out, "{indent}{w}?;").expect("write");
            } else if self.rng.below(2) == 0 {
                writeln!(out, "{indent}{w}?;\n{indent}Ok(())").expect("write");
            } else {
                writeln!(out, "{indent}{w}").expect("write");
            }
        }
        out
    }

    /// `D<seed>` and `E<seed>`, each with a `Display` made of writes: of a
    /// `String`, a `usize`, and a `char` field and values computed from them
    /// that may panic (`n - 1` at 0, a slice off a char boundary), as
    /// `write!` arguments or `f.write_str` texts (a field, a `String` built,
    /// a `let` the writes capture, a literal), and in `E` a `D` printed
    /// through its own `Display`, an arm that is one bare write or a block
    /// with a `let`, and the `match` itself one write followed by another.
    /// `Fixed<seed>` has §7.11's text fixed per value, in each of its shapes.
    fn display_types(&mut self) -> String {
        let s = self.seed;
        const D_VALUES: &[(&str, bool)] = &[
            ("self.w", false),
            ("self.w.trim()", false),
            ("&self.w[1..]", false),
            ("self.w.to_uppercase()", false),
            ("self.n", true),
            ("self.n + self.w.len()", true),
            ("self.n - 1", true),
            ("self.c", false),
            ("self.c.to_ascii_uppercase()", false),
            ("self.w.is_empty()", false),
            ("k", true),
        ];
        const D_TEXTS: &[&str] =
            &["&self.w", "self.w.trim()", "&self.w[1..]", "&self.w.to_uppercase()", "&t", "\"é\"", "\"`${x}` {}\""];
        let lead = "        let k = self.w.chars().count() * 2;\n        let t = format!(\"{}/{k}\", self.c);\n";
        let d_body = format!("{lead}{}", self.writes(D_VALUES, D_TEXTS, &[], "        "));
        let a = self.writes(&[("d.n", true), ("d.c", false)], &["&d.w", "\"<\""], &["d"], "                ");
        let mut b_values = vec![("n", true), ("w", false), ("w.len() - n", true)];
        let b_texts = &["w", "w.trim()", "&w[1..]", "\"-\""];
        let b = match self.rng.below(3) {
            0 => format!("{},", self.plain_write(&b_values, b_texts)),
            1 => {
                b_values.push(("m", true));
                let writes = self.writes(&b_values, b_texts, &[], "                ");
                format!("{{\n                let m = w.len() + n;\n{writes}            }}")
            }
            _ => format!("{{\n{}            }}", self.writes(&b_values, b_texts, &[], "                ")),
        };
        let c = self.rng.pick(&["f.write_str(\"c\")", "Ok(())", "f.write_str(\"`${é}`\")"]);
        let after = self.rng.pick(&["", "?;\n        f.write_str(\"!\")", "?;\n        Ok(())"]);
        let fixed = match self.rng.below(4) {
            0 => format!("f.write_str(match self {{\n            Fixed{s}::X => \"x\",\n            Fixed{s}::Y => \"y {{}} é\",\n        }})"),
            1 => format!(
                "match self {{\n            Fixed{s}::X => write!(f, \"x {{{{b}}}}\"),\n            Fixed{s}::Y => f.write_str(\"`${{y}}`\"),\n        }}"
            ),
            2 => format!(
                "let t = match self {{\n            Fixed{s}::X => \"\",\n            Fixed{s}::Y => \"😀\",\n        }};\n        f.write_str(t)"
            ),
            _ => "write!(f, \"fixed {{}}\")".to_string(),
        };
        format!(
            "pub struct D{s} {{\n    pub w: String,\n    pub n: usize,\n    pub c: char,\n}}\n\n\
             impl std::fmt::Display for D{s} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n{d_body}    }}\n}}\n\n\
             pub enum E{s} {{\n    A(D{s}),\n    B {{ n: usize, w: String }},\n    C,\n}}\n\n\
             impl std::fmt::Display for E{s} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        match self {{\n            \
             E{s}::A(d) => {{\n{a}            }}\n            E{s}::B {{ n, w }} => {b}\n            E{s}::C => {c},\n        }}{after}\n    }}\n}}\n\n\
             pub enum Fixed{s} {{\n    X,\n    Y,\n}}\n\n\
             impl std::fmt::Display for Fixed{s} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        {fixed}\n    }}\n}}\n\n"
        )
    }

    /// A `D<seed>`, `E<seed>`, or `Fixed<seed>` printed by `format!` (alone, or
    /// after a value computed first) or `to_string`.
    fn shown(&mut self, d: u32) -> String {
        let s = self.seed;
        let dv = format!("D{s} {{ w: {}, n: {}, c: {} }}", self.owned(d), self.size(d), self.ch());
        let value = match self.rng.below(5) {
            0 => dv,
            1 => format!("E{s}::A({dv})"),
            2 => format!("E{s}::B {{ n: {}, w: String::from({}) }}", self.size(d), self.text(d)),
            3 => format!("E{s}::C"),
            _ => format!("Fixed{s}::{}", self.rng.pick(&["X", "Y"])),
        };
        match self.rng.below(3) {
            0 => format!("{value}.to_string()"),
            1 => format!("format!(\"[{{}}]\", {value})"),
            _ => format!("format!(\"{{}}|{{}}\", {}, {value})", self.size(d)),
        }
    }

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

    /// A `&str` separator for `split`: a non-empty literal, or `t` or a
    /// local, which may be empty (a literal `""` is refused).
    fn sep(&mut self) -> String {
        if let Some(v) = self.local(T::Str) {
            return v;
        }
        match self.rng.below(3) {
            0 => "t".into(),
            _ => format!("{:?}", self.rng.pick(&PIECES[2..])),
        }
    }

    /// `s.split(p)` with a `&str`, `s.split_whitespace()`, or `s.lines()`.
    fn pieces(&mut self, d: u32) -> String {
        let x = self.text(d);
        match self.rng.below(3) {
            0 => format!("{x}.split({})", self.sep()),
            1 => format!("{x}.split_whitespace()"),
            _ => format!("{x}.lines()"),
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
        match self.rng.below(16) {
            13 => format!("{}.count()", self.pieces(d)),
            14 => format!("{}.to_string().len()", self.size(d)),
            15 => format!("{}.map(|y| y.chars().count()).sum::<usize>()", self.pieces(d)),
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
        if d > 0 && self.rng.below(6) == 0 {
            return self.shown(d - 1);
        }
        match self.rng.below(if d > 0 { 16 } else { 14 }) {
            14 | 15 => self.formatted(d - 1),
            8 => format!("{}.to_lowercase()", self.text(d)),
            9 => format!("{}.to_uppercase()", self.text(d)),
            10 => format!("{}.repeat({})", self.text(d), self.rng.pick(&["0usize", "1usize", "3usize", "n"])),
            11 => {
                let p = self.pieces(d);
                format!("{p}.collect::<Vec<&str>>().join({})", self.needle())
            }
            12 => match self.rng.below(4) {
                0 => format!("{}.to_string()", self.size(d)),
                1 => format!("{}.to_string()", self.ch()),
                2 => format!("{}.is_empty().to_string()", self.text(d)),
                _ => format!("{}.to_string()", self.text(d)),
            },
            13 => format!("{}.to_lowercase().to_uppercase()", self.text(d)),
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

    /// `format!` over one to three arguments of each type it prints, which
    /// may panic (a slice off a char boundary, `usize` past 2^53), placed
    /// once each in the order written; locals captured inline (`{x}`), any
    /// number of times; widths, alignments, fills, and zero padding; and
    /// text a template literal reads as its own (`` ` ``, `${`, `\\`).
    fn formatted(&mut self, d: u32) -> String {
        const TEXT: &[&str] = &["", "", "-", " ", "é", "😀", "{{", "}}", "`", "${{x}}", "\\\\", "\u{85}"];
        let mut template = String::new();
        let mut args: Vec<String> = Vec::new();
        for _ in 0..1 + self.rng.below(3) {
            template.push_str(self.rng.pick(TEXT));
            let (value, int) = match self.rng.below(5) {
                0 => (self.text(d), false),
                1 => (self.ch(), false),
                2 => (self.size(d), true),
                3 => (self.boolean(d), false),
                _ => (self.owned(d), false),
            };
            let spec = match self.rng.below(9) {
                0 if int => ":05".to_string(),
                1 if int => ":01".to_string(),
                2 => ":5".into(),
                3 => ":<4".into(),
                4 => ":^5".into(),
                5 => ":>3".into(),
                6 => ":*^6".into(),
                7 => ":😀>4".into(),
                _ => String::new(),
            };
            // A local is captured by name; it reads the same wherever the
            // template places it.
            let named: Vec<String> = self
                .scope
                .iter()
                .filter(|(n, t)| {
                    n.chars().all(|c| c.is_alphanumeric()) && matches!(t, T::Str | T::String | T::Char | T::Usize)
                })
                .map(|(n, _)| n.clone())
                .collect();
            if !named.is_empty() && self.rng.below(3) == 0 {
                let n = self.rng.pick(&named);
                // A name may be bound again with another type: the last binding counts.
                let int = self.scope.iter().rev().find(|(m, _)| *m == n).is_some_and(|(_, t)| *t == T::Usize);
                let spec = if spec.starts_with(":0") && !int { String::new() } else { spec };
                write!(template, "{{{n}{spec}}}").expect("write");
            } else {
                write!(template, "{{{spec}}}").expect("write");
                args.push(value);
            }
        }
        template.push_str(self.rng.pick(TEXT));
        let args: String = args.iter().map(|a| format!(", {a}")).collect();
        format!("format!(\"{template}\"{args})")
    }

    /// A `String`, possibly a block that builds one; only where a block
    /// may stand (a `let` or the tail).
    fn owned_top(&mut self, d: u32) -> String {
        match self.rng.below(8) {
            7 => {
                let x = self.pieces(d);
                let w = self.fresh("w");
                let p = self.fresh("p");
                self.scope.push((p.clone(), T::Str));
                let piece = self.owned(1);
                self.scope.pop();
                let sep = self.ch();
                format!(
                    "{{\n        let mut {w} = String::new();\n        for {p} in {x} {{\n            {w}.push_str(&{piece});\n            {w}.push({sep});\n        }}\n        {w}\n    }}"
                )
            }
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
        match self.rng.below(8) {
            5 => {
                let p = self.pieces(d);
                format!("{p}.map(|y| String::from(y)).collect::<Vec<String>>()")
            }
            6 => {
                let p = self.pieces(d);
                format!("{p}.map(|y| y.to_lowercase()).collect::<Vec<String>>()")
            }
            7 => {
                let p = self.pieces(d);
                let v = self.fresh("v");
                format!("{{\n        let {v}: Vec<&str> = {p}.collect();\n        vec![{v}.join(\"\"), {v}.join({})]\n    }}", self.needle())
            }
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
        match self.rng.below(4) {
            3 => {
                let p = self.pieces(d);
                format!("{p}.map(|y| y.len()).collect::<Vec<usize>>()")
            }
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
    "ΟΔΟΣ ὈΔΥΣΣΕΎΣ σ.",
    "ﬁ ŉ ǅ straße",
    "a\r\nb\n\nc\r",
    "\r\n\r\n",
    "x\n",
    "\r",
    "\n\r a",
];

#[test]
fn generated_strings_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let mut source = String::new();
    let mut fns = Vec::new();
    for &seed in &seeds {
        let mut g = G { rng: Rng::new(seed ^ 0x5717), next: 0, scope: Vec::new(), seed };
        let mut types = G { rng: Rng::new(seed ^ 0xd15), next: 0, scope: Vec::new(), seed };
        source.push_str(&types.display_types());
        // Every type and variant printed once, so none of the `fmt`s can
        // be skipped unseen.
        let name = format!("x{seed}_types");
        writeln!(
            source,
            "pub fn {name}({PARAMS}) -> String {{\n    let d = D{seed} {{ w: String::from(s), n, c }};\n    \
             let a = E{seed}::A(D{seed} {{ w: String::from(t), n: n + 1, c }});\n    \
             let b = E{seed}::B {{ n, w: String::from(t) }};\n    let e = E{seed}::C;\n    \
             let x = Fixed{seed}::X;\n    let y = Fixed{seed}::Y;\n    format!(\"{{d}}|{{a}}|{{b}}|{{e}}|{{x}}|{{y}}\")\n}}\n"
        )
        .expect("write");
        fns.push(name);
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
    compare_rows("generated_strings", &seeds, &source, "", &fns, &rows);
}
