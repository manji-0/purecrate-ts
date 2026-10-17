// Unsuffixed literals typed by a later use: a `let` of a `match` or an `if`
// whose results are literals or tuples of them, and an array of literals a
// `for` walks; `i32`, rustc's fallback, where no use types one.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Piece {
    Pawn,
    Knight,
    King,
}

// The arms take `u8` from the sum, so `200 + x` overflows as `u8`.
pub fn weight(p: Piece, x: u8) -> u8 {
    let w = match p {
        Piece::Pawn => 1,
        Piece::Knight => 3,
        Piece::King => 200,
    };
    w + x
}

// A diverging arm, and a negative literal typed `i8` by the parameter.
pub fn step(p: Piece, by: i8) -> i8 {
    let d = match p {
        Piece::Pawn => -1,
        Piece::Knight => 2,
        Piece::King => return 0,
    };
    d * by
}

// Both sides of an `if`, typed `i64` by the return.
pub fn pick(c: bool, base: i64) -> i64 {
    let n = if c { 9_000_000_000 } else { -7 };
    base - n
}

// A `panic!` in an arm; the value is a `u32`.
pub fn rank(n: u32) -> u32 {
    let r = match n {
        0 => 10,
        1..=9 => 20,
        _ => panic!("rank out of range"),
    };
    r * n
}

// Indexes: a `usize` from `v[i]`, walked in the array's order.
pub fn gather(v: &[u16]) -> u32 {
    let mut s: u32 = 0;
    for i in [2, 0, 1] {
        s = s * 100 + u32::from(v[i]);
    }
    s
}

// Offsets of `i16`, from the accumulator they are added to.
pub fn around(at: i16) -> i16 {
    let mut t = at;
    for d in [-1, 1, -300] {
        t += d;
    }
    t
}

// No use types the counter: `i32`, so it overflows past `i32::MAX`.
pub fn count_from(start: bool, xs: &[u8]) -> bool {
    let mut n = if start { 2_147_483_640 } else { 0 };
    for x in xs {
        if *x > 3 {
            n += 1;
        }
    }
    n > 5
}

// A `for` over a range and an array no use types, as text.
pub fn listed(k: u8) -> String {
    let mut s = String::new();
    for i in 0..3 {
        s.push_str(&(i * 1_000_000_000).to_string());
    }
    for d in [-2, 7] {
        if k > 9 {
            s.push_str(&d.to_string());
        }
    }
    s
}

// A tuple of literals: `dr` an `i64` from the product, `start` a `u8`
// from the sum, `last` an `i32` (`i64::from` asks none), `on` a `bool`.
pub fn ranks(white: bool, x: u8) -> i64 {
    let (dr, start, last, on) = match white {
        true => (1, 1, 7, true),
        false => (-1, 250, -8, false),
    };
    let r: i64 = dr * 3;
    match on {
        true => r + i64::from(start + x) - i64::from(last),
        false => r - i64::from(last * 300_000_000),
    }
}

// A tuple whose places a `_` and a diverging arm leave.
pub fn base(c: u8) -> u16 {
    let (b, _, k) = match c {
        0 => (0, 9, 1),
        1 => (56, -3, 2),
        _ => return 7,
    };
    b * 1000 + k
}
