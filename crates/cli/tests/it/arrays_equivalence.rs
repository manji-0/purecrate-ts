//! Fixed-size arrays `[T; N]` (design/01 §7.27): parameters, results,
//! fields, variant payloads, crate consts, and locals; built by `[a, b]`
//! and `[x; N]`; indexing in and past range, `len`, iteration, `get`,
//! `first`, `last`, `contains`, slicing, `==`, and `cmp`; slice patterns
//! exhaustive by the one length; a local written in place, and the copy
//! `let c = a` keeps when `a` is written after. Then the length check a
//! `pub` function makes on entry, wherever in the argument the array is.

use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_pack::assemble;
use purecrate_syntax::parse_source;

use crate::support;

purecrate_canon::fixture!(mod arrays = "fixtures/arrays.rs");

use arrays::{Frame, Packet};

const BYTES: [u8; 6] = [0, 1, 2, 127, 200, 255];

#[test]
fn generated_arrays_match_rust() {
    support::equivalence("arrays", arrays::SOURCE, |cases| {
        for a in BYTES {
            for b in BYTES {
                let four = [a, b, a.wrapping_add(b), 255 - a];
                cases.push(case!(arrays::p08_array(four)));
                cases.push(case!(arrays::split(four)));
                cases.push(case!(arrays::total(&four)));
                cases.push(case!(arrays::to_vec(four)));
                cases.push(case!(arrays::middle(four)));
                cases.push(case!(arrays::code([a, b])));
                cases.push(case!(arrays::literal(a, b)));
                cases.push(case!(arrays::same([a, b], [b, a])));
                cases.push(case!(arrays::order([a, b], [b, a])));
                cases.push(case!(arrays::order([a, b], [a, b])));
                cases.push(case!(arrays::maybe(Some([a, b]))));
                cases.push(case!(arrays::grid([[a, b], [b, a]])));
                cases.push(case!(arrays::doubled([a, b, 3])));
                cases.push(case!(arrays::written(a)));
                cases.push(case!(arrays::copied_then_written([a, b, 1])));
            }
        }
        cases.push(case!(arrays::maybe(None::<[u8; 2]>)));
        for i in [0usize, 1, 3, 4, 5, usize::MAX >> 11] {
            cases.push(case!(arrays::at([9, 8, 7, 6], i)));
            cases.push(case!(arrays::get([-1, 0, i32::MAX], i)));
            cases.push(case!(arrays::write_at(i)));
            cases.push(case!(arrays::digit(i)));
            cases.push(case!(arrays::square(i)));
            cases.push(case!(arrays::slots(i)));
            cases.push(case!(arrays::slots_from_local(i)));
            cases.push(case!(arrays::empties(i)));
        }
        for k in [arrays::Pick::A, arrays::Pick::B, arrays::Pick::C] {
            cases.push(case!(arrays::picks(k)));
        }
        cases.push(case!(arrays::tally()));
        for b in [[0u32, 0, 0], [1, 2, 3], [u32::MAX, 0, 0], [u32::MAX, 1, 0], [2_147_483_648, 2_147_483_647, 1]] {
            cases.push(case!(arrays::len(b)));
            cases.push(case!(arrays::sum(b)));
        }
        for b in [[0u16, 0, 0], [3, 65535, 2], [1, 2, 3]] {
            cases.push(case!(arrays::by_value(b)));
        }
        for x in [0u16, 7, u16::MAX] {
            cases.push(case!(arrays::repeat(x)));
        }
        for c in ['a', 'é', '😀', '\u{ffff}'] {
            cases.push(case!(arrays::repeat_const(c)));
            cases.push(case!(arrays::has(['a', '😀', '\u{ffff}'], c)));
        }
        for b in [[0i32, 0, 0, 0], [5, 1, 1, 2], [2, 9, 9, 5], [i32::MIN, 0, 0, i32::MAX], [i32::MAX, 0, 0, -1]] {
            cases.push(case!(arrays::ends(b)));
        }
        for b in [[i32::MIN, 0, i32::MAX], [1, 2, 3]] {
            cases.push(case!(arrays::first_last(b)));
        }
        let words = ["", "a", "é", "😀", "\u{ffff}"];
        for a in words {
            for b in words {
                cases.push(case!(arrays::words_less([a.to_string(), b.to_string()], [b.to_string(), a.to_string()])));
                cases.push(case!(arrays::words_less([a.to_string(), "x".to_string()], [a.to_string(), b.to_string()])));
            }
        }
        let packets = [
            Packet { tag: [1, 2], body: vec![], note: None },
            Packet { tag: [255, 255], body: vec![[1, 2, 3, 4], [255, 255, 255, 255]], note: Some(['é', 'x']) },
            Packet { tag: [0, 0], body: vec![[0; 4]], note: Some(['😀', 'a']) },
        ];
        for p in &packets {
            cases.push(case!(arrays::packet_sum(p)));
        }
        let frames = [
            Frame::Short([1, 2]),
            Frame::Long { bytes: [1, 2, 3, 4], more: true },
            Frame::Pair([1, 2], [3, 4, 5]),
            Frame::Empty,
        ];
        for f in frames {
            cases.push(case!(arrays::frame_len(f)));
        }
        cases.push(case!(arrays::rows(Vec::<[u8; 2]>::new())));
        cases.push(case!(arrays::rows(vec![[1, 2], [3, 4]])));
        cases.push(case!(arrays::magic()));
    });
}

