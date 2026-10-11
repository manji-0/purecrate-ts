//! Function bodies generated from a seed: `let` and `let mut`, assignment
//! and `op=`, `if` and `match` as statements, range `for` and `while`,
//! `break`, `continue`, early `return`, and `?` wherever a statement holds
//! an expression (a range's ends, a `while` test, an `if` test). The
//! expressions inside are `generated_equivalence.rs`'s; this test adds the
//! statements around them, where the audit of 0.8.1 found a `?` in a
//! range's end that ran before its start.
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
//! followed TS's control flow, roadmap §8.16 and §8.17). Drawing blocks,
//! shadowing, and made-up names, it found a block's binding that outlived
//! the block or was declared twice, a closure dropped where its block
//! folded, and narrowing TS keeps in a closure or through a `bool` that
//! decides only when it fails (`review_holes3.rs`).

use std::fmt::Write as _;

use crate::generated_equivalence::{compare, Gen, Ty, PARAMS};
use crate::support::generated::{fn_count, seeds, SEEDS};

struct Body {
    g: Gen,
    /// Loops around the current statement, for `break` and `continue`.
    loops: usize,
    /// Lines written so far, indented.
    out: String,
    indent: usize,
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

    fn stmt(&mut self, depth: u32) {
        let e = 2;
        let pick = if depth == 0 { self.g.rng.pick(&[0, 1, 2, 3, 13, 13, 14]) } else { self.g.rng.below(23) };
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
                self.loops += 1;
                self.g.scope.push((i.clone(), Ty::Int, false));
                self.block(&format!("for {i} in ({lo}) % 4..({hi}) % 6"), depth - 1, 4);
                self.g.scope.pop();
                self.loops -= 1;
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
                self.loops += 1;
                self.block_from(&format!("while {test}"), Some(&tail), depth - 1, 4);
                self.loops -= 1;
            }
            8 if self.loops > 0 => {
                let test = self.g.boolean(e);
                let jump = self.g.rng.pick(&["break", "continue"]);
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
        if self.loops > 0 && self.g.rng.below(2) == 0 {
            let jump = self.g.rng.pick(&["break", "continue"]);
            self.line(&format!("{other} => {jump},"));
        } else {
            self.block(&format!("{other} =>"), depth - 1, 3);
        }
        self.indent -= 1;
        self.line("}");
    }
}

fn generate(seed: u64, count: usize, source: &mut String, fns: &mut Vec<(String, Ty)>) {
    let mut body = Body { g: Gen::new(seed ^ 0x5717), loops: 0, out: String::new(), indent: 1 };
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
