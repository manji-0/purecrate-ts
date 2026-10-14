// Slice patterns on a `&[T]`, `xs.as_slice()`, and `&xs[..]`: fixed
// lengths, `..` at the front, the back, and between, `rest @ ..` binding a
// slice, literal, variant, `Some`, tuple, and struct elements, guards, a
// last `_`, a tuple of slices, `Some` of a slice, and `if let`, `let`-`else`,
// `matches!`, and `while let`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Circle(u32),
    Square(u32),
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub name: String,
    pub qty: u32,
}

/// The corpus's shape.
pub fn first(xs: &[u32]) -> u32 {
    match xs {
        [] => 0,
        [x, ..] => *x,
    }
}

pub fn last(xs: &[u32]) -> u32 {
    match xs {
        [.., x] => *x,
        [] => 0,
    }
}

/// Every length below the longest fixed pattern is a case of its own.
pub fn describe(xs: &[u32]) -> u32 {
    match xs {
        [] => 0,
        [x] => *x,
        [x, y] => x * 10 + y,
        [x, .., y] => x * 100 + y,
    }
}

/// The elements between, as a slice: its length and its sum, which
/// overflows only when it runs.
pub fn ends_and_middle(xs: &[u32]) -> (u32, usize, u32) {
    match xs {
        [first, rest @ .., last] => {
            let mut total: u32 = 0;
            for x in rest {
                total += x;
            }
            (first + last, rest.len(), total)
        }
        [x] => (*x, 0, 0),
        [] => (0, 0, 0),
    }
}

pub fn tail(xs: &[u32]) -> &[u32] {
    match xs {
        [_, rest @ ..] => rest,
        [] => xs,
    }
}

/// A guard that reads what `rest @ ..` binds.
pub fn long_tail(xs: &[u32]) -> u32 {
    match xs {
        [a, rest @ ..] if rest.len() > 1 => a + rest[0],
        [a, ..] => *a,
        [] => 0,
    }
}

pub fn init(xs: &[u32]) -> usize {
    match xs {
        [rest @ .., _] => rest.len(),
        [] => 0,
    }
}

/// Literals, a `..` at both ends of different arms, and a last `_`.
pub fn code(xs: &[u8]) -> u32 {
    match xs {
        [0] => 1,
        [0, ..] => 2,
        [.., 0] => 3,
        [1, 2, 3] => 4,
        [255, _, ..] => 5,
        [a, b, .., c, d] if a == d && b == c => 6,
        _ => 7,
    }
}

/// Guards tried in order, each only where its length matched.
pub fn guarded(xs: &[i32]) -> i32 {
    match xs {
        [x, ..] if *x > 10 => *x,
        [x, y, ..] if x + y > 10 => x + y,
        [.., y] if *y < 0 => -1,
        [_, ..] => 1,
        [] => 0,
    }
}

pub fn words(xs: &[&str]) -> u32 {
    match xs {
        ["é", ..] => 1,
        [_, "日本"] => 2,
        [a, rest @ ..] if a.len() > 2 => rest.len() as u32 + 10,
        [_, _, _, ..] => 3,
        _ => 0,
    }
}

pub fn chars(xs: &[char]) -> u32 {
    match xs {
        ['a'..='z', ..] => 1,
        [.., '😀'] => 2,
        _ => 3,
    }
}

/// Variant, `Some`, and tuple elements, tested further in.
pub fn shapes(xs: &[Shape]) -> u32 {
    match xs {
        [Shape::Circle(r)] => *r,
        [Shape::Square(s), Shape::Square(t)] => s + t,
        [Shape::Empty, .., Shape::Empty] => 1,
        [Shape::Circle(0), rest @ ..] => rest.len() as u32 + 100,
        [_, ..] => 2,
        [] => 3,
    }
}

pub fn options(xs: &[Option<u32>]) -> u32 {
    match xs {
        [Some(x), None] => *x,
        [None, Some(y)] => y + 1,
        [Some(x), Some(y), ..] => x + y,
        [None, None, ..] => 7,
        [Some(_)] => 8,
        [None] | [] => 9,
        [Some(_), None, _, ..] | [None, Some(_), _, ..] => 10,
    }
}

pub fn pairs(xs: &[(u32, bool)]) -> u32 {
    match xs {
        [(n, true), ..] => *n,
        [(_, false), (m, _)] => *m,
        _ => 0,
    }
}

/// A struct pattern in an element.
pub fn items(xs: &[Item]) -> u32 {
    match xs {
        [Item { qty: 0, .. }, ..] => 0,
        [Item { name, qty }, ..] if name.len() > 3 => *qty,
        [.., Item { qty, .. }] => qty + 1,
        [] => 2,
    }
}

/// `as_slice()` and `&xs[..]` on a `Vec`.
pub fn owned(xs: Vec<u32>) -> u32 {
    let a = match xs.as_slice() {
        [x, y, ..] => x + y,
        _ => 0,
    };
    let b = match &xs[..] {
        [.., x] => *x,
        [] => 0,
    };
    a + b
}

/// A tuple of slices: each element's length is tested in turn.
pub fn both(xs: &[u32], ys: &[u32]) -> u32 {
    match (xs, ys) {
        ([], []) => 0,
        ([x, ..], []) if *x > 5 => *x,
        ([], [y, ..]) if *y > 5 => *y,
        ([_, ..], []) | ([], [_, ..]) => 1,
        ([a, ..], [.., b]) => a + b,
    }
}

/// A slice pattern inside one, on a slice of slices.
pub fn nested(xss: &[&[u32]]) -> u32 {
    match xss {
        [[a, ..], .., [.., z]] => a + z,
        [[x], ..] => *x,
        [[], rest @ ..] => rest.len() as u32,
        _ => 0,
    }
}

/// `Some` of a slice, through `as_deref`.
pub fn maybe(o: Option<Vec<u32>>) -> u32 {
    match o.as_deref() {
        Some([x, ..]) => *x,
        Some([]) => 1,
        None => 2,
    }
}

pub fn pair_if_let(xs: &[u32]) -> u32 {
    if let [a, b] = xs {
        a * b
    } else {
        0
    }
}

pub fn ends_let_else(xs: &[u32]) -> u32 {
    let [first, .., last] = xs else {
        return 0;
    };
    first + last
}

/// `mut` on an element of a `let`-`else`.
pub fn bumped(xs: &[u32]) -> u32 {
    let [mut a, .., b] = xs else {
        return 0;
    };
    a += 1;
    a + b
}

pub fn starts_matches(xs: &[u32]) -> bool {
    matches!(xs, [1, ..] | [.., 1])
}

pub fn long_matches(xs: &[u32]) -> bool {
    matches!(xs, [_, _, _, ..])
}

pub fn guard_matches(xs: &[u32]) -> bool {
    matches!(xs, [x, .., y] if x < y)
}

/// `while let` walks the slice, rebinding it to its rest each pass.
pub fn walk(xs: &[u32]) -> u32 {
    let mut cur = xs;
    let mut total: u32 = 0;
    while let [x, rest @ ..] = cur {
        total += x;
        cur = rest;
    }
    total
}

/// Pairs from both ends, to the middle.
pub fn palindrome(xs: &[char]) -> bool {
    let mut cur = xs;
    loop {
        match cur {
            [] | [_] => return true,
            [a, rest @ .., b] => {
                if a != b {
                    return false;
                }
                cur = rest;
            }
        }
    }
}
