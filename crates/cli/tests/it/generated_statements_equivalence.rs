//! Function bodies generated from a seed: `let` and `let mut`, assignment
//! and `op=`, `if` and `match` as statements, range `for` and `while`,
//! `break`, `continue`, early `return`, and `?` wherever a statement holds
//! an expression (a range's ends, a `while` test, an `if` test). The
//! expressions inside are `generated_equivalence.rs`'s; this test adds the
//! statements around them, where the audit of 0.8.1 found a `?` in a
//! range's end that ran before its start.
//!
//! From 0.21.0 a range `for` is also stepped by `step_by` with `rev` on
//! either side, and an accumulator `let mut s = 0;` that its first use
//! types sums one, mixed into `h0`. These draw from a second generator, so
//! each seed's other statements are the ones it drew before.
//!
//! From 0.22.0 each seed also has an error enum with two `impl From` and a
//! function chaining `?` over both sources (`conversions`), its own draws.
//!
//! From 0.22.0 each seed also declares a trait with a default method, a
//! struct and an enum implementing it (the enum overriding the default in
//! some seeds), and a generic function over it (`traits`), which a
//! statement calls at both types, mixed into `h0`. Its draws are a third
//! generator's, for the same reason.
//!
//! Every loop is bounded: a range's ends are taken `% 4` and `% 6`, and a
//! `while` counts a local nothing else writes up to a bound under 5, first
//! in its body, so `continue` cannot skip it. Overflow, division by zero,
//! and the rest panic as in Rust and are compared as panics.
//!
//! Scoping and names: bare blocks `{ .. }` with their own `let` and
//! `let mut`, nested and side by side; `let` rebinding a name in scope from
//! its own value, as another type too (`let x: bool = x < 3;`), in blocks,
//! `if` / `else` sides, `match` arms, loop bodies, and closures (whose
//! parameter is bound again in their block); a `let mut` from outside
//! written in a block that then binds the same name, read after it; blocks,
//! `if`, and `match` as a `let`'s value with statements inside (`?`,
//! `return`, `break`, and `continue` among them); and names the translator
//! makes up (`value`, `opt`, `end`, `x_option`, ..) or JS globals rebind
//! (`Math`, `String`, ..). Every function folds what its names hold
//! at points into `h0`, which it returns mixed in, so a binding that leaks
//! past its block changes the result.
//!
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.
//!
//! What it found: one hole (`match o.ok_or(x.ok_or(e)?)` left only an
//! inline function, and the `Err` arm ran), a temporary declared twice,
//! and output tsc refused (`order_of_eval.rs`, `narrowing.rs`). Seeds 1 to
//! 120 agree on every value and type-check (60 did before narrowing
//! followed TS's control flow, CHANGELOG 0.10.2 and 0.10.3). Drawing blocks,
//! shadowing, and made-up names, it found a block's binding that outlived
//! the block or was declared twice, a closure dropped where its block
//! folded, and narrowing TS keeps in a closure or through a `bool` that
//! decides only when it fails (`review_holes3.rs`).

use std::fmt::Write as _;

use crate::generated_equivalence::{compare, Gen, Ty, PARAMS};
use crate::support::generated::{fn_count, seeds, SEEDS};
use crate::support::Rng;

struct Body {
    g: Gen,
    /// The seed, which names the module of associated consts and
    /// functions `paths` reads (`modules`).
    seed: u64,
    /// `paths`'s own draws, apart from `g`'s, so the statements every
    /// other seed drew before it was added stay as they were.
    paths_rng: crate::support::Rng,
    /// Loops around the current statement, for `break` and `continue`,
    /// innermost last: whether each is a `loop` whose `break` carries an
    /// `i32`.
    loops: Vec<bool>,
    /// Lines written so far, indented.
    out: String,
    indent: usize,
    /// What draws the 0.21.0 shapes, apart from `g`'s generator.
    extra: Rng,
    /// What draws the 0.22.0 trait calls, apart from the others.
    traits_rng: Rng,
    /// What draws the 0.24.0 literals a later use types, apart from the
    /// others.
    lits_rng: Rng,
}

/// Names a binding may take besides `v<n>`: ones the translator makes up
/// for its temporaries (and what the renamer turns them and a shadowed
/// name into), and JS globals a strict-mode module may rebind. `check`
/// refuses TS reserved words, the runtime's names (`Int`, `Str`, ..),
/// `Array`, `NaN`, and `Infinity`, so those are left out; `c`, `o`, and `r` keep their type, as the
/// expression generator falls back to them.
const NAMES: &[&str] = &[
    "value", "value2", "option", "opt", "result", "res", "copy", "end", "i2", "x", "y", "t1", "x_option", "a_or", "a2",
    "matched", "arg", "remove", "ord", "raw", "name", "Math", "Number", "String", "Object", "BigInt", "Symbol", "JSON",
    "Error", "Boolean", "Function", "Date", "console", "Reflect",
];

/// Names a binding of another type than the parameter's may not take.
const KEEP_TYPE: &[&str] = &["c", "o", "r"];

impl Body {
    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// `{ .. }` around `count` statements; names bound inside go out of
    /// scope after it.
    fn block(&mut self, head: &str, depth: u32, count: u64) {
        self.block_from(head, None, depth, count);
    }

    /// A block whose first line is `first`.
    fn block_from(&mut self, head: &str, first: Option<&str>, depth: u32, count: u64) {
        if head.is_empty() {
            self.line("{");
        } else {
            self.line(&format!("{head} {{"));
        }
        let scope = self.g.scope.len();
        self.indent += 1;
        if let Some(first) = first {
            self.line(first);
        }
        let n = 1 + self.g.rng.below(count);
        for _ in 0..n {
            self.stmt(depth);
        }
        self.indent -= 1;
        self.g.scope.truncate(scope);
        self.line("}");
    }

    /// A value for an early `return`, or a function's tail.
    fn ret_value(&mut self, depth: u32) -> String {
        let ret = self.g.ret;
        match ret {
            Ty::Opt if self.g.rng.below(2) == 0 => format!("Some({})", self.g.int(depth)),
            Ty::Res if self.g.rng.below(2) == 0 => format!("Ok({})", self.g.int(depth)),
            _ => self.g.expr(ret, depth),
        }
    }

