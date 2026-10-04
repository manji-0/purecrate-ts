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
//! `PURECRATE_GEN_SEED`, `PURECRATE_GEN_FNS`, `PURECRATE_GEN_DUMP`, and
//! `PURECRATE_GEN_TYPES=report` work as in `generated_equivalence.rs`.
//!
//! What it found: one hole (`match o.ok_or(x.ok_or(e)?)` left only an
//! inline function, and the `Err` arm ran), a temporary declared twice,
//! and output tsc refused (`order_of_eval.rs`, `narrowing.rs`). Seeds 1 to
//! 120 agree on every value; 60 of them type-check. Of the rest, 47 stop
//! only at narrowing TS does where control flow joins or loops, which the
//! printer's fold does not follow (roadmap §3); 13 also at `??` on a known
//! `null`, unreachable code, or a temporary TS types from itself in a loop.

use std::fmt::Write as _;

use crate::generated_equivalence::{compare, fn_count, seeds, Gen, Ty, PARAMS};

struct Body {
    g: Gen,
    /// Loops around the current statement, for `break` and `continue`.
    loops: usize,
    /// Lines written so far, indented.
    out: String,
    indent: usize,
}

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
        self.line(&format!("{head} {{"));
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
        let pick = if depth == 0 { self.g.rng.below(4) } else { self.g.rng.below(13) };
        match pick {
            0 => {
                let ty = self.g.rng.pick(&[Ty::Int, Ty::Int, Ty::Opt, Ty::Bool]);
                let value = self.g.expr(ty, e);
                let v = self.g.fresh();
                self.line(&format!("let mut {v}: {} = {value};", ty.rust()));
                self.g.scope.push((v, ty, true));
            }
            1 => {
                let ty = self.g.rng.pick(&[Ty::Int, Ty::Opt, Ty::Res, Ty::Bool]);
                let value = self.g.expr(ty, e);
                let v = self.g.fresh();
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
                let i = self.g.fresh();
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
                let test = if self.g.rng.below(2) == 0 {
                    format!("{k} < ({bound}) % 5")
                } else {
                    let more = self.g.without_try(|g| g.boolean(e));
                    format!("{k} < ({bound}) % 5 && {more}")
                };
                self.loops += 1;
                self.block_from(&format!("while {test}"), Some(&format!("{k} += 1;")), depth - 1, 4);
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
            _ => self.assign(e),
        }
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
        let v = self.g.fresh();
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

/// Seeds checked on every run.
const SEEDS: &[u64] = &[1, 2, 3, 4, 5, 6, 7, 8];

fn generate(seed: u64, count: usize, source: &mut String, fns: &mut Vec<(String, Ty)>) {
    let mut body = Body { g: Gen::new(seed ^ 0x5717), loops: 0, out: String::new(), indent: 1 };
    for i in 0..count {
        let ret = body.g.rng.pick(&[Ty::Int, Ty::Bool, Ty::Opt, Ty::Res]);
        body.g.ret = ret;
        body.g.next_name = 0;
        body.g.scope = Gen::params();
        body.out.clear();
        let n = 2 + body.g.rng.below(4);
        for _ in 0..n {
            body.stmt(3);
        }
        let tail = body.ret_value(2);
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
