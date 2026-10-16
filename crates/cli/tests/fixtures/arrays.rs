// Fixed-size arrays `[T; N]` (design/01 §7.27): a `ReadonlyArray<T>` whose
// length a `pub` function checks on entry. Parameters, results, fields, a
// variant's payload, crate consts, and locals; built by `[a, b]` and
// `[x; N]`, read as a slice is, matched by slice patterns exhaustive by the
// one length, and written only as a local `let mut` array of its own.

use std::cmp::Ordering;

pub const WIDTH: usize = 4;
pub const HALF: usize = WIDTH / 2;

/// A table, as domain code keeps one.
pub const DIGITS: [char; 4] = ['0', '1', 'x', 'é'];
pub const SQUARES: [u32; WIDTH] = [0, 1, 4, 9];
pub const MAGIC: &[u8; 4] = b"\x89PNG";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub tag: [u8; 2],
    pub body: Vec<[u8; WIDTH]>,
    pub note: Option<[char; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Short([u8; 2]),
    Long { bytes: [u8; 4], more: bool },
    Pair([u16; 2], [u16; 3]),
    Empty,
}

/// The corpus's shape: the first byte.
pub fn p08_array(b: [u8; 4]) -> u8 {
    b[0]
}

/// Rust's message past the end.
pub fn at(b: [u8; 4], i: usize) -> u8 {
    b[i]
}

pub fn len(b: [u32; 3]) -> usize {
    b.len()
}

/// Overflows only where the sum does.
pub fn sum(b: [u32; 3]) -> u32 {
    b.iter().sum()
}

pub fn total(b: &[u8; WIDTH]) -> u32 {
    let mut t = 0u32;
    for x in b {
        t += u32::from(*x);
    }
    t
}

pub fn by_value(b: [u16; 3]) -> u16 {
    let mut m = 0u16;
    for x in b {
        m = m.max(x);
    }
    m
}

pub fn literal(a: u8, b: u8) -> [u8; 2] {
    [a, b]
}

/// `x` once, however many slots.
pub fn repeat(x: u16) -> [u16; 3] {
    [x; 3]
}

pub fn repeat_const(x: char) -> [char; HALF] {
    [x; HALF]
}

/// Irrefutable: the one length.
pub fn split(b: [u8; 4]) -> (u8, u8, u8, u8) {
    let [a, b2, c, d] = b;
    (a, b2, c, d)
}

/// No `[]` arm: a `[u8; 2]` has two elements.
pub fn code(b: [u8; 2]) -> u32 {
    match b {
        [0, x] => u32::from(x),
        [x, 0] => u32::from(x) * 100,
        [x, y] => u32::from(x) + u32::from(y) * 1000,
    }
}

pub fn ends(b: [i32; 4]) -> i32 {
    match b {
        [first, .., last] if first > last => first - last,
        [_, rest @ ..] => rest.len() as i32,
    }
}

pub fn first_last(b: [i32; 3]) -> (i32, i32) {
    let f = match b.first() {
        Some(x) => *x,
        None => 0,
    };
    let l = match b.last() {
        Some(x) => *x,
        None => 0,
    };
    (f, l)
}

pub fn get(b: [i32; 3], i: usize) -> i32 {
    match b.get(i) {
        Some(x) => *x,
        None => -1,
    }
}

pub fn has(b: [char; 3], c: char) -> bool {
    b.contains(&c)
}

pub fn to_vec(b: [u8; 4]) -> Vec<u8> {
    b.iter().copied().collect()
}

pub fn doubled(b: [u8; 3]) -> Vec<u8> {
    b.iter().map(|x| x * 2).collect()
}

pub fn middle(b: [u8; 4]) -> usize {
    let s = &b[1..3];
    s.len() + b.as_slice().len() + (&b[..]).len()
}

pub fn same(a: [u8; 2], b: [u8; 2]) -> bool {
    a == b
}

pub fn order(a: [u8; 2], b: [u8; 2]) -> Ordering {
    a.cmp(&b)
}

pub fn words_less(a: [String; 2], b: [String; 2]) -> bool {
    a < b
}

/// A local of its own, written in place.
pub fn written(x: u8) -> [u8; 4] {
    let mut a = [0u8; 4];
    a[1] = x;
    a[3] = x / 2;
    a
}

/// A write past the end panics as Rust's does.
pub fn write_at(i: usize) -> [u8; 3] {
    let mut a = [1u8, 2, 3];
    a[i] = 7;
    a
}

/// Arrays are `Copy`: the copy keeps what it had.
pub fn copied_then_written(b: [u8; 3]) -> ([u8; 3], [u8; 3], [u8; 3]) {
    let mut a = b;
    let c = a;
    a[0] = 9;
    (a, c, b)
}

pub fn packet_sum(p: &Packet) -> u32 {
    let mut t = u32::from(p.tag[0]) + u32::from(p.tag[1]);
    for row in &p.body {
        for x in row {
            t += u32::from(*x);
        }
    }
    match p.note {
        Some([c, _]) => t + u32::from(c),
        None => t,
    }
}