    /// A statement, and now and then an accumulator after it.
    fn stmt(&mut self, depth: u32) {
        if self.paths_rng.below(10) == 0 {
            self.paths();
        }
        self.drawn(depth);
        if self.extra.below(10) == 0 {
            self.accumulate();
        }
        if self.traits_rng.below(8) == 0 {
            self.trait_call();
        }
        if self.lits_rng.below(8) == 0 {
            self.literals();
        }
    }

    /// A `let` of a `match` or an `if` whose results are unsuffixed
    /// literals, or an array of them a `for` walks, each typed by its first
    /// use as rustc types it: an `i32` mixed into `h0`, or a `u8` added to
    /// one. Its tests draw from `lits_rng`, as `g`'s generator in turn.
    fn literals(&mut self) {
        let names = self.g.next_name;
        std::mem::swap(&mut self.g.rng, &mut self.lits_rng);
        let (k, test) = (self.g.int(2), self.g.boolean(2));
        std::mem::swap(&mut self.g.rng, &mut self.lits_rng);
        self.g.next_name = names;
        let r = &mut self.lits_rng;
        let n = r.below(1 << 20);
        let (v, d) = (format!("w{n}x"), format!("d{n}x"));
        let u8_side = r.below(2) == 0;
        let mut lit = || match u8_side {
            true => r.pick(&[0i64, 1, 7, 128, 255]).to_string(),
            false => r.pick(&[0i64, 1, -1, 9, 2_147_483_647, -2_147_483_648]).to_string(),
        };
        let (a, b, c) = (lit(), lit(), lit());
        let value = match r.below(3) {
            0 => format!("match ({k}) % 3 {{ 0 => {a}, 1 | -1 => {b}, _ => {c} }}"),
            1 => format!("if {test} {{ {a} }} else {{ {b} }}"),
            _ => String::new(),
        };
        let mix = |x: &str| match u8_side {
            true => format!("h0 = h0.wrapping_mul(31).wrapping_add(i32::from(((h0 & 255) as u8).wrapping_add({x})));"),
            false => format!("h0 = h0.wrapping_mul(31).wrapping_add({x});"),
        };
        if value.is_empty() {
            self.line(&format!("for {d} in [{a}, {b}, {c}] {{"));
            self.line(&format!("    {}", mix(&d)));
            self.line("}");
        } else {
            self.line(&format!("let {v} = {value};"));
            self.line(&mix(&v));
        }
    }

    /// `traits`'s generic function called at its struct and at its enum,
    /// on `i32`s in scope or literals, mixed into `h0`.
    fn trait_call(&mut self) {
        let s = self.seed;
        let scope = &self.g.scope;
        let ints: Vec<String> = (scope.iter().enumerate())
            .filter(|(i, (n, t, _))| *t == Ty::Int && !scope[i + 1..].iter().any(|(m, _, _)| m == n))
            .map(|(_, (v, _, _))| v.clone())
            .collect();
        let r = &mut self.traits_rng;
        let mut int = || match r.below(ints.len() as u64 + 2) as usize {
            i if i < ints.len() => ints[i].clone(),
            i if i == ints.len() => {
                format!("{}i32", r.pick(&[0i64, 1, -1, 7, 2_147_483_647, -2_147_483_648]))
            }
            _ => format!("{}i32", r.below(201) as i64 - 100),
        };
        let (a, b, c, d) = (int(), int(), int(), int());
        self.line(&format!(
            "h0 = h0.wrapping_mul(31).wrapping_add(tsum{s}(&vec![Ta{s} {{ n: {a} }}, Ta{s} {{ n: {b} }}], {c}) ^ tsum{s}(&vec![Tb{s}::P({d}), Tb{s}::Q], {c}));"
        ));
    }

    /// `let mut s = <unsuffixed>;` summing a stepped range, the start typed
    /// by `s += i` as rustc types it, then mixed into `h0`. Its ends draw
    /// from `extra`, as `g`'s generator in turn.
    fn accumulate(&mut self) {
        let names = self.g.next_name;
        std::mem::swap(&mut self.g.rng, &mut self.extra);
        let (lo, hi) = (self.g.int(2), self.g.int(2));
        std::mem::swap(&mut self.g.rng, &mut self.extra);
        self.g.next_name = names;
        let n = self.extra.below(1 << 20);
        let (v, i) = (format!("s{n}x"), format!("n{n}x"));
        let start = self.extra.pick(&["0", "1", "-1", "7"]);
        let range = self.stepped(&lo, &hi);
        self.line(&format!("let mut {v} = {start};"));
        self.line(&format!("for {i} in {range} {{"));
        self.line(&format!("    {v} += {i};"));
        self.line("}");
        self.line(&format!("h0 = h0.wrapping_mul(31).wrapping_add({v});"));
    }

    /// A range of `lo % 4` to `hi % 6` stepped by `step_by` (by 0, which
    /// panics, now and then), with `rev` before it, after it, or both.
    fn stepped(&mut self, lo: &str, hi: &str) -> String {
        let k = self.extra.pick(&["1", "2", "3", "2", "3", "0"]);
        match self.extra.below(5) {
            0 => format!("(({lo}) % 4..({hi}) % 6).step_by({k})"),
            1 => format!("(({lo}) % 4..=({hi}) % 6).step_by({k})"),
            2 => format!("(({lo}) % 4..=({hi}) % 6).rev().step_by({k})"),
            3 => format!("(({lo}) % 4..({hi}) % 6).step_by({k}).rev()"),
            _ => format!("(({lo}) % 4..({hi}) % 6).rev().step_by({k}).rev()"),
        }
    }

