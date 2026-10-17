// `==` through an `Option`, a `Vec`, a slice, an array, a tuple, and a
// `BTreeMap` by each part's own type: `===` on an `Option` of a primitive
// (floats at NaN and `-0.0`, `i64`, `u128`, `char`, strings, `bool`), with
// `Some(x)` and `None` written on either side; a crate type's `eq`, an
// all-unit enum's variant, and `Ordering` inside them, nested; a `T` and a
// `Result`, which stay `Eq.deep`; a generic type's value written out; what
// a narrowed `Option` compares to; and calls that panic (each side once,
// the left first).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    White,
    Black,
}

#[derive(Clone, PartialEq)]
pub enum Shape {
    Dot(Point),
    Line { from: Point, to: Point },
    Path(Vec<Point>),
    Tagged(Option<String>, Vec<Option<Point>>),
}

#[derive(Clone, PartialEq)]
pub struct Slot<T> {
    pub value: T,
    pub n: u8,
}

pub fn options_u8(a: Option<u8>, b: Option<u8>, x: u8) -> ((bool, bool, bool), (bool, bool, bool)) {
    ((a == b, a != b, a == Some(x)), (Some(x) != a, a == None, None != a))
}

pub fn options_f64(a: Option<f64>, b: Option<f64>) -> (bool, bool, bool) {
    (a == b, a != b, a == Some(0.0))
}

pub fn options_wide(a: Option<i64>, b: Option<u128>, c: Option<char>) -> (bool, bool, bool, bool) {
    (a == Some(-1), b == Some(!0), c == Some('é'), Some(3u8) == Some(3u8))
}

pub fn options_text(a: Option<String>, b: Option<String>, c: Option<&str>) -> (bool, bool, bool) {
    (a == b, c == Some("x"), a.as_deref() == c)
}

pub fn options_bool(a: Option<bool>) -> (bool, bool, bool) {
    (a == Some(true), a == Some(false), a != None)
}

pub fn options_of_types(a: Option<Point>, b: Option<Point>, c: Option<Color>, d: Option<Color>) -> (bool, bool, bool) {
    (a == b, c == d, c == Some(Color::Black))
}

pub fn options_of_ordering(a: u8, b: u8, c: Option<Ordering>) -> bool {
    Some(a.cmp(&b)) == c
}

pub fn lists(a: &[Point], b: Vec<Point>) -> (bool, bool, bool) {
    (a == b.as_slice(), b != a, a[..a.len().min(1)] == b[..b.len().min(1)])
}

pub fn nested(a: &Vec<Vec<Option<Point>>>, b: &Vec<Vec<Option<Point>>>) -> bool {
    a == b
}

pub fn floats(a: Vec<f64>, b: Vec<f64>, c: [f32; 2], d: [f32; 2]) -> (bool, bool) {
    (a == b, c == d)
}

pub fn texts(a: &[String], b: &[String]) -> bool {
    a == b
}

pub fn colors(a: [Color; 3], b: [Color; 3]) -> (bool, bool) {
    (a == b, a == [Color::White; 3])
}

pub fn tuples(a: (u8, Point), b: (u8, Point), x: u8) -> (bool, bool, bool) {
    (a == b, a != b, (a.0, Color::White) == (x, Color::White))
}

pub fn tuple_parts(a: (f64, Option<Point>, Vec<Color>), b: (f64, Option<Point>, Vec<Color>)) -> bool {
    a == b
}

pub fn shapes(a: &Shape, b: &Shape) -> (bool, bool) {
    (a == b, Some(a) == Some(b))
}

pub fn maps(a: &BTreeMap<String, Point>, b: &BTreeMap<String, Point>) -> (bool, bool) {
    (a == b, a != b)
}

pub fn map_lists(a: &BTreeMap<u32, Vec<f64>>, b: &BTreeMap<u32, Vec<f64>>) -> bool {
    a == b
}

pub fn sets(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    a == b
}

pub fn results(a: Result<Point, String>, b: Result<Point, String>) -> bool {
    a == b
}

fn same_list<T: PartialEq>(a: &[T], b: &[T]) -> bool {
    a == b
}

fn same_pair<T: PartialEq>(a: (T, u8), b: (T, u8)) -> bool {
    a == b
}

pub fn generic(a: Vec<Point>, b: Vec<Point>, x: u8) -> (bool, bool) {
    (same_list(&a, &b), same_pair((a, x), (b, 1)))
}

pub fn slots(a: Option<Slot<u8>>, b: Option<Slot<u8>>) -> (bool, bool) {
    (a == b, a == Some(Slot { value: 2, n: 1 }))
}

// What TS narrows by `===` is what the printer knows.
pub fn narrowed(a: Option<u8>, b: Option<u8>) -> u8 {
    if a == Some(3) {
        return match a {
            Some(v) => v,
            None => 0,
        };
    }
    if a.is_none() {
        if a == b {
            return 10;
        }
        return match b {
            Some(v) => v,
            None => 11,
        };
    }
    if let Some(v) = b {
        if a == b {
            return v + 20;
        }
        return match a {
            Some(w) => w,
            None => 21,
        };
    }
    if a != None {
        return 30;
    }
    40
}

fn opt_at(n: u8) -> Option<u8> {
    if n > 9 {
        panic!("opt {n}");
    }
    if n == 0 {
        None
    } else {
        Some(n)
    }
}

fn points_at(n: u8) -> Vec<Point> {
    if n > 9 {
        panic!("points {n}");
    }
    let mut out: Vec<Point> = Vec::new();
    for _ in 0..n % 3 {
        out.push(Point { x: f64::from(n), y: 0 });
    }
    out
}

// Each side once, the left first.
pub fn calls(x: u8, y: u8) -> (bool, bool, bool) {
    (opt_at(x) == opt_at(y), opt_at(x) == Some(y), points_at(x) == points_at(y))
}

// `o === v` would narrow `o` by `v`, whose type TS infers from `o` through
// the loop: "implicitly has type 'any'". `Eq.strict` narrows nothing.
pub fn looped(o: Option<u8>, n: u8) -> Option<u8> {
    let mut h = 0u8;
    if o.is_none() {
        return None;
    }
    for _ in 0..n {
        let v = o?;
        if o == Some(v) {
            h = h.wrapping_add(v);
        }
    }
    Some(h)
}
