// Crate consts of the crate's open structs (design/01 §7.29): a plain
// struct, one nested in another, structs in a tuple, in a variant's
// payload, and in arrays, a struct holding an array, and another const's
// name as a field. Each is a typed `export const` of its folded value,
// its fields in the declared order, read as any value of its type is.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub row: i8,
    pub col: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pawn,
    Knight,
    Rook,
}

/// Declared `kind, at, steps`; the literals below name them in other
/// orders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    pub kind: Kind,
    pub at: Point,
    pub steps: [(i8, i8); 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub text: &'static str,
    pub mark: char,
    pub weight: f64,
    pub shown: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Square {
    Empty,
    Held(Piece),
    Marked { at: Point, by: Kind },
}

pub const ORIGIN: Point = Point { row: 0, col: 0 };
pub const CORNER: Point = Point { col: 7, row: -8 };
pub const PAWN: Piece = Piece { steps: [(1, 0), (2, 0)], kind: Kind::Pawn, at: ORIGIN };
pub const PIECES: [Piece; 3] = [
    PAWN,
    Piece { at: CORNER, kind: Kind::Rook, steps: [(0, 1); 2] },
    Piece { kind: Kind::Knight, at: Point { row: 1, col: -2 }, steps: [(2, 1), (-1, -2)] },
];
pub const GRID: [[Point; 2]; 2] = [[ORIGIN, CORNER], [Point { row: 3, col: 4 }; 2]];
pub const LABELS: [Label; 2] = [
    Label { text: "é", mark: '😀', weight: -0.5, shown: true },
    Label { shown: false, weight: 1e300, mark: 'a', text: "" },
];
pub const START: (Point, Kind, u8) = (CORNER, Kind::Rook, 2);
pub const BOARD: [Square; 3] = [
    Square::Held(PAWN),
    Square::Empty,
    Square::Marked { by: Kind::Knight, at: Point { row: 5, col: 5 } },
];

fn on_board(p: Point) -> bool {
    p.row >= 0 && p.row < 8 && p.col >= 0 && p.col < 8
}

pub fn origin() -> Point {
    ORIGIN
}

/// Field reads, and the const where a `Point` goes.
pub fn corner_sum() -> i32 {
    let row = i32::from(CORNER.row);
    let col = i32::from(CORNER.col);
    row * 10 + col + if on_board(CORNER) { 100 } else { 0 }
}

/// Indexing, in range and past the end, then a field of the element.
pub fn piece_at(i: usize) -> Piece {
    PIECES[i]
}

pub fn piece_kind(i: usize) -> Kind {
    PIECES[i].kind
}

/// Each piece's steps from where it stands, walked by reference, and
/// every piece by value.
pub fn reach(i: usize) -> Vec<Point> {
    let at = PIECES[i].at;
    let mut out: Vec<Point> = Vec::new();
    for &(dr, dc) in PIECES[i].steps.iter() {
        let p = Point { row: at.row + dr, col: at.col + dc };
        if on_board(p) {
            out.push(p);
        }
    }
    for p in PIECES {
        out.push(Point { row: p.at.row + 1, col: p.at.col });
    }
    out
}

/// A `match` on an element's fields.
pub fn describe(i: usize, j: usize) -> String {
    let p = GRID[i][j];
    match (p.row, p.col) {
        (0, 0) => String::from("origin"),
        (row, col) if row < 0 => format!("below {col}"),
        (row, col) => format!("{row},{col}"),
    }
}

/// `==` through the derived `eq`, and `contains`.
pub fn is_corner(p: Point) -> bool {
    p == CORNER || GRID[1].contains(&p)
}

pub fn same_piece(p: Piece) -> bool {
    PIECES.contains(&p) && p != PIECES[1]
}

pub fn label_at(i: usize) -> (String, f64, bool) {
    let l = LABELS[i].clone();
    (format!("{}{}", l.mark, l.text), l.weight, l.shown)
}

pub fn start() -> (Point, Kind, u8) {
    START
}

fn letter(k: Kind) -> char {
    match k {
        Kind::Pawn => 'p',
        Kind::Knight => 'n',
        Kind::Rook => 'r',
    }
}

/// A struct in a variant's payload, matched.
pub fn show_square(i: usize) -> String {
    match BOARD[i] {
        Square::Empty => String::from("."),
        Square::Held(p) if p.kind == Kind::Pawn => format!("pawn@{},{}", p.at.row, p.at.col),
        Square::Held(p) => format!("{}", letter(p.kind)),
        Square::Marked { at, by } => format!("{}@{},{}", letter(by), at.row, at.col),
    }
}

pub fn board(i: usize) -> Square {
    BOARD[i]
}

/// A copy of a const's array, written, leaves the const as it was.
pub fn moved(dr: i8) -> (Piece, Piece) {
    let mut steps = PAWN.steps;
    steps[0] = (dr, dr);
    let p = Piece { steps, ..PAWN };
    (p, PAWN)
}

pub fn all() -> ([Point; 2], Piece, (Point, Kind, u8)) {
    (GRID[0], PAWN, START)
}