    fn drawn(&mut self, depth: u32) {
        let e = 2;
        let pick = if depth == 0 { self.g.rng.pick(&[0, 1, 2, 3, 13, 13, 14]) } else { self.g.rng.below(25) };
        match pick {
            0 => {
                let ty = self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Opt, Ty::Bool]);
                let value = self.g.expr(ty, e);
                let v = self.name(ty);
                self.line(&format!("let mut {v}: {} = {value};", ty.rust()));
                self.g.scope.push((v, ty, true));
            }
            1 => {
                let ty = self.g.rng.pick(&[Ty::Int, Ty::Opt, Ty::Res, Ty::Bool]);
                let value = self.g.expr(ty, e);
                let v = self.name(ty);
                self.line(&format!("let {v}: {} = {value};", ty.rust()));
                self.g.scope.push((v, ty, false));
            }
            2 | 3 => self.assign(e),
            4 | 5 => {
                let test = self.g.boolean(e);
                self.block(&format!("if {test}"), depth - 1, 3);
                if self.g.rng.below(2) == 0 {
                    self.block("else", depth - 1, 3);
                }
            }
            6 => {
                let (lo, hi) = (self.g.int(e), self.g.int(e));
                let i = self.name(Ty::Int);
                self.loops.push(false);
                self.g.scope.push((i.clone(), Ty::Int, false));
                // `a..b`, `a..=b`, and either stepped down with `.rev()`.
                let range = match self.g.rng.below(5) {
                    0 => format!("({lo}) % 4..=({hi}) % 6"),
                    1 => format!("(({lo}) % 4..({hi}) % 6).rev()"),
                    2 => format!("(({lo}) % 4..=({hi}) % 6).rev()"),
                    _ => format!("({lo}) % 4..({hi}) % 6"),
                };
                // Or stepped by `step_by`, with `rev` on either side.
                let range = if self.extra.below(3) == 0 { self.stepped(&lo, &hi) } else { range };
                self.block(&format!("for {i} in {range}"), depth - 1, 4);
                self.g.scope.pop();
                self.loops.pop();
            }
            7 => {
                // The counter is in no scope the generator draws from, so
                // only its own `+= 1` writes it.
                self.g.next_name += 1;
                let k = format!("k{}", self.g.next_name);
                self.line(&format!("let mut {k}: i32 = 0;"));
                let bound = self.g.int(e);
                let mut tail = format!("{k} += 1;");
                let test = match self.g.rng.below(3) {
                    0 => format!("{k} < ({bound}) % 5"),
                    1 => {
                        let more = self.g.without_try(|g| g.boolean(e));
                        format!("{k} < ({bound}) % 5 && {more}")
                    }
                    // A flag the body sets last, read by the test at the
                    // loop's head (not as it stood before the loop).
                    _ => {
                        let d = format!("d{}", self.g.next_name);
                        self.line(&format!("let mut {d}: bool = false;"));
                        let stop = self.g.without_try(|g| g.boolean(e));
                        tail = format!("{k} += 1; {d} = {stop};");
                        format!("{k} < ({bound}) % 5 && !{d}")
                    }
                };
                self.loops.push(false);
                self.block_from(&format!("while {test}"), Some(&tail), depth - 1, 4);
                self.loops.pop();
            }
            8 if !self.loops.is_empty() => {
                let test = self.g.boolean(e);
                let jump = self.jump(e);
                self.line(&format!("if {test} {{"));
                self.indent += 1;
                self.line(&format!("{jump};"));
                self.indent -= 1;
                self.line("}");
            }
            9 => {
                let test = self.g.boolean(e);
                let value = self.ret_value(e);
                self.line(&format!("if {test} {{"));
                self.indent += 1;
                self.line(&format!("return {value};"));
                self.indent -= 1;
                self.line("}");
            }
            10 | 11 => self.match_stmt(depth),
            13 => self.shadow(e),
            14 => self.observe(),
            15 | 16 => self.block("", depth - 1, 4),
            17 => self.shadow_dance(depth),
            18 => self.block_value(depth),
            19 => self.branch_value(depth),
            20 => self.closure_stmt(e),
            12 => self.panics(e),
            22..=24 => match self.g.rng.below(5) {
                0 => self.loop_value(depth),
                1 => self.while_let(depth),
                2 => self.let_else(e),
                3 => self.if_let(depth),
                _ => self.tuple_field(e),
            },
            21 => {
                // A bare block, then what it may have changed, read.
                self.block("", depth - 1, 3);
                self.observe();
            }
            _ => self.assign(e),
        }
    }

    /// A name for a new binding of type `ty`: a fresh one, one in scope
    /// (shadowing it), or one of [`NAMES`].
    fn name(&mut self, _ty: Ty) -> String {
        let n = match self.g.rng.below(10) {
            0..=3 => self.g.fresh(),
            4..=6 => {
                let names: Vec<String> = self.g.scope.iter().map(|(n, _, _)| n.clone()).collect();
                self.g.rng.pick(&names)
            }
            _ => self.g.rng.pick(NAMES).to_string(),
        };
        // `c`, `o`, and `r` are never rebound here, which could give them
        // another type or make them `let mut`; `shadow` rebinds them as
        // they are.
        if KEEP_TYPE.contains(&n.as_str()) {
            self.g.next_name += 1;
            format!("v{}", self.g.next_name)
        } else {
            n
        }
    }

    /// The innermost binding of some type, if any.
    fn any_var(&mut self) -> Option<(String, Ty)> {
        let ty = self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Opt, Ty::Res, Ty::Bool]);
        let v = self.g.var(ty)?;
        Some((v, ty))
    }

    /// `let x = <x as another type>;`: a name in scope bound again, from
    /// its own old value.
    fn shadow(&mut self, e: u32) {
        let Some((v, from)) = self.any_var() else { return self.assign(e) };
        let to = if KEEP_TYPE.contains(&v.as_str()) {
            from
        } else {
            self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Opt, Ty::Res, Ty::Bool])
        };
        let value = self.convert(&v, from, to, e);
        let m = if self.g.rng.below(3) == 0 && !KEEP_TYPE.contains(&v.as_str()) { "mut " } else { "" };
        self.line(&format!("let {m}{v}: {} = {value};", to.rust()));
        self.g.scope.push((v, to, !m.is_empty()));
    }

    /// An expression of type `to` that reads `v`, of type `from`.
    fn convert(&mut self, v: &str, from: Ty, to: Ty, e: u32) -> String {
        if self.g.rng.below(2) == 0 {
            if let Some(x) = self.convert_std(v, from, to, e) {
                return x;
            }
        }
        let g = &mut self.g;
        match (from, to) {
            (Ty::Int, Ty::Int) => {
                let op = g.rng.pick(&["+", "-", "^", "&", "|", "*"]);
                format!("({v} {op} {})", g.int(e))
            }
            (Ty::Int, Ty::Bool) => format!("({v} < {})", g.int(e)),
            (Ty::Int, Ty::Opt) if g.rng.below(2) == 0 => format!("Some({v})"),
            (Ty::Int, Ty::Opt) => format!("{v}.checked_add({})", g.int(e)),
            (Ty::Int, Ty::Res) => {
                g.without_try(|g| format!("if {v} > {} {{ Ok({v}) }} else {{ Err({}) }}", g.int(e), g.int(e)))
            }
            (Ty::Bool, Ty::Int) => g.without_try(|g| format!("if {v} {{ {} }} else {{ {} }}", g.int(e), g.int(e))),
            (Ty::Bool, Ty::Bool) => format!("(!{v} || {})", g.without_try(|g| g.boolean(e))),
            (Ty::Bool, Ty::Opt) => g.without_try(|g| format!("if {v} {{ Some({}) }} else {{ None }}", g.int(e))),
            (Ty::Bool, Ty::Res) => {
                g.without_try(|g| format!("if {v} {{ Ok({}) }} else {{ Err({}) }}", g.int(e), g.int(e)))
            }
            (Ty::Opt, Ty::Int) => format!("{v}.unwrap_or({})", g.int(e)),
            (Ty::Opt, Ty::Bool) => format!("{v}.is_some()"),
            // The closure's parameter shadows the name it maps.
            (Ty::Opt, Ty::Opt) => format!("{v}.map(|{v}| {v} ^ {})", g.lit()),
            (Ty::Opt, Ty::Res) => format!("{v}.ok_or({})", g.int(e)),
            (Ty::Res, Ty::Int) => format!("{v}.ok().unwrap_or({})", g.int(e)),
            (Ty::Res, Ty::Bool) => format!("{v}.ok().is_some()"),
            (Ty::Res, Ty::Opt) => format!("{v}.ok()"),
            (Ty::Res, Ty::Res) => format!("{v}.map_err(|{v}| {v} | {})", g.lit()),
        }
    }

    /// What `convert` gives through 0.18.0's std methods: a value argument
    /// (evaluated on both sides) may hold `?`, a closure's body reads only
    /// its parameter and literals. `unwrap` and `expect` panic on `None`;
    /// `Vec`s are built in place.
    fn convert_std(&mut self, v: &str, from: Ty, to: Ty, e: u32) -> Option<String> {
        let g = &mut self.g;
        let k = g.rng.below(5);
        Some(match (from, to, k) {
            (Ty::Opt, Ty::Int, 0) => format!("{v}.unwrap_or_else(|| {})", g.lit()),
            (Ty::Opt, Ty::Int, 1) => format!("{v}.map_or({}, |x| x ^ {})", g.int(e), g.lit()),
            (Ty::Opt, Ty::Int, 2) => format!("{v}.map_or_else(|| {}, |x| x | {})", g.lit(), g.lit()),
            (Ty::Opt, Ty::Int, 3) => format!("{v}.unwrap()"),
            (Ty::Opt, Ty::Int, _) => format!("{v}.expect(\"no {v}\")"),
            (Ty::Opt, Ty::Bool, 0 | 1) => format!("{v}.is_some_and(|x| x < {})", g.lit()),
            (Ty::Opt, Ty::Bool, _) => format!("{v}.is_none_or(|x| x != {})", g.lit()),
            (Ty::Opt, Ty::Opt, 0) => format!("{v}.and_then(|x| x.checked_mul({}))", g.lit()),
            (Ty::Opt, Ty::Opt, 1) => format!("{v}.filter(|x| *x > {})", g.lit()),
            (Ty::Opt, Ty::Opt, 2) => format!("{v}.or({})", g.expr(Ty::Opt, e)),
            (Ty::Opt, Ty::Opt, _) => format!("{v}.or_else(|| Some({}))", g.lit()),
            (Ty::Opt, Ty::Res, _) => format!("{v}.ok_or_else(|| {})", g.lit()),
            (Ty::Res, Ty::Int, 0 | 1) => format!("{v}.unwrap_or({})", g.int(e)),
            (Ty::Res, Ty::Int, 2) => format!("{v}.unwrap_or_else(|x| x ^ {})", g.lit()),
            (Ty::Res, Ty::Int, _) => format!("{v}.map_or({}, |x| x & {})", g.int(e), g.lit()),
            (Ty::Res, Ty::Bool, 0) => format!("{v}.is_ok()"),
            (Ty::Res, Ty::Bool, 1) => format!("{v}.is_err()"),
            (Ty::Res, Ty::Bool, 2) => format!("{v}.is_ok_and(|x| x > {})", g.lit()),
            (Ty::Res, Ty::Bool, _) => format!("{v}.is_err_and(|x| x == {})", g.lit()),
            (Ty::Res, Ty::Opt, _) => format!("{v}.err()"),
            (Ty::Res, Ty::Res, 0 | 1) => {
                format!("{v}.and_then(|x| if x > {} {{ Ok(x) }} else {{ Err(x) }})", g.lit())
            }
            (Ty::Res, Ty::Res, _) => format!("{v}.or_else(|x| if x < {} {{ Ok(x) }} else {{ Err(x) }})", g.lit()),
            (Ty::Int, Ty::Int, 0) => {
                format!("vec![{v}, {}, {}].iter().fold({}, |a, x| a ^ x)", g.int(e), g.lit(), g.lit())
            }
            (Ty::Int, Ty::Int, 1) => format!("vec![{v}, {}].iter().rev().fold(0i32, |a, x| a * 3 + x)", g.int(e)),
            (Ty::Int, Ty::Int, 2) => {
                format!(
                    "vec![{v}, {}].iter().zip(vec![{}, {}].iter()).map(|(x, y)| x - y).sum::<i32>()",
                    g.int(e),
                    g.lit(),
                    g.lit()
                )
            }
            (Ty::Int, Ty::Opt, 0) => format!("vec![{v}, {}].get({}).copied()", g.int(e), g.rng.below(4)),
            (Ty::Int, Ty::Opt, 1) => format!("vec![{v}, {}].first().copied()", g.int(e)),
            (Ty::Int, Ty::Opt, 2) => format!("vec![{}, {v}].last().copied()", g.int(e)),
            (Ty::Int, Ty::Bool, 0 | 1) => format!("vec![{}, {}].contains(&{v})", g.int(e), g.lit()),
            _ => return None,
        })
    }

    /// `assert!` with and without a message, and `panic!` under a test.
    fn panics(&mut self, e: u32) {
        let line = match (self.g.rng.below(3), self.g.var(Ty::Int)) {
            (0, Some(v)) => format!("assert!({v} != {});", self.g.lit()),
            (1, _) => {
                let test = self.g.boolean(e);
                let shown = self.g.var(Ty::Int).unwrap_or_else(|| "h0".to_string());
                format!("assert!({test}, \"failed with {{}}\", {shown});")
            }
            _ => {
                let test = self.g.boolean(e);
                format!("if {test} {{ panic!(\"stop at {{h0}}\") }}")
            }
        };
        self.line(&line);
    }

    /// `h0`, which every function returns mixed in, takes in a name in
    /// scope as it stands here.
    fn observe(&mut self) {
        let Some((v, ty)) = self.any_var() else { return };
        let x = match ty {
            Ty::Int => v,
            Ty::Bool => format!("if {v} {{ 1i32 }} else {{ 2i32 }}"),
            Ty::Opt => format!("{v}.unwrap_or(-7i32)"),
            Ty::Res => format!("{v}.ok().unwrap_or(-9i32)"),
        };
        self.line(&format!("h0 = h0.wrapping_mul(31).wrapping_add({x});"));
    }

    /// A bare block that writes a `let mut` from outside, binds its name
    /// again, writes that, and leaves; then the outer one is read.
    fn shadow_dance(&mut self, depth: u32) {
        let Some(v) = self.g.var_where(Ty::Int, true) else { return self.block("", depth - 1, 3) };
        self.line("{");
        let scope = self.g.scope.len();
        self.indent += 1;
        let value = self.g.int(2);
        self.line(&format!("{v} += {value};"));
        let to = self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Bool, Ty::Opt]);
        let value = self.convert(&v, Ty::Int, to, 2);
        self.line(&format!("let mut {v}: {} = {value};", to.rust()));
        self.g.scope.push((v.clone(), to, true));
        let n = 1 + self.g.rng.below(3);
        for _ in 0..n {
            self.stmt(depth - 1);
        }
        // Unless a statement above bound the name again.
        if self.g.scope.iter().rev().find(|(n, _, _)| *n == v) == Some(&(v.clone(), to, true)) {
            let value = self.g.expr(to, 2);
            self.line(&format!("{v} = {value};"));
        }
        self.observe();
        self.indent -= 1;
        self.g.scope.truncate(scope);
        self.line("}");
        self.line(&format!("h0 = h0.wrapping_mul(31).wrapping_add({v});"));
    }

    /// The statements of a block whose value is its tail, then the tail.
    fn value_body(&mut self, ty: Ty, depth: u32) {
        let scope = self.g.scope.len();
        self.indent += 1;
        let n = self.g.rng.below(4);
        for _ in 0..n {
            self.stmt(depth);
        }
        let tail = self.g.expr(ty, 2);
        self.line(&tail);
        self.indent -= 1;
        self.g.scope.truncate(scope);
    }

    /// `let y: T = { ..; tail };`
    fn block_value(&mut self, depth: u32) {
        let ty = self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Opt, Ty::Bool, Ty::Res]);
        let v = self.name(ty);
        let m = if self.g.rng.below(3) == 0 { "mut " } else { "" };
        self.line(&format!("let {m}{v}: {} = {{", ty.rust()));
        self.value_body(ty, depth - 1);
        self.line("};");
        self.g.scope.push((v, ty, !m.is_empty()));
    }

    /// `let y: T = if .. { ..; tail } else { ..; tail };`, or a `match`
    /// whose arms are such blocks.
    fn branch_value(&mut self, depth: u32) {
        let ty = self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Opt, Ty::Bool, Ty::Res]);
        let v = self.name(ty);
        if self.g.rng.below(2) == 0 {
            let test = self.g.boolean(2);
            self.line(&format!("let {v}: {} = if {test} {{", ty.rust()));
            self.value_body(ty, depth - 1);
            self.line("} else {");
            self.value_body(ty, depth - 1);
            self.line("};");
        } else {
            let st = self.g.rng.pick(&[Ty::Opt, Ty::Res]);
            let scrutinee = self.g.place(st, 2);
            let (some, none) = if st == Ty::Opt { ("Some", "None") } else { ("Ok", "Err") };
            self.line(&format!("let {v}: {} = match {scrutinee} {{", ty.rust()));
            self.indent += 1;
            let p = self.name(Ty::Int);
            self.line(&format!("{some}({p}) => {{"));
            self.g.scope.push((p, Ty::Int, false));
            self.value_body(ty, depth - 1);
            self.g.scope.pop();
            self.line("}");
            if st == Ty::Opt {
                self.line(&format!("{none} => {{"));
            } else {
                let p = self.name(Ty::Int);
                self.line(&format!("{none}({p}) => {{"));
                self.g.scope.push((p, Ty::Int, false));
            }
            let scope = self.g.scope.len() - usize::from(st == Ty::Res);
            self.value_body(ty, depth - 1);
            self.g.scope.truncate(scope);
            self.line("}");
            self.indent -= 1;
            self.line("};");
        }
        self.g.scope.push((v, ty, false));
    }

    /// A closure whose block binds its parameter's name again, called at
    /// once.
    fn closure_stmt(&mut self, e: u32) {
        // A closure is in no scope the generator draws from: its name is
        // one nothing in scope has, and it is called only here.
        let free: Vec<&str> = NAMES.iter().copied().filter(|n| !self.g.scope.iter().any(|(m, _, _)| m == n)).collect();
        let f = if free.is_empty() || self.g.rng.below(3) == 0 {
            self.g.next_name += 1;
            format!("f{}", self.g.next_name)
        } else {
            self.g.rng.pick(&free).to_string()
        };
        let p = self.name(Ty::Int);
        let scope = self.g.scope.len();
        self.g.closure += 1;
        self.g.no_try += 1;
        self.line(&format!("let {f} = |{p}: i32| {{"));
        self.indent += 1;
        self.g.scope.push((p.clone(), Ty::Int, false));
        let value = self.convert(&p, Ty::Int, Ty::Int, 2);
        self.line(&format!("let {p}: i32 = {value};"));
        self.g.scope.push((p.clone(), Ty::Int, false));
        let n = self.g.rng.below(3);
        for _ in 0..n {
            let ty = self.g.rng.pick(&[Ty::Int, Ty::Opt, Ty::Bool]);
            let value = self.g.expr(ty, 2);
            let v = self.name(ty);
            self.line(&format!("let {v}: {} = {value};", ty.rust()));
            self.g.scope.push((v, ty, false));
        }
        let tail = self.g.int(2);
        self.line(&tail);
        self.indent -= 1;
        self.line("};");
        self.g.scope.truncate(scope);
        self.g.no_try -= 1;
        self.g.closure -= 1;
        let arg = self.g.int(e);
        let v = self.name(Ty::Int);
        self.line(&format!("let {v}: i32 = {f}({arg});"));
        self.g.scope.push((v, Ty::Int, false));
    }

    /// `x = e;` or `x op= e;` on a `let mut` in scope, or a new one.
    fn assign(&mut self, e: u32) {
        let ty = self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Int, Ty::Opt, Ty::Bool]);
        let Some(v) = self.g.var_where(ty, true) else {
            let value = self.g.expr(ty, e);
            let v = self.g.fresh();
            self.line(&format!("let mut {v}: {} = {value};", ty.rust()));
            self.g.scope.push((v, ty, true));
            return;
        };
        let value = self.g.expr(ty, e);
        if ty == Ty::Int && self.g.rng.below(2) == 0 {
            let op = self.g.rng.pick(&["+=", "-=", "*=", "&=", "|=", "^="]);
            self.line(&format!("{v} {op} {value};"));
        } else {
            self.line(&format!("{v} = {value};"));
        }
    }

    /// `break` or `continue` of the innermost loop; a `loop` with a value
    /// breaks with an `i32`.
    fn jump(&mut self, e: u32) -> String {
        match (self.g.rng.below(2), self.loops.last()) {
            (0, _) => "continue".to_string(),
            (_, Some(true)) => format!("break {}", self.g.int(e)),
            _ => "break".to_string(),
        }
    }

    /// A counter nothing else writes, bumped first in a loop's body, so
    /// `continue` cannot skip it; and the bound it stops at, under 5.
    fn counter(&mut self, e: u32) -> (String, String) {
        self.g.next_name += 1;
        let k = format!("k{}", self.g.next_name);
        self.line(&format!("let mut {k}: i32 = 0;"));
        let bound = self.g.int(e);
        (k, format!("({bound}) % 5"))
    }

    /// `let v: i32 = loop { k += 1; if k > n { break e; } .. };`, its other
    /// `break`s carrying values too; or a `loop` as a statement.
    fn loop_value(&mut self, depth: u32) {
        let (k, bound) = self.counter(2);
        let valued = self.g.rng.below(3) != 0;
        let (head, out) = if valued {
            let v = self.name(Ty::Int);
            (format!("let {v}: i32 = loop"), Some(v))
        } else {
            ("loop".to_string(), None)
        };
        let stop = if valued { format!("break {}", self.g.int(2)) } else { "break".to_string() };
        self.loops.push(valued);
        self.block_from(&head, Some(&format!("{k} += 1; if {k} > {bound} {{ {stop}; }}")), depth - 1, 4);
        self.loops.pop();
        if let Some(v) = out {
            // The block's `}` ends a `let`.
            self.out.pop();
            self.out.push_str(";\n");
            self.g.scope.push((v, Ty::Int, false));
        }
    }

    /// `while let Some(v) = if k < n { .. } else { None } { k += 1; .. }`:
    /// the scrutinee runs before every pass; or on `Ok`, `Err`, a range.
    fn while_let(&mut self, depth: u32) {
        let (k, bound) = self.counter(2);
        let (pat, value, ty) = match self.g.rng.below(3) {
            0 => ("Some", self.g.without_try(|g| g.place(Ty::Opt, 2)), Ty::Opt),
            1 => ("Ok", self.g.without_try(|g| g.place(Ty::Res, 2)), Ty::Res),
            _ => ("", self.g.without_try(|g| g.int(2)), Ty::Int),
        };
        let other = match ty {
            Ty::Opt => "None".to_string(),
            Ty::Res => "Err(0)".to_string(),
            _ => "-1".to_string(),
        };
        let v = self.name(Ty::Int);
        let pattern = if pat.is_empty() { "0..=9".to_string() } else { format!("{pat}({v})") };
        let head = format!("while let {pattern} = if {k} < {bound} {{ {value} }} else {{ {other} }}");
        self.loops.push(false);
        let scope = self.g.scope.len();
        if !pat.is_empty() {
            self.g.scope.push((v, Ty::Int, false));
        }
        self.block_from(&head, Some(&format!("{k} += 1;")), depth - 1, 3);
        self.g.scope.truncate(scope);
        self.loops.pop();
    }

    /// `let Some(v) = o else { <leave> };`, then `v` in scope; on `Ok`,
    /// `Err`, or a range binding nothing too.
    fn let_else(&mut self, e: u32) {
        let leave = if !self.loops.is_empty() && self.g.rng.below(2) == 0 {
            self.jump(e)
        } else {
            format!("return {}", self.ret_value(e))
        };
        let v = self.name(Ty::Int);
        let m = if self.g.rng.below(3) == 0 { "mut " } else { "" };
        let (pattern, value, binds) = match self.g.rng.below(4) {
            0 => (format!("Some({m}{v})"), self.g.place(Ty::Opt, 2), true),
            1 => (format!("Ok({m}{v})"), self.g.place(Ty::Res, 2), true),
            2 => (format!("Err({m}{v})"), self.g.place(Ty::Res, 2), true),
            _ => ("0..=3".to_string(), self.g.int(e), false),
        };
        self.line(&format!("let {pattern} = ({value}) else {{ {leave}; }};"));
        if binds {
            self.g.scope.push((v, Ty::Int, !m.is_empty()));
        }
    }

    /// `if let Some(v) = o { .. } else { .. }`, on `Ok`, `Err`, a literal,
    /// or a range.
    fn if_let(&mut self, depth: u32) {
        let v = self.name(Ty::Int);
        let (pattern, value, binds) = match self.g.rng.below(5) {
            0 => (format!("Some({v})"), self.g.place(Ty::Opt, 2), true),
            1 => (format!("Ok({v})"), self.g.place(Ty::Res, 2), true),
            2 => (format!("Err({v})"), self.g.place(Ty::Res, 2), true),
            3 => ("7".to_string(), self.g.int(2), false),
            _ => ("0..=100".to_string(), self.g.int(2), false),
        };
        let scope = self.g.scope.len();
        if binds {
            self.g.scope.push((v, Ty::Int, false));
        }
        self.block(&format!("if let {pattern} = {value}"), depth - 1, 3);
        self.g.scope.truncate(scope);
        if self.g.rng.below(2) == 0 {
            self.block("else", depth - 1, 3);
        }
    }

    /// An associated const, a function, or a variant of `modules`'s
    /// module, named through `crate::`, `self::`, or the module, taken
    /// into `h0`. Drawn from `paths_rng` alone; the argument is an `i32` in
    /// scope or a literal.
    fn paths(&mut self) {
        let s = self.seed;
        let r = &mut self.paths_rng;
        // The innermost binding of a name is the one in scope, as `var` reads.
        let scope = &self.g.scope;
        let ints: Vec<&String> = (scope.iter().enumerate())
            .filter(|(i, (n, t, _))| *t == Ty::Int && !scope[i + 1..].iter().any(|(m, _, _)| m == n))
            .map(|(_, (v, _, _))| v)
            .collect();
        let x = match r.below(ints.len() as u64 + 2) as usize {
            i if i < ints.len() => ints[i].clone(),
            i if i == ints.len() => {
                format!("{}i32", r.pick(&[0i64, 1, -1, 1000, -1000, 2_147_483_647, -2_147_483_648]))
            }
            _ => format!("{}i32", r.below(4001) as i64 - 2000),
        };
        let module = r.pick(&["crate::", "self::", ""]);
        let read = match r.below(6) {
            0 => format!("{module}s{s}::K{s}::A"),
            1 => format!("{module}s{s}::K{s}::B"),
            2 => format!("{module}s{s}::K{s}::get()"),
            3 => format!("{module}s{s}::inner{s}::less{s}({x})"),
            4 => format!("{module}s{s}::inner{s}::more{s}({x})"),
            _ => format!("if matches!({module}s{s}::pick{s}({x}), {module}s{s}::E{s}::A) {{ 1i32 }} else {{ 2i32 }}"),
        };
        self.line(&format!("h0 = h0.wrapping_mul(31).wrapping_add({read});"));
    }

    /// A tuple bound whole, read by `.0` and `.1`.
    fn tuple_field(&mut self, e: u32) {
        let (x, c) = (self.g.int(e), self.g.boolean(e));
        self.g.next_name += 1;
        let t = format!("tp{}", self.g.next_name);
        self.line(&format!("let {t}: (i32, bool) = ({x}, {c});"));
        let v = self.name(Ty::Int);
        let other = self.g.int(e);
        self.line(&format!("let {v}: i32 = if {t}.1 {{ {t}.0 }} else {{ {other} }};"));
        self.g.scope.push((v, Ty::Int, false));
    }

    /// A `match` on an `Option` or `Result` whose arms are blocks, or, in
    /// a loop, a jump.
    fn match_stmt(&mut self, depth: u32) {
        let ty = self.g.rng.pick(&[Ty::Opt, Ty::Res]);
        let scrutinee = self.g.place(ty, 2);
        let (some, none) = if ty == Ty::Opt { ("Some", "None") } else { ("Ok", "Err") };
        self.line(&format!("match {scrutinee} {{"));
        self.indent += 1;
        let v = self.name(Ty::Int);
        self.g.scope.push((v.clone(), Ty::Int, false));
        self.block(&format!("{some}({v}) =>"), depth - 1, 3);
        self.g.scope.pop();
        let other = if ty == Ty::Opt { none.to_string() } else { format!("{none}(_)") };
        if !self.loops.is_empty() && self.g.rng.below(2) == 0 {
            let jump = self.jump(2);
            self.line(&format!("{other} => {jump},"));
        } else {
            self.block(&format!("{other} =>"), depth - 1, 3);
        }
        self.indent -= 1;
        self.line("}");
    }
}

