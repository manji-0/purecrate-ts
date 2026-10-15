//! Transitions generated from a seed, over the crate's own types: a `match`
//! on `(state, event)` (or on one of them, `if let`, or `matches!`) whose
//! arms test variants, tuple and struct fields, nested `Option`s, literals,
//! ranges, and `|` (`if let` on `Option` only, as on the crate's enums it
//! is refused), bind names that may hide a parameter, and are guarded by
//! the expressions `generated_equivalence.rs` draws, closures that hide
//! names included. The bodies build the next state or fail, as
//! `fn step(state, event) -> Result<State, Error>` does. Beside them, from
//! a generator of its own, a `match *xs` whose arms are slice patterns of
//! the integer patterns (`[a, .., 0..=9]`), `if let [p, rest @ ..]`, and
//! `matches!` on a slice.
//!
//! And from a third, generic functions over `T: Ord`, `T: PartialOrd`,
//! `T: Display`, and `T: Default`, called at `i32`, `bool`, a tuple, and
//! the crate's types, which pass their dictionaries (design/01 §7.25).
//!
//! The audit of 0.9.1 found a guard reading a name its closure hid, in a
//! struct pattern no fixture spelled; this looks for that class.
//!
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.

use std::fmt::Write as _;

use crate::generated_equivalence::{Gen, Ty};
use crate::support::generated::{compare_rows, fn_count, seeds, Args, SEEDS};
use crate::support::{Js, Rng};

/// The crate's types, the same in every seed.
const TYPES: &str = "#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Acc {
    pub total: i32,
    pub last: Option<i32>,
}

impl std::fmt::Display for Acc {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, \"acc {}\", self.total)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum State {
    #[default]
    Idle,
    Count(i32),
    Pair(i32, bool),
    Rec { n: i32, open: bool },
    Held(Acc),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    Tick,
    Add(i32),
    Set(Option<i32>),
    Close { force: bool },
}
";

/// The generic functions `generic_function` calls, the same in every seed:
/// each orders, prints, or makes a `T` through its bound's dictionary.
const GENERICS: &str = "
fn g_max<T: Ord + Copy>(a: T, b: T) -> T {
    if a < b { b } else { a }
}

fn g_cmp<T: Ord>(a: &T, b: &T) -> std::cmp::Ordering {
    a.cmp(b)
}

fn g_top<T: Ord + Clone>(xs: &[T]) -> Option<T> {
    xs.iter().max().cloned()
}

fn g_rank<T>(a: T, b: T, c: T) -> i32 where T: PartialOrd {
    if a < b { 1 } else if a <= c { 2 } else { 3 }
}

