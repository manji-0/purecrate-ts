// `==` and `!=` on the crate's enums as a test of `kind`: against a unit
// variant written out on either side, between two values of an enum whose
// variants are all unit, through references, on fields, elements, and
// calls (each evaluated once, left first), and where a `match` or an `if`
// has narrowed one side or both. A payload, an enum of one variant, an
// `Option`, and a generic `T` still compare through `Eq.deep`.

use std::cmp::Ordering;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    North,
    East,
    South,
    West,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tile {
    Empty,
    Wall,
    Coin(u8),
    Door { open: bool },
}

#[derive(Clone, Copy, PartialEq)]
pub enum Only {
    One,
}

#[derive(Clone, PartialEq)]
pub enum Slot<T> {
    Free,
    Taken(T),
}

#[derive(Clone, PartialEq)]
pub struct Piece {
    pub kind: Dir,
    pub at: Tile,
}

pub fn is_east(d: Dir) -> bool {
    d == Dir::East
}

pub fn not_east_left(d: Dir) -> bool {
    Dir::East != d
}

pub fn same(a: Dir, b: Dir) -> bool {
    a == b
}

pub fn differ(a: Dir, b: Dir) -> bool {
    a != b
}

pub fn through_refs(a: &Dir, b: &Dir) -> (bool, bool, bool, bool) {
    (a == b, *a == Dir::South, &Dir::West == a, *a != *b)
}

pub fn home() -> Dir {
    Dir::North
}

pub fn at_home(d: Dir) -> bool {
    d == home()
}

pub fn literals() -> (bool, bool) {
    (Dir::North == Dir::North, Dir::North != Dir::East)
}

pub fn tile_tests(c: Tile, n: u8) -> (bool, bool, bool, bool, bool) {
    (c == Tile::Empty, Tile::Wall != c, c == Tile::Coin(n), c == Tile::Door { open: true }, c != Tile::Coin(3))
}

pub fn tiles_same(a: Tile, b: Tile) -> bool {
    a == b
}

pub fn only_tests(a: Only, b: Only) -> (bool, bool) {
    (a == Only::One, a == b)
}

pub fn slot_tests(s: Slot<u8>, t: Slot<u8>) -> (bool, bool, bool) {
    (s == Slot::Free, Slot::Taken(2u8) == s, s == t)
}

pub fn option_test(d: Option<Dir>) -> bool {
    d == Some(Dir::East)
}

pub fn generic_eq<T: PartialEq>(a: T, b: T) -> bool {
    a == b
}

pub fn generic_dirs(a: Dir, b: Dir) -> bool {
    generic_eq(a, b)
}

pub fn piece_tests(p: &Piece, q: &Piece) -> (bool, bool, bool, bool) {
    (p.kind == Dir::West, p.at == Tile::Wall, p.kind == q.kind, p.at == q.at)
}

pub fn board_tests(cells: &[Tile], dirs: &[Dir], i: usize) -> (bool, bool, bool) {
    (cells[i] == Tile::Empty, dirs[i] != Dir::North, dirs[i] == dirs[0])
}

fn turn(d: Dir, by: u8) -> Dir {
    if by > 7 {
        panic!("turn {by}");
    }
    let order = [Dir::North, Dir::East, Dir::South, Dir::West];
    let i = order.iter().position(|x| *x == d).unwrap_or(0);
    order[(i + usize::from(by)) % 4]
}

fn tile_at(n: u8) -> Tile {
    if n > 9 {
        panic!("tile {n}");
    }
    match n % 3 {
        0 => Tile::Empty,
        1 => Tile::Coin(n),
        _ => Tile::Wall,
    }
}

// Each side runs once, the left first: `turn` panics with its argument.
pub fn calls(d: Dir, x: u8, y: u8) -> (bool, bool, bool) {
    (turn(d, x) == Dir::South, Dir::East != turn(d, y), turn(d, x) == turn(d, y))
}

pub fn tile_calls(x: u8) -> (bool, bool) {
    (tile_at(x) == Tile::Wall, tile_at(x) != Tile::Empty)
}

// The arms narrow `d`: each test is decided there.
pub fn narrowed(d: Dir) -> u8 {
    match d {
        Dir::North => {
            if d == Dir::North { 1 } else { 2 }
        }
        Dir::East | Dir::South => {
            if d == Dir::North {
                3
            } else if d != Dir::South {
                4
            } else {
                5
            }
        }
        Dir::West => 6,
    }
}

// Both sides narrowed: decided where they are apart or one variant, and
// through `Eq.deep` where only one side is (TS would narrow the other).
pub fn both_narrowed(a: Dir, b: Dir) -> u8 {
    match (a, b) {
        (Dir::North, Dir::East) => {
            if a == b { 1 } else { 2 }
        }
        (Dir::North, Dir::North) => {
            if a != b { 3 } else { 4 }
        }
        (Dir::East, _) => {
            if a == b {
                5
            } else if b == Dir::East {
                6
            } else {
                7
            }
        }
        _ => {
            if a == b {
                8
            } else if a == Dir::South && b != Dir::South {
                9
            } else {
                10
            }
        }
    }
}

pub fn after_return(a: Dir, b: Dir) -> u8 {
    if a == Dir::North {
        return 0;
    }
    if a == Dir::North || a == b {
        return 1;
    }
    if b == Dir::North && a != Dir::East {
        return 2;
    }
    3
}

pub fn tile_narrowed(c: Tile) -> u8 {
    match c {
        Tile::Coin(n) => {
            if c == Tile::Empty {
                0
            } else {
                n
            }
        }
        _ => {
            if c == Tile::Wall {
                100
            } else if c != Tile::Empty {
                101
            } else {
                102
            }
        }
    }
}

pub fn runs(dirs: &[Dir]) -> u32 {
    let mut n = 0;
    let mut last = Dir::North;
    for d in dirs {
        if *d == last {
            n += 1;
        }
        if *d != Dir::West {
            last = *d;
        }
    }
    n
}

pub fn count_free(cells: &[Tile]) -> usize {
    cells.iter().filter(|c| **c == Tile::Empty).count()
}

pub fn ordering_narrowed(a: u8, b: u8) -> u8 {
    let x = a.cmp(&b);
    let y = b.cmp(&a);
    match x {
        Ordering::Less => {
            if x == y {
                1
            } else if y == Ordering::Greater {
                2
            } else {
                3
            }
        }
        _ => {
            if x == y { 4 } else { 5 }
        }
    }
}