/// A module per seed for `Body::paths`: a type with associated consts (one
/// private, read by a method, and one that reads another as `Self::`), an
/// enum, and a child module whose functions reach them through `super::`
/// and `crate::`, overflowing at the edges of `i32`.
fn modules(seed: u64) -> String {
    let mut rng = crate::support::Rng::new(seed ^ 0x9a7e);
    let a = 1 + rng.below(1000) as i32;
    let b = rng.below(2000) as i32 - 1000;
    let s = seed;
    format!(
        "pub mod s{s} {{\n    pub struct K{s} {{}}\n\n    impl K{s} {{\n        pub const A: i32 = {a};\n        pub const B: i32 = Self::A * 3 + {b};\n        const C: i32 = -Self::B;\n\n        pub fn get() -> i32 {{\n            Self::C + K{s}::A\n        }}\n    }}\n\n\
         \x20   pub enum E{s} {{\n        A,\n        B,\n    }}\n\n\
         \x20   pub fn pick{s}(x: i32) -> E{s} {{\n        if x > K{s}::B {{\n            E{s}::A\n        }} else {{\n            self::E{s}::B\n        }}\n    }}\n\n\
         \x20   pub mod inner{s} {{\n        pub fn less{s}(x: i32) -> i32 {{\n            x - super::K{s}::A\n        }}\n\n\
         \x20       pub fn more{s}(x: i32) -> i32 {{\n            x + crate::s{s}::K{s}::B * super::super::s{s}::K{s}::A\n        }}\n    }}\n}}\n\n"
    )
}