#[test]
fn generated_array_stages_match_rust() {
    support::equivalence("arrays_more", arrays::SOURCE, |cases| {
        for a in [0u8, 1, 200, 255] {
            for b in [0u8, 3, 255] {
                cases.push(case!(arrays::pair_code([a, b], [b, a])));
                cases.push(case!(arrays::pair_code([0, a], [b, 0])));
                cases.push(case!(arrays::local_literal(a)));
                cases.push(case!(arrays::running([a, b, 7])));
                cases.push(case!(arrays::zipped([a, b, 1], [b, a])));
                cases.push(case!(arrays::blocks([a, b])));
                cases.push(case!(arrays::walked_copy([a, b, 9])));
                cases.push(case!(arrays::generic_slice([a, b, 1])));
                cases.push(case!(arrays::mut_let([a, b, 1])));
            }
        }
        for b in [[0u32, 0, 0, 0], [1, 2, 3, 4], [4, 3, 2, 1], [u32::MAX, 1, 0, 2], [1, u32::MAX, 0, 0]] {
            cases.push(case!(arrays::stages(b)));
        }
        for b in [[0u32, 0, 0], [3, 1, 2], [u32::MAX, u32::MAX, 1]] {
            cases.push(case!(arrays::through_slice(b)));
        }
        cases.push(case!(arrays::sorted(vec![[2, 1], [1, 9], [1, 2], [255, 0], [1, 2]])));
        cases.push(case!(arrays::sorted(Vec::<[u8; 2]>::new())));
        for (a, b) in [("", "é"), ("😀", "x"), ("a\u{301}", "\u{ffff}")] {
            cases.push(case!(arrays::words([a.to_string(), b.to_string()])));
        }
    });
}

const SOURCE: &str = r#"
use std::collections::BTreeMap;

pub fn one(b: [u8; 4]) -> u8 {
    helper(&b)
}

fn helper(b: &[u8; 4]) -> u8 {
    b[0]
}

pub struct Row {
    pub cells: [u8; 2],
    pub label: String,
}

pub enum Shape {
    Square([u16; 2]),
    Grid { rows: Vec<[u16; 2]> },
    Empty,
}

pub enum Tree {
    Leaf([u8; 2]),
    Node(Box<Tree>, Box<Tree>),
}

pub struct Count {
    pub n: u8,
}

pub struct Sealed {
    cells: [u8; 2],
}

impl Sealed {
    pub fn new(cells: [u8; 2]) -> Sealed {
        Sealed { cells }
    }

    pub fn first(&self) -> u8 {
        self.cells[0]
    }
}

pub fn deep(r: Row, s: Shape, xs: Vec<Option<[u8; 3]>>, pair: (u8, [u8; 1]), grid: [[u8; 2]; 2], m: BTreeMap<String, [u8; 2]>) -> usize {
    let shape = match s {
        Shape::Square(c) => c.len(),
        Shape::Grid { rows } => rows.len(),
        Shape::Empty => 0,
    };
    let (_, last) = pair;
    r.cells.len() + shape + xs.len() + last.len() + grid.len() + m.len()
}

pub fn leaves(t: Tree) -> usize {
    match t {
        Tree::Leaf(_) => 1,
        Tree::Node(a, b) => leaves(*a) + leaves(*b),
    }
}

pub fn unchecked(c: Count, s: Sealed) -> u8 {
    s.first() + c.n
}
"#;

