// The derived `eq` printed per type: structs with float fields (NaN unequal
// to itself, `-0.0` equal to `0.0`), nested crate types, a recursive tree
// through `Box`, generic types, `Option` and `Vec` fields, a field of `()`,
// a struct with no field, a type reached only as another's field, a type
// with its own method `eq` (which keeps `Eq.deep`), and `matches!` on an
// enum of one variant read from a call.

#[derive(Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Blue,
}

#[derive(Clone, PartialEq)]
pub struct Pen {
    pub color: Color,
    pub width: u8,
    pub at: Point,
}

#[derive(Clone, PartialEq)]
pub struct Stroke {
    pub pen: Pen,
    pub path: Vec<Point>,
    pub label: Option<String>,
    pub big: i64,
    pub wide: u128,
    pub done: (),
}

#[derive(Clone, PartialEq)]
pub enum Tree {
    Leaf,
    Node(Box<Tree>, i32, Box<Tree>),
    Named { name: String, child: Option<Box<Tree>> },
}

#[derive(Clone, PartialEq)]
pub struct Pair<T> {
    pub left: T,
    pub right: T,
    pub n: u32,
}

#[derive(Clone, PartialEq)]
pub enum Either<A, B> {
    Left(A),
    Right(B),
    Neither,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Nothing {}

#[derive(Clone, Copy, PartialEq)]
pub struct Cents(pub i64);

#[derive(Clone, PartialEq)]
pub struct Money {
    pub cents: i64,
    pub currency: String,
}

impl Money {
    /// Its own `eq`, by currency alone; `==` is still the derived one.
    pub fn eq(&self, other: &Money) -> bool {
        self.currency == other.currency
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Only {
    One,
}

pub fn points_eq(a: Point, b: Point) -> (bool, bool) {
    (a == b, a != b)
}

pub fn strokes_eq(a: &Stroke, b: &Stroke) -> bool {
    a == b
}

pub fn trees_eq(a: &Tree, b: &Tree) -> (bool, bool) {
    (a == b, *a == Tree::Leaf)
}

pub fn pairs_eq(a: Pair<Point>, b: Pair<Point>) -> bool {
    a == b
}

pub fn eithers_eq(a: Either<u8, String>, b: Either<u8, String>) -> (bool, bool) {
    (a == b, a == Either::Left(3))
}

pub fn nothing_eq(a: Nothing, b: Nothing) -> bool {
    a == b
}

pub fn cents_eq(a: Cents, b: Cents) -> bool {
    a == b
}

pub fn money_eq(a: &Money, b: &Money) -> (bool, bool) {
    (a == b, a.eq(b))
}

pub fn pens_in(pens: &[Pen], p: &Pen) -> (bool, Option<usize>) {
    (pens.contains(p), pens.iter().position(|q| q == p))
}

fn pen_at(n: u8) -> Pen {
    if n > 9 {
        panic!("pen {n}");
    }
    Pen { color: if n % 2 == 0 { Color::Red } else { Color::Blue }, width: n, at: Point { x: 0.0, y: 0.0 } }
}

// Each side once, the left first: `pen_at` panics with its argument.
pub fn pen_calls(x: u8, y: u8) -> bool {
    pen_at(x) == pen_at(y)
}

fn only_at(n: u8) -> Only {
    if n > 9 {
        panic!("only {n}");
    }
    Only::One
}

pub fn only_calls(n: u8) -> (bool, bool) {
    (matches!(only_at(n), Only::One), !matches!(only_at(n), Only::One) || n == 3)
}