/// From 0.22.0, per seed: an error enum with two `impl From` (from `i32`
/// and from a crate type, the conversion overflowing now and then), and a
/// function that chains `?` over both sources in a drawn order, inline or
/// as statements, beside a `?` whose types match and a conversion spelled
/// `Fe::from`, `From::from`, or `into()`. The public function reads the
/// result back as an `i32`. Its draws are its own, so the other functions
/// are the ones each seed drew before.
fn conversions(seed: u64, fns: &mut Vec<(String, Ty)>) -> String {
    let mut rng = Rng::new(seed ^ 0x0220);
    let s = seed;
    let k = 1 + rng.below(1000) as i32;
    let m = 2 + rng.below(5) as i32;
    let low = rng.pick(&["n.wrapping_mul(3)", "n * 2", "n - 7", "n"]);
    let spelled = rng.pick(&[format!("Fe{s}::from(b)"), "From::from(b)".to_string(), "b.into()".to_string()]);
    let chain = match rng.below(4) {
        0 => format!("let x = r?;\n    let y = low{s}(a.wrapping_add(x))?;\n    Ok(x ^ y)"),
        1 => format!("let y = low{s}(a)?;\n    let x = r?;\n    Ok(x.wrapping_sub(y))"),
        2 => format!("Ok(low{s}(a)?.wrapping_add(r?))"),
        _ => format!("let z = same{s}(b)?;\n    Ok(low{s}(z)? ^ r?)"),
    };
    let name = format!("t{s}_from");
    fns.push((name.clone(), Ty::Int));
    format!(
        "#[derive(Debug, Clone, PartialEq)]\npub enum Fe{s} {{\n    Low(i32),\n    Neg(i32),\n    Gone,\n}}\n\n\
         pub struct Lo{s} {{\n    pub v: i32,\n}}\n\n\
         impl From<i32> for Fe{s} {{\n    fn from(n: i32) -> Self {{\n        Fe{s}::Low({low})\n    }}\n}}\n\n\
         impl From<Lo{s}> for Fe{s} {{\n    fn from(l: Lo{s}) -> Self {{\n        if l.v < 0 {{\n            Fe{s}::Neg(l.v - {k})\n        }} else {{\n            Fe{s}::Gone\n        }}\n    }}\n}}\n\n\
         fn low{s}(x: i32) -> Result<i32, Lo{s}> {{\n    if x % {m} == 0 {{\n        Err(Lo{s} {{ v: x }})\n    }} else {{\n        Ok(x / 2)\n    }}\n}}\n\n\
         fn same{s}(x: i32) -> Result<i32, Fe{s}> {{\n    if x == {k} {{\n        Err(Fe{s}::Gone)\n    }} else {{\n        Ok(x)\n    }}\n}}\n\n\
         fn chain{s}(a: i32, b: i32, r: Result<i32, i32>, c: bool) -> Result<i32, Fe{s}> {{\n    if c && b < 0 {{\n        return Err({spelled});\n    }}\n    {chain}\n}}\n\n\
         pub fn {name}({PARAMS}) -> i32 {{\n    match chain{s}(a, b, r, c) {{\n        Ok(v) => v,\n        Err(Fe{s}::Low(n)) => n ^ 1,\n        Err(Fe{s}::Neg(n)) => n ^ 2,\n        Err(Fe{s}::Gone) => 3,\n    }}\n}}\n\n"
    )
}

