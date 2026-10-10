// `==` and `!=` on values whose `PartialEq` is derived: structs, enums of
// every variant shape, a recursive tree, `Option`, `Vec`, tuples, and
// `Result`, with floats inside (NaN unequal to itself, `-0.0` equal to
// `0.0`).

#[derive(Clone, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

#[derive(Clone, PartialEq)]
pub enum Shape {
    Dot,
    Circle { at: Point, r: f64 },
    Poly(Vec<Point>, Option<String>),
}

#[derive(Clone, PartialEq)]
pub struct Tagged {
    pub id: u64,
    pub label: Option<String>,
    pub shapes: Vec<Shape>,
    pub unit: (),
}

pub fn points_eq(a: Point, b: Point) -> (bool, bool) {
    (a == b, a != b)
}

pub fn json_eq(a: Json, b: Json) -> (bool, bool) {
    (a == b, a != b)
}

pub fn shapes_eq(a: Shape, b: Shape) -> bool {
    a == b
}

pub fn tagged_eq(a: Tagged, b: Tagged) -> bool {
    a == b
}

pub fn options_eq(a: Option<Point>, b: Option<Point>) -> bool {
    a == b
}

pub fn lists_eq(a: Vec<Json>, b: Vec<Json>) -> bool {
    a == b
}

pub fn pairs_eq(a: (u8, Json), b: (u8, Json)) -> bool {
    a == b
}

pub fn results_eq(a: Result<Point, String>, b: Result<Point, String>) -> bool {
    a == b
}

/// RFC 6902 `test` compares two documents; this finds the index of the first
/// element equal to `x`.
pub fn position_of(xs: Vec<Json>, x: Json) -> Option<usize> {
    let mut i: usize = 0;
    while i < xs.len() {
        if xs[i] == x {
            return Some(i);
        }
        i += 1;
    }
    None
}
