//! Transitions generated from a seed, over the crate's own types: a `match`
//! on `(state, event)` (or on one of them, `if let`, or `matches!`) whose
//! arms test variants, tuple and struct fields, nested `Option`s, literals,
//! ranges, and `|` (`if let` on `Option` only, as on the crate's enums it
//! is refused), bind names that may hide a parameter, and are guarded by
//! the expressions `generated_equivalence.rs` draws, closures that hide
//! names included. The bodies build the next state or fail, as
//! `fn step(state, event) -> Result<State, Error>` does.
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
const TYPES: &str = "#[derive(Clone, Copy)]
pub struct Acc {
    pub total: i32,
    pub last: Option<i32>,
}

#[derive(Clone, Copy)]
pub enum State {
    Idle,
    Count(i32),
    Pair(i32, bool),
    Rec { n: i32, open: bool },
    Held(Acc),
}

#[derive(Clone, Copy)]
pub enum Event {
    Tick,
    Add(i32),
    Set(Option<i32>),
    Close { force: bool },
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

const PARAMS: &str = "s: State, e: Event, a: i32, b: i32, o: Option<i32>, r: Result<i32, i32>, c: bool";

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
    }
    let mut rng = Rng::new(seeds[0] ^ 0x5eed);
    let rows: Vec<_> = (0..12)
        .map(|_| {
            let (s, e) = (state_value(&mut rng), event_value(&mut rng));
            let int =
                |rng: &mut Rng| if rng.below(2) == 0 { rng.pick(&[-1, 0, 1, 3]) } else { rng.edgy(32, true) as i32 };
            let (a, b) = (int(&mut rng), int(&mut rng));
            let o = if rng.below(3) == 0 { None } else { Some(int(&mut rng)) };
            let r: Result<i32, i32> = if rng.below(3) == 0 { Err(int(&mut rng)) } else { Ok(int(&mut rng)) };
            (s, e, a, b, o, r, rng.below(2) == 0)
        })
        .collect();

    let rows: Vec<Args> = rows
        .iter()
        .map(|(s, e, a, b, o, r, c)| Args {
            rust: format!("{}, {}, {a}, {b}, {o:?}, {r:?}, {c}", s.0, e.0),
            js: [s.1.clone(), e.1.clone(), a.js(), b.js(), o.js(), r.js(), c.js()].join(", "),
        })
        .collect();
    compare_rows("generated_patterns", &seeds, &source, SHOW_TYPES, &fns, &rows);
}
