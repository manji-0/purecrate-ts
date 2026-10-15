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