/// A trait per seed for `Body::trait_call` (design/01 §7.25): a method and
/// a default that calls it through the dictionary, a struct that takes the
/// default, an enum that overrides it in some seeds, and a function over
/// any implementor. `+` and `-` overflow at the edges of `i32`.
fn traits(seed: u64) -> String {
    let mut rng = crate::support::Rng::new(seed ^ 0x7a17);
    let a = rng.below(201) as i32 - 100;
    let b = rng.below(7) as i32 - 3;
    let c = rng.below(2001) as i32 - 1000;
    let d = rng.below(201) as i32 - 100;
    let s = seed;
    let own = if rng.below(2) == 0 {
        "\n\n    fn w(&self, k: i32) -> i32 {\n        self.v() + k\n    }".to_string()
    } else {
        String::new()
    };
    format!(
        "pub trait Tr{s} {{\n    fn v(&self) -> i32;\n\n    fn w(&self, k: i32) -> i32 {{\n        self.v().wrapping_mul(k) - {c}\n    }}\n}}\n\n\
         pub struct Ta{s} {{\n    pub n: i32,\n}}\n\n\
         impl Tr{s} for Ta{s} {{\n    fn v(&self) -> i32 {{\n        self.n + {a}\n    }}\n}}\n\n\
         pub enum Tb{s} {{\n    P(i32),\n    Q,\n}}\n\n\
         impl Tr{s} for Tb{s} {{\n    fn v(&self) -> i32 {{\n        match self {{\n            Tb{s}::P(n) => n.wrapping_mul({b}),\n            Tb{s}::Q => {d},\n        }}\n    }}{own}\n}}\n\n\
         pub fn tsum{s}<T: Tr{s}>(xs: &[T], k: i32) -> i32 {{\n    let mut t = 0i32;\n    for x in xs {{\n        t = t.wrapping_add(x.w(k));\n    }}\n    t\n}}\n\n"
    )
}

