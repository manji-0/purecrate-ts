// Unsuffixed literals typed by a later use: a `let` of a `match` or an `if`
// whose results are literals, and an array of literals a `for` walks.

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
