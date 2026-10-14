// `if let` and `let`-`else` on an enum's variant (tuple, struct, unit,
// nested in `Some`), on literals and ranges, chained `else if let`, with
// `mut` bindings and several names; and a tuple's `.0`, `.1`, nested.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Circle(u32),
    Rect { w: u32, h: u32 },
    Pair(u8, u8),
    Empty,
}

pub fn radius(s: Shape) -> u32 {
    if let Shape::Circle(r) = s {
        r
    } else {
        0
    }
}

/// A chain of `else if let`, its arms overflowing on large input.
pub fn area(s: Shape) -> u32 {
    if let Shape::Rect { w, h } = s {
        w * h
    } else if let Shape::Circle(r) = s {
        3 * r * r
    } else if let Shape::Pair(a, b) = s {
        u32::from(a) * u32::from(b)
    } else {
        1
    }
}

pub fn is_empty(s: Shape) -> bool {
    if let Shape::Empty = s {
        return true;
    }
    false
}

/// A variant inside `Some`: the `Option` prints as its payload or `null`,
/// and the payload is then tested for its variant.
pub fn inner(o: Option<Shape>) -> u32 {
    if let Some(Shape::Pair(a, _)) = o {
        u32::from(a) + 1
    } else {
        0
    }
}

pub fn inner_match(o: Option<Shape>) -> u32 {
    match o {
        Some(Shape::Circle(r)) => r,
        _ => 7,
    }
}

/// Literals and ranges.
pub fn small(n: u8) -> u8 {
    if let 0..=9 = n {
        n + 100
    } else if let 255 = n {
        1
    } else {
        n / 2
    }
}

pub fn first_char(s: &str) -> u32 {
    if let "" = s {
        return 0;
    }
    let mut n = 0u32;
    for c in s.chars() {
        if let 'a'..='z' = c {
            n += 1;
        }
    }
    n
}

/// `let`-`else` returning on a variant, then using its names.
pub fn dims(s: Shape) -> u32 {
    let Shape::Rect { w, h } = s else { return 0 };
    w + h
}

/// Several names, one `mut`.
pub fn pair_sum(s: Shape) -> u32 {
    let Shape::Pair(a, mut b) = s else {
        return 1000;
    };
    b = b / 2;
    u32::from(a) + u32::from(b)
}

/// `let`-`else` binding nothing.
pub fn only_empty(s: Shape) -> u32 {
    let Shape::Empty = s else { return 1 };
    2
}

/// `let`-`else` on `Option` and `Result`, a tuple bound whole.
pub fn parse_pair(s: &str) -> Result<u32, u32> {
    let Some(p) = s.split_once(',') else {
        return Err(0);
    };
    let Ok(x) = p.0.parse::<u32>() else {
        return Err(1);
    };
    let Ok(mut y) = p.1.parse::<u32>() else {
        return Err(2);
    };
    y += 1;
    Ok(x + y)
}

/// `let`-`else` with `continue` and `break` in a loop.
pub fn sum_circles(xs: Vec<Shape>) -> u32 {
    let mut n = 0u32;
    for x in xs {
        let Shape::Circle(r) = x else { continue };
        if r == 0 {
            break;
        }
        n += r;
    }
    n
}

pub fn first_rect(xs: Vec<Shape>) -> u32 {
    let mut i = 0usize;
    while i < xs.len() {
        let Shape::Rect { w, .. } = xs[i] else {
            i += 1;
            continue;
        };
        return w;
    }
    99
}

/// `let`-`else` whose `else` panics on overflow before it returns.
pub fn checked(a: u8, s: Shape) -> u8 {
    let Shape::Pair(x, _) = s else {
        return a + 1;
    };
    x
}

/// A tuple's elements, nested, and a tuple held in a `Vec`.
pub fn widest(xs: Vec<u8>) -> (u8, usize) {
    let mut best = (0u8, 0usize);
    for i in 0..xs.len() {
        let c = (xs[i], i);
        if c.0 > best.0 {
            best = c;
        }
    }
    best
}

pub fn nested(a: u8, b: bool, s: &str) -> u32 {
    let t = (a, (b, s), s.len());
    let inner = t.1;
    if inner.0 {
        u32::from(t.0) + t.2 as u32
    } else {
        inner.1.len() as u32 * 10
    }
}

pub fn pairs(xs: Vec<(u8, u8)>) -> u32 {
    let mut n = 0u32;
    for i in 0..xs.len() {
        n += u32::from(xs[i].0) * u32::from(xs[i].1);
    }
    n
}