fn g_show<T: std::fmt::Display>(x: T) -> String {
    format!(\"<{}>\", x)
}

fn g_zero<T: Default>() -> T {
    T::default()
}
";

/// `Show` for the crate's types, as `purecrate_canon` prints them.
const SHOW_TYPES: &str = r#"
impl Show for Acc {
    fn show(&self) -> String { format!("Acc {{ total: {}, last: {} }}", self.total.show(), self.last.show()) }
}
impl Show for State {
    fn show(&self) -> String {
        match self {
            State::Idle => "State::Idle".into(),
            State::Count(x) => format!("State::Count({})", x.show()),
            State::Pair(x, y) => format!("State::Pair({}, {})", x.show(), y.show()),
            State::Rec { n, open } => format!("State::Rec {{ n: {}, open: {} }}", n.show(), open.show()),
            State::Held(a) => format!("State::Held({})", a.show()),
        }
    }
}
"#;

const PARAMS: &str = "s: State, e: Event, a: i32, b: i32, o: Option<i32>, r: Result<i32, i32>, c: bool, xs: &[i32]";

/// What a function returns.
#[derive(Clone, Copy, PartialEq)]
enum Ret {
    Step,
    Int,
}

struct Pats {
    g: Gen,
    /// Names the pattern being drawn binds, kept apart.
    bound: Vec<String>,
}

impl Pats {
    /// A name for a pattern to bind, of type `ty`, put in scope.
    fn bind(&mut self, ty: Ty) -> String {
        let mut v = self.g.fresh();
        while self.bound.contains(&v) {
            v = self.g.fresh();
        }
        self.bound.push(v.clone());
        self.g.scope.push((v.clone(), ty, false));
        v
    }

    fn small(&mut self) -> &'static str {
        self.g.rng.pick(&["0", "7", "-1", "0..=9", "1..=100", "-5..=-1"])
    }

    fn int_pat(&mut self) -> String {
        match self.g.rng.below(4) {
            0 => "_".into(),
            1 => self.small().into(),
            _ => self.bind(Ty::Int),
        }
    }

    fn bool_pat(&mut self) -> String {
        match self.g.rng.below(4) {
            0 => "_".into(),
            1 => "true".into(),
            2 => "false".into(),
            _ => self.bind(Ty::Bool),
        }
    }

    /// `field`: inside a struct pattern, where a field is `_`, a name, or a
    /// pattern that binds nothing (`[pattern/other]`).
    fn opt_pat(&mut self, field: bool) -> String {
        match self.g.rng.below(5) {
            0 => "_".into(),
            1 => "None".into(),
            2 if field => format!("Some({})", self.small()),
            2 => format!("Some({})", self.int_pat()),
            _ => self.bind(Ty::Opt),
        }
    }

    /// A slice pattern over `*xs`, whose elements bind `i32`s.
    fn slice_pat(&mut self) -> String {
        match self.g.rng.below(8) {
            0 => "[]".into(),
            1 => format!("[{}]", self.int_pat()),
            2 => {
                let (x, y) = (self.int_pat(), self.int_pat());
                format!("[{x}, {y}]")
            }
            3 => format!("[{}, ..]", self.int_pat()),
            4 => format!("[.., {}]", self.int_pat()),
            5 => {
                let (x, y) = (self.int_pat(), self.int_pat());
                format!("[{x}, .., {y}]")
            }
            6 => {
                let (x, y) = (self.int_pat(), self.int_pat());
                format!("[{x}, {y}, _, ..]")
            }
            _ => "[] | [_]".into(),
        }
    }

    fn state_pat(&mut self) -> String {
        match self.g.rng.below(8) {
            0 => "State::Idle".into(),
            1 => format!("State::Count({})", self.int_pat()),
            2 => format!("State::Pair({}, {})", self.int_pat(), self.bool_pat()),
            3 => {
                let (n, open) = (self.int_pat(), self.bool_pat());
                match (n.as_str(), open.as_str()) {
                    ("_", "_") => "State::Rec { .. }".into(),
                    ("_", _) => format!("State::Rec {{ open: {open}, .. }}"),
                    (_, "_") => format!("State::Rec {{ n: {n}, .. }}"),
                    _ => format!("State::Rec {{ n: {n}, open: {open} }}"),
                }
            }
            4 => format!("State::Held(Acc {{ total: {}, last: {} }})", self.int_pat(), self.opt_pat(true)),
            5 => format!("State::Held(Acc {{ last: {}, .. }})", self.opt_pat(true)),
            6 => "State::Idle | State::Count(_)".into(),
            _ => "_".into(),
        }
    }

    fn event_pat(&mut self) -> String {
        match self.g.rng.below(7) {
            0 => "Event::Tick".into(),
            1 => format!("Event::Add({})", self.int_pat()),
            2 => format!("Event::Set({})", self.opt_pat(false)),
            3 => format!("Event::Close {{ force: {} }}", self.bool_pat()),
            4 => "Event::Tick | Event::Add(_)".into(),
            _ => "_".into(),
        }
    }

    /// A state built from what is in scope.
    fn state(&mut self, d: u32) -> String {
        match self.g.rng.below(7) {
            0 => "s".into(),
            1 => "State::Idle".into(),
            2 => format!("State::Count({})", self.g.int(d)),
            3 => format!("State::Pair({}, {})", self.g.int(d), self.g.boolean(d)),
            4 => format!("State::Rec {{ n: {}, open: {} }}", self.g.int(d), self.g.boolean(d)),
            _ => {
                let opt = self.g.place(Ty::Opt, d);
                format!("State::Held(Acc {{ total: {}, last: {opt} }})", self.g.int(d))
            }
        }
    }

    /// An arm's value: the next state or a failure, or an integer.
    fn body(&mut self, ret: Ret) -> String {
        match ret {
            Ret::Int => self.g.int(2),
            Ret::Step => match self.g.rng.below(5) {
                0 => format!("Err({})", self.g.int(2)),
                1 => {
                    let test = self.g.without_try(|g| g.boolean(2));
                    let (then, else_) = self.g.without_try(|g| (g.int(1), g.int(1)));
                    let st = self.g.without_try(|g| g.int(1));
                    format!("if {test} {{ Ok(State::Count({then} + {st})) }} else {{ Err({else_}) }}")
                }
                _ => {
                    let st = self.state(2);
                    format!("Ok({st})")
                }
            },
        }
    }

    /// The arms of a `match`, each pattern drawn by `pat`, and a last `_`.
    /// With `once`, a variant is tested by one arm only: `check` refuses a
    /// second (`` `Event::Tick` is matched more than once ``).
    fn arms(&mut self, ret: Ret, once: bool, pat: impl Fn(&mut Self) -> String) -> String {
        let mut out = String::new();
        let mut used: Vec<String> = Vec::new();
        let n = 1 + self.g.rng.below(4);
        let mut drawn = 0;
        for _ in 0..n + 8 {
            if drawn == n {
                break;
            }
            let scope = self.g.scope.len();
            self.bound.clear();
            let p = pat(self);
            let variants: Vec<String> = p
                .split(|c: char| !(c.is_alphanumeric() || c == ':'))
                .filter(|w| w.starts_with("State::") || w.starts_with("Event::"))
                .map(str::to_string)
                .collect();
            // A bare `_` arm would leave the ones after it unreachable.
            if p == "_" || p == "(_, _)" || (once && variants.iter().any(|v| used.contains(v))) {
                self.g.scope.truncate(scope);
                continue;
            }
            used.extend(variants);
            drawn += 1;
            let guard = if self.g.rng.below(3) == 0 {
                format!(" if {}", self.g.without_try(|g| g.boolean(2)))
            } else {
                String::new()
            };
            let body = self.body(ret);
            writeln!(out, "        {p}{guard} => {body},").expect("write");
            self.g.scope.truncate(scope);
        }
        let last = self.body(ret);
        writeln!(out, "        _ => {last},").expect("write");
        out
    }

    /// A `match` on `xs`, or a test of it. Drawn from a generator of its
    /// own, so the functions above stay as each seed drew them before.
    fn slice_function(&mut self, ret: Ret) -> String {
        match self.g.rng.below(3) {
            // `*xs`: the elements bind values, as the other patterns do.
            0 | 1 => format!("    match *xs {{\n{}    }}", self.arms(ret, false, |p| p.slice_pat())),
            // What `rest @ ..` binds, and `matches!` on a slice.
            _ => {
                let head = match self.g.rng.below(3) {
                    0 => "_".into(),
                    _ => self.small().to_string(),
                };
                let n = self.g.int(1);
                let then = match ret {
                    Ret::Int => format!("rest.len() as i32 + {n}"),
                    Ret::Step => format!("Ok(State::Count(rest.len() as i32 + {n}))"),
                };
                let else_ = self.body(ret);
                let test = if self.g.rng.below(2) == 0 {
                    format!("let [{head}, rest @ ..] = xs")
                } else {
                    format!("let [rest @ .., {head}] = xs")
                };
                // `matches!` binds names only for its guard; none here.
                let scope = self.g.scope.len();
                self.bound.clear();
                let other = self.slice_pat();
                let other = if self.bound.is_empty() { other } else { "[_, _, ..]".into() };
                self.g.scope.truncate(scope);
                format!(
                    "    if matches!(*xs, {other}) {{\n        {else_}\n    }} else if {test} {{\n        {then}\n    }} else {{\n        {}\n    }}",
                    self.body(ret)
                )
            }
        }
    }

    /// An event built from what is in scope.
    fn event(&mut self, d: u32) -> String {
        match self.g.rng.below(5) {
            0 => "e".into(),
            1 => "Event::Tick".into(),
            2 => format!("Event::Add({})", self.g.int(d)),
            3 => format!("Event::Set({})", self.g.place(Ty::Opt, d)),
            _ => format!("Event::Close {{ force: {} }}", self.g.boolean(d)),
        }
    }

    /// A function that orders the crate's types by their derived `Ord`
    /// (design/01 §7.25): `<` .. `>=` on states, and on tuples and `Option`s
    /// of events, `cmp` in a `match`, `max`, `min`, `clamp` (which panics
    /// where its bounds cross), `sort`, and the iterator's `max` / `min`.
    /// Drawn from a generator of its own, so the functions above stay as
    /// each seed drew them before.
    fn order_function(&mut self, ret: Ret) -> String {
        let op = self.g.rng.pick(&["<", "<=", ">", ">="]);
        let int_or = |p: &mut Self, value: String| match ret {
            Ret::Step => format!("Ok({value})"),
            Ret::Int => {
                let (x, y) = (p.g.int(1), p.g.int(1));
                format!("if {value} == s {{ {x} }} else {{ {y} }}")
            }
        };
        match self.g.rng.below(8) {
            0 => {
                let t = self.state(1);
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if s {op} ({t}) {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
            1 => {
                let (ev, n) = (self.event(1), self.g.int(1));
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if (e, a) {op} ({ev}, {n}) {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
            2 => {
                let ev = self.event(1);
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if Some(e) {op} (if c {{ Some({ev}) }} else {{ None }}) {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
            3 => {
                let t = self.state(1);
                let (x, y, z) = (self.body(ret), self.body(ret), self.body(ret));
                format!(
                    "    match s.cmp(&({t})) {{\n        std::cmp::Ordering::Less => {x},\n        std::cmp::Ordering::Equal => {y},\n        std::cmp::Ordering::Greater => {z},\n    }}"
                )
            }
            4 => {
                let (t, u) = (self.state(1), self.state(1));
                let picked = match self.g.rng.below(3) {
                    0 => format!("s.max({t})"),
                    1 => format!("s.min({t})"),
                    _ => format!("s.clamp({t}, {u})"),
                };
                format!("    let m = {picked};\n    {}", int_or(self, "m".into()))
            }
            5 => {
                let (t, u) = (self.state(1), self.state(1));
                let at = self.g.rng.below(3);
                format!("    let mut v = vec![s, {t}, {u}];\n    v.sort();\n    {}", int_or(self, format!("v[{at}]")))
            }
            6 => {
                let (t, u) = (self.state(1), self.state(1));
                let which = self.g.rng.pick(&["max", "min"]);
                format!(
                    "    let v = vec![{t}, s, {u}];\n    let m = v.into_iter().{which}().unwrap_or(State::Idle);\n    {}",
                    int_or(self, "m".into())
                )
            }
            _ => {
                let ev = self.event(1);
                let which = self.g.rng.pick(&["max", "min"]);
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if e.{which}({ev}) == e {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
        }
    }

    /// A function that calls the generic functions of `GENERICS` at a
    /// primitive or a crate type, each passing its dictionary. Drawn from a
    /// generator of its own, as `order_function` is.
    fn generic_function(&mut self, ret: Ret) -> String {
        let op = self.g.rng.pick(&["<", "<=", ">", ">="]);
        let int_or = |p: &mut Self, value: String| match ret {
            Ret::Step => format!("Ok({value})"),
            Ret::Int => {
                let (x, y) = (p.g.int(1), p.g.int(1));
                format!("if {value} == s {{ {x} }} else {{ {y} }}")
            }
        };
        match self.g.rng.below(8) {
            0 => {
                let (n, m) = (self.g.int(1), self.g.int(1));
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if g_max(a, {n}) {op} {m} {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
            1 => {
                let t = self.state(1);
                format!("    let m = g_max(s, {t});\n    {}", int_or(self, "m".into()))
            }
            2 => {
                let ev = self.event(1);
                let (x, y, z) = (self.body(ret), self.body(ret), self.body(ret));
                format!(
                    "    match g_cmp(&e, &({ev})) {{\n        std::cmp::Ordering::Less => {x},\n        std::cmp::Ordering::Equal => {y},\n        std::cmp::Ordering::Greater => {z},\n    }}"
                )
            }
            3 => {
                let n = self.g.int(1);
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    let m = g_top(xs).unwrap_or({n});\n    if m {op} a {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
            4 => {
                let shown = match self.g.rng.below(3) {
                    0 => format!("g_show({})", self.g.int(1)),
                    1 => format!("g_show({})", self.g.boolean(1)),
                    _ => format!("g_show(Acc {{ total: {}, last: None }})", self.g.int(1)),
                };
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if {shown} {op} g_show(a) {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
            5 => {
                let (x, y) = (self.body(ret), self.body(ret));
                format!(
                    "    let z: i32 = g_zero();\n    let t: State = g_zero();\n    if t == s || z {op} a {{\n        {x}\n    }} else {{\n        {y}\n    }}"
                )
            }
            6 => {
                let (n, k) = (self.g.int(1), self.g.boolean(1));
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if g_max((a, c), ({n}, {k})) == (a, c) {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
            _ => {
                let (ev, fv) = (self.event(1), self.event(1));
                let r = self.g.rng.pick(&["1", "2", "3"]);
                let (x, y) = (self.body(ret), self.body(ret));
                format!("    if g_rank(e, {ev}, {fv}) == {r} {{\n        {x}\n    }} else {{\n        {y}\n    }}")
            }
        }
    }

    fn function(&mut self, ret: Ret) -> String {
        match self.g.rng.below(6) {
            0 => format!("    match s {{\n{}    }}", self.arms(ret, true, |p| p.state_pat())),
            1 => format!("    match e {{\n{}    }}", self.arms(ret, true, |p| p.event_pat())),
            2 => {
                let scope = self.g.scope.len();
                self.bound.clear();
                // `if let` takes `Option` and `Result` only
                // (`[pattern/if-let-variant]` on the crate's enums).
                let p = format!("Some({})", self.int_pat());
                let then = self.body(ret);
                self.g.scope.truncate(scope);
                let else_ = self.body(ret);
                format!("    if let {p} = o {{\n        {then}\n    }} else {{\n        {else_}\n    }}")
            }
            3 if ret == Ret::Int => {
                self.bound.clear();
                let scope = self.g.scope.len();
                let p = self.event_pat();
                self.g.scope.truncate(scope);
                // `matches!` binds names only for its guard; none here.
                // `matches!(e, _)` tests nothing (`[pattern/arm]`).
                let p = if self.bound.is_empty() && p != "_" { p } else { "Event::Tick".into() };
                let (x, y) = (self.g.int(2), self.g.int(2));
                format!("    if matches!(e, {p}) {{ {x} }} else {{ {y} }}")
            }
            _ => format!(
                "    match (s, e) {{\n{}    }}",
                self.arms(ret, false, |p| {
                    let (st, ev) = (p.state_pat(), p.event_pat());
                    format!("({st}, {ev})")
                })
            ),
        }
    }
}

/// A state and an event to call with: the Rust literal, and the TS one.
fn state_value(rng: &mut Rng) -> (String, String) {
    let int = |rng: &mut Rng| rng.edgy(32, true) as i32;
    match rng.below(5) {
        0 => ("State::Idle".into(), "({ kind: \"Idle\" })".into()),
        1 => {
            let x = int(rng);
            (format!("State::Count({x})"), format!("({{ kind: \"Count\", value: {} }})", x.js()))
        }
        2 => {
            let (x, y) = (int(rng), rng.below(2) == 0);
            (format!("State::Pair({x}, {y})"), format!("({{ kind: \"Pair\", content: [{}, {y}] }})", x.js()))
        }
        3 => {
            let (n, open) = (rng.pick(&[0, 3, 7, 50, -2]), rng.below(2) == 0);
            (
                format!("State::Rec {{ n: {n}, open: {open} }}"),
                format!("({{ kind: \"Rec\", n: {}, open: {open} }})", n.js()),
            )
        }
        _ => {
            let total = rng.pick(&[0, 1, -1, 100, i32::MAX]);
            let last = match rng.below(3) {
                0 => None,
                _ => Some(rng.pick(&[0, 2, 9, -4, i32::MIN])),
            };
            (
                format!("State::Held(Acc {{ total: {total}, last: {last:?} }})"),
                format!("({{ kind: \"Held\", value: {{ total: {}, last: {} }} }})", total.js(), last.js()),
            )
        }
    }
}

fn event_value(rng: &mut Rng) -> (String, String) {
    match rng.below(4) {
        0 => ("Event::Tick".into(), "({ kind: \"Tick\" })".into()),
        1 => {
            let x = rng.pick(&[0, 1, 5, 9, 100, -3, i32::MAX]);
            (format!("Event::Add({x})"), format!("({{ kind: \"Add\", value: {} }})", x.js()))
        }
        2 => {
            let x = match rng.below(3) {
                0 => None,
                _ => Some(rng.pick(&[0, 2, 3, 4, -1])),
            };
            (format!("Event::Set({x:?})"), format!("({{ kind: \"Set\", value: {} }})", x.js()))
        }
        _ => {
            let force = rng.below(2) == 0;
            (format!("Event::Close {{ force: {force} }}"), format!("({{ kind: \"Close\", force: {force} }})"))
        }
    }
}

#[test]
fn generated_patterns_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let mut source = String::from(TYPES);
    source.push_str(GENERICS);
    let mut fns = Vec::new();
    for &seed in &seeds {
        let mut p = Pats { g: Gen::new(seed ^ 0x9a77), bound: Vec::new() };
        for i in 0..count {
            let ret = if p.g.rng.below(4) == 0 { Ret::Int } else { Ret::Step };
            // `?` on a `Result<i32, i32>` leaves a `Result<State, i32>`.
            p.g.ret = if ret == Ret::Step { Ty::Res } else { Ty::Int };
            p.g.next_name = 0;
            p.g.scope = Gen::params();
            let body = p.function(ret);
            let name = format!("p{seed}_f{i}");
            let rt = if ret == Ret::Step { "Result<State, i32>" } else { "i32" };
            writeln!(source, "\npub fn {name}({PARAMS}) -> {rt} {{\n{body}\n}}").expect("write");
            fns.push(name);
        }
        let mut p = Pats { g: Gen::new(seed ^ 0x511ce), bound: Vec::new() };
        for i in 0..count.div_ceil(4) {
            let ret = if p.g.rng.below(2) == 0 { Ret::Int } else { Ret::Step };
            p.g.ret = if ret == Ret::Step { Ty::Res } else { Ty::Int };
            p.g.next_name = 0;
            p.g.scope = Gen::params();
            let body = p.slice_function(ret);
            let name = format!("p{seed}_s{i}");
            let rt = if ret == Ret::Step { "Result<State, i32>" } else { "i32" };
            writeln!(source, "\npub fn {name}({PARAMS}) -> {rt} {{\n{body}\n}}").expect("write");
            fns.push(name);
        }
        let mut p = Pats { g: Gen::new(seed ^ 0x0bde), bound: Vec::new() };
        for i in 0..count.div_ceil(4) {
            let ret = if p.g.rng.below(2) == 0 { Ret::Int } else { Ret::Step };
            p.g.ret = if ret == Ret::Step { Ty::Res } else { Ty::Int };
            p.g.next_name = 0;
            p.g.scope = Gen::params();
            let body = p.order_function(ret);
            let name = format!("p{seed}_o{i}");
            let rt = if ret == Ret::Step { "Result<State, i32>" } else { "i32" };
            writeln!(source, "\npub fn {name}({PARAMS}) -> {rt} {{\n{body}\n}}").expect("write");
            fns.push(name);
        }
        let mut p = Pats { g: Gen::new(seed ^ 0x9e7e), bound: Vec::new() };
        for i in 0..count.div_ceil(4) {
            let ret = if p.g.rng.below(2) == 0 { Ret::Int } else { Ret::Step };
            p.g.ret = if ret == Ret::Step { Ty::Res } else { Ty::Int };
            p.g.next_name = 0;
            p.g.scope = Gen::params();
            let body = p.generic_function(ret);
            let name = format!("p{seed}_g{i}");
            let rt = if ret == Ret::Step { "Result<State, i32>" } else { "i32" };
            writeln!(source, "\npub fn {name}({PARAMS}) -> {rt} {{\n{body}\n}}").expect("write");
            fns.push(name);
        }
    }
    let mut rng = Rng::new(seeds[0] ^ 0x5eed);
    let mut lists = Rng::new(seeds[0] ^ 0x511ce);
    let rows: Vec<_> = (0..12)
        .map(|_| {
            let (s, e) = (state_value(&mut rng), event_value(&mut rng));
            let int =
                |rng: &mut Rng| if rng.below(2) == 0 { rng.pick(&[-1, 0, 1, 3]) } else { rng.edgy(32, true) as i32 };
            let (a, b) = (int(&mut rng), int(&mut rng));
            let o = if rng.below(3) == 0 { None } else { Some(int(&mut rng)) };
            let r: Result<i32, i32> = if rng.below(3) == 0 { Err(int(&mut rng)) } else { Ok(int(&mut rng)) };
            let c = rng.below(2) == 0;
            // Of a generator of its own, so the other arguments stay as each
            // seed drew them before.
            let xs: Vec<i32> = (0..lists.below(5)).map(|_| int(&mut lists)).collect();
            (s, e, a, b, o, r, c, xs)
        })
        .collect();

    let rows: Vec<Args> = rows
        .iter()
        .map(|(s, e, a, b, o, r, c, xs)| Args {
            rust: format!("{}, {}, {a}, {b}, {o:?}, {r:?}, {c}, &{xs:?}", s.0, e.0),
            js: [s.1.clone(), e.1.clone(), a.js(), b.js(), o.js(), r.js(), c.js(), xs.js()].join(", "),
        })
        .collect();
    compare_rows("generated_patterns", &seeds, &source, SHOW_TYPES, &fns, &rows);
}
