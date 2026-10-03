// Rewrites that only change how the output reads: a negated test flipped,
// same-named fields destructured, an arm that returns the variant it
// matched, and a variant built again from all its own fields.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Less,
    Equal,
    Greater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Rect { w: i32, h: i32 },
    Pair(i32, i32),
    Empty,
}

pub fn flipped(c: bool, a: i32) -> i32 {
    if !c { a } else { 0 }
}

pub fn area(s: Shape) -> i32 {
    match s {
        Shape::Rect { w, h } => w * h,
        Shape::Pair(a, b) => a + b,
        Shape::Empty => 0,
    }
}

fn tie(a: i32, b: i32) -> Dir {
    if a < b { Dir::Less } else if a > b { Dir::Greater } else { Dir::Equal }
}

pub fn then_by(d: Dir, a: i32, b: i32) -> Dir {
    match d {
        Dir::Less => Dir::Less,
        Dir::Equal => tie(a, b),
        Dir::Greater => Dir::Greater,
    }
}

fn width(s: Shape) -> i32 {
    match s {
        Shape::Rect { w, .. } => w,
        Shape::Pair(a, _) => a,
        Shape::Empty => 0,
    }
}

pub fn same_again(s: Shape) -> i32 {
    match s {
        Shape::Pair(a, b) => {
            let t = Shape::Pair(a, b);
            width(t) + 1
        }
        Shape::Rect { w, h } => {
            let t = Shape::Rect { h, w };
            width(t)
        }
        Shape::Empty => 0,
    }
}

/// Fields swapped: a different value, not the place.
pub fn swapped(s: Shape) -> i32 {
    match s {
        Shape::Pair(a, b) => width(Shape::Pair(b, a)),
        _ => 0,
    }
}