pub fn frame_len(f: Frame) -> usize {
    match f {
        Frame::Short(b) => b.len(),
        Frame::Long { bytes, more } => bytes.len() + if more { 1 } else { 0 },
        Frame::Pair(a, b) => a.len() * 10 + b.len(),
        Frame::Empty => 0,
    }
}

pub fn maybe(o: Option<[u8; 2]>) -> u8 {
    match o {
        Some([a, _]) => a,
        None => 0,
    }
}

pub fn grid(g: [[u8; 2]; 2]) -> u8 {
    g[1][0] + g[0][1]
}

pub fn rows(g: Vec<[u8; 2]>) -> usize {
    g.len()
}

pub fn digit(i: usize) -> char {
    DIGITS[i]
}

pub fn square(i: usize) -> u32 {
    SQUARES[i] + u32::from(MAGIC[i])
}

pub fn magic() -> [u8; 3] {
    *b"abc"
}

/// A tuple of arrays, matched element by element.
pub fn pair_code(a: [u8; 2], b: [u8; 2]) -> u32 {
    match (a, b) {
        ([0, x], [y, 0]) => u32::from(x) + u32::from(y),
        ([x, _], [_, y]) => u32::from(x) * u32::from(y),
    }
}

/// A local bound to a literal is rustc's array, matched by its length.
pub fn local_literal(x: u8) -> u8 {
    let bytes = [x, 2, 3, 4];
    let [a, .., d] = bytes;
    a.wrapping_add(d)
}

/// Built in a loop and read whole inside it, where it is written after.
pub fn running(b: [u8; 3]) -> Vec<[u8; 3]> {
    let mut a = [0u8; 3];
    let mut seen: Vec<[u8; 3]> = Vec::new();
    for i in 0..b.len() {
        a[i] = b[i];
        seen.push(a);
    }
    seen
}

pub fn stages(b: [u32; 4]) -> (Vec<u32>, u32, Option<u32>, usize) {
    let rev: Vec<u32> = b.iter().rev().copied().collect();
    let mut pairs = 0u32;
    for (i, x) in b.iter().enumerate() {
        pairs += x * (i as u32);
    }
    let windows = b.windows(2).filter(|w| w[0] < w[1]).count();
    (rev, pairs, b.iter().copied().max(), windows)
}

pub fn zipped(a: [u8; 3], b: [u8; 2]) -> Vec<(u8, u8)> {
    a.iter().copied().zip(b.iter().copied()).collect()
}

/// Ordered element by element: a `Vec` of arrays sorts as Rust's does.
pub fn sorted(rows: Vec<[u8; 2]>) -> Vec<[u8; 2]> {
    let mut v = rows;
    v.sort();
    v
}

fn largest(xs: &[u32]) -> u32 {
    let mut m = 0;
    for x in xs {
        if *x > m {
            m = *x;
        }
    }
    m
}

/// An array where a slice goes.
pub fn through_slice(b: [u32; 3]) -> u32 {
    largest(&b) + largest(&b[1..])
}

pub fn words(w: [String; 2]) -> String {
    let [a, b] = w;
    format!("{b}-{a}")
}

pub fn blocks(x: [u8; 2]) -> [[u8; 2]; 3] {
    [x; 3]
}

/// `for x in a` walks a copy: a write in the body shows in no later `x`.
pub fn walked_copy(b: [u8; 3]) -> Vec<u8> {
    let mut a = b;
    let mut out: Vec<u8> = Vec::new();
    for x in a {
        a[2] = x;
        out.push(x);
    }
    out.push(a[2]);
    out
}

fn first_of<T: Clone>(xs: &[T]) -> Option<T> {
    xs.first().cloned()
}

/// An array where a generic slice goes.
pub fn generic_slice(a: [u8; 3]) -> Option<u8> {
    first_of(&a)
}

pub fn mut_let(arr: [u8; 3]) -> u8 {
    let [mut a, b, _] = arr;
    a = a.wrapping_add(b);
    a
}

/// Variants listed, or repeated, with no type for TS to read them as: the
/// list is typed, so a `kind` stays the enum's and other variants go in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    A,
    B,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Empty,
    Full(u8),
}

pub fn picks(k: Pick) -> Vec<Pick> {
    let mut out: Vec<Pick> = Vec::new();
    for p in [Pick::A, k] {
        match p {
            Pick::A | Pick::B => out.push(p),
            Pick::C => out.push(Pick::A),
        }
    }
    out
}

pub fn slots(i: usize) -> [Slot; 4] {
    let mut a: [Slot; 4] = [Slot::Empty; 4];
    a[i] = Slot::Full(1);
    a
}

pub fn slots_from_local(i: usize) -> [Slot; 3] {
    let e: Slot = Slot::Empty;
    let mut a = [e; 3];
    a[i] = Slot::Full(2);
    a
}

/// The enum named nowhere else in the function: its type is imported for
/// the list's annotation.
pub fn tally() -> u8 {
    let mut n = 0u8;
    for p in [Pick::A, Pick::B, Pick::A] {
        if p == Pick::A {
            n += 1;
        }
    }
    n
}

pub fn empties(i: usize) -> usize {
    let mut a = [Slot::Empty; 3];
    a[i] = Slot::Full(1);
    a.iter().filter(|s| **s == Slot::Empty).count()
}
