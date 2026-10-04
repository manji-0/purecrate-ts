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

pub fn bump(v: i32) -> i32 {
    v + 1
}

/// Printed in `bumped_twice.ts`, its one caller's file, which imports `bump`:
/// its parameter is renamed there, or it would hide that import.
fn doubled(bump: i32) -> i32 {
    bump * 2
}

pub fn bumped_twice(v: i32) -> i32 {
    doubled(v) + bump(v)
}

/// `matches!` of literals on a value that is not a place reads it once.
pub fn reserved_pair(s: &str, open: bool) -> bool {
    open || matches!(&s[0..2], "00" | "01" | "99")
}

pub fn halve(n: i32) -> Result<i32, i32> {
    if n % 2 == 0 {
        Ok(n / 2)
    } else {
        Err(n)
    }
}

/// Temporaries are named after the local they are for.
pub fn named_temporaries(n: i32) -> Result<i32, i32> {
    let half = halve(n).map_err(|e| e + 100)?;
    let sum = half.checked_add(1i32).ok_or(-1i32)?;
    Ok(sum)
}

/// A `?` inside what `unwrap_or` takes leaves from the statement, with no
/// block around the value.
pub fn added_or_zero(n: i32) -> Result<i32, i32> {
    let s = n.checked_add(halve(n)?).unwrap_or(0i32);
    Ok(s)
}

/// A guarded arm after a first one: `else if`, not `else { if .. }`.
pub fn banded(x: u64, w: u64) -> u64 {
    let y: u64 = match x {
        0 => 1,
        n if n > 10 => {
            let k = n * w;
            k
        }
        _ => 2,
    };
    y
}

/// A `match` of `true` / `false` as a test is the test.
pub fn above_ten(b: u32) -> u32 {
    if match b { a if a > 10 => true, _ => false } { 1 } else { 2 }
}