fn generate(seed: u64, count: usize, source: &mut String, fns: &mut Vec<(String, Ty)>) {
    source.push_str(&modules(seed));
    source.push_str(&conversions(seed, fns));
    source.push_str(&traits(seed));
    let mut body = Body {
        g: Gen::new(seed ^ 0x5717),
        seed,
        paths_rng: crate::support::Rng::new(seed ^ 0x9a7f),
        loops: Vec::new(),
        out: String::new(),
        indent: 1,
        extra: Rng::new(seed ^ 0x0221),
        traits_rng: Rng::new(seed ^ 0x0222),
        lits_rng: Rng::new(seed ^ 0x0240),
    };
    for i in 0..count {
        let ret = body.g.rng.pick(&[Ty::Int, Ty::Bool, Ty::Opt, Ty::Res]);
        body.g.ret = ret;
        body.g.next_name = 0;
        body.g.scope = Gen::params();
        body.out.clear();
        body.line("let mut h0: i32 = 0;");
        let n = 2 + body.g.rng.below(4);
        for _ in 0..n {
            body.stmt(3);
        }
        body.observe();
        // `h0` is in no scope, so nothing but `observe` writes it.
        let tail = match ret {
            Ty::Int => format!("h0.wrapping_add({})", body.ret_value(2)),
            Ty::Bool => format!("((h0 & 1) == 0) != {}", body.g.boolean(2)),
            Ty::Opt if body.g.rng.below(2) == 0 => format!("Some(h0 ^ {})", body.g.int(2)),
            Ty::Res if body.g.rng.below(2) == 0 => format!("Ok(h0 ^ {})", body.g.int(2)),
            _ => body.ret_value(2),
        };
        body.line(&tail);
        let name = format!("t{seed}_f{i}");
        writeln!(source, "pub fn {name}({PARAMS}) -> {} {{\n{}}}\n", ret.rust(), body.out).expect("write");
        fns.push((name, ret));
    }
}

#[test]
fn generated_statements_match_rust() {
    let seeds = seeds(SEEDS);
    let count = fn_count(40);
    let (mut source, mut fns) = (String::new(), Vec::new());
    for &seed in &seeds {
        generate(seed, count, &mut source, &mut fns);
    }
    compare("generated_statements", &seeds, &source, &fns);
}