const DRIVER: &str = r#"
import { deep, leaves, one, Sealed, unchecked } from "./src/index.ts";
const out = [];
const panics = (f) => {
  try {
    f();
    return "returned";
  } catch (e) {
    return e instanceof Error && e.name === "Panic" ? e.message : `threw ${e}`;
  }
};
const row = (cells) => ({ cells, label: "a" });
const ok = () => [row([1, 2]), { kind: "Empty" }, [], [0, [7]], [[1, 2], [3, 4]], new Map()];
const leaf = (value) => ({ kind: "Leaf", value });
for (const [what, f, name, found, len] of [
  ["short", () => one([1, 2, 3]), "b", "an array of length 3", 4],
  ["long", () => one([1, 2, 3, 4, 5]), "b", "an array of length 5", 4],
  ["empty", () => one([]), "b", "an array of length 0", 4],
  ["not an array", () => one("abcd"), "b", "a non-array", 4],
  ["in a struct", () => deep(row([1]), ...ok().slice(1)), "r", "an array of length 1", 2],
  ["in a variant", () => deep(ok()[0], { kind: "Square", value: [1, 2, 3] }, ...ok().slice(2)), "s", "an array of length 3", 2],
  ["in a variant's Vec", () => deep(ok()[0], { kind: "Grid", rows: [[1, 2], [3]] }, ...ok().slice(2)), "s", "an array of length 1", 2],
  ["in an Option in a Vec", () => { const a = ok(); a[2] = [null, [1, 2]]; return deep(...a); }, "xs", "an array of length 2", 3],
  ["in a tuple", () => { const a = ok(); a[3] = [0, []]; return deep(...a); }, "pair", "an array of length 0", 1],
  ["outer of nested", () => { const a = ok(); a[4] = [[1, 2]]; return deep(...a); }, "grid", "an array of length 1", 2],
  ["inner of nested", () => { const a = ok(); a[4] = [[1, 2], [3]]; return deep(...a); }, "grid", "an array of length 1", 2],
  ["in a map's value", () => { const a = ok(); a[5] = new Map([["k", [1]]]); return deep(...a); }, "m", "an array of length 1", 2],
  ["deep in a recursive type", () => leaves({ kind: "Node", content: [leaf([1, 2]), { kind: "Node", content: [leaf([1, 2]), leaf([1])] }] }), "t", "an array of length 1", 2],
  ["through a constructor", () => Sealed.new([1]), "cells", "an array of length 1", 2],
]) {
  const want = `\`${name}\` holds ${found} where one of length ${len} goes`;
  const got = panics(f);
  if (got !== want) out.push(`${what}: ${got}`);
}
for (const [what, f, want] of [
  ["one", () => one([5, 2, 3, 4]), 5],
  ["deep", () => deep(row([1, 2]), { kind: "Grid", rows: [[1, 2], [3, 4]] }, [null, [1, 2, 3]], [1, [2]], [[1, 2], [3, 4]], new Map([["k", [1, 2]]])), 10],
  ["a tree", () => leaves({ kind: "Node", content: [leaf([1, 2]), leaf([3, 4])] }), 2],
  ["a non-array", () => unchecked({ n: 3 }, Sealed.new([1, 2])), 4],
]) {
  const got = f();
  if (got !== want) out.push(`${what}: ${got}, want ${want}`);
}
console.log(out.length === 0 ? "ok" : out.join("\n"));
"#;

#[test]
fn a_wrong_length_panics_on_entry() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let krate = parse_source("lengths", SOURCE).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("rejected: {d:#?}"));
    let dir = support::scratch("array-lengths");
    support::write_package(&dir, &assemble(&typed));
    support::typecheck(&dir);
    let one = fs::read_to_string(dir.join("src/one.ts")).expect("one.ts");
    assert_eq!(one.matches("Slice.lengths").count(), 1, "only the pub function checks:\n{one}");
    let unchecked = fs::read_to_string(dir.join("src/unchecked.ts")).expect("unchecked.ts");
    assert!(!unchecked.contains("Slice.lengths"), "neither holds an array a caller wrote:\n{unchecked}");
    fs::write(dir.join("driver.ts"), DRIVER).expect("write driver");
    let output = Command::new("node")
        .arg(format!("--conditions={}", support::SOURCE_CONDITION))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.trim() == "ok",
        "in {}:\n{stdout}\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}
