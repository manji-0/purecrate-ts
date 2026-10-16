// Crate consts of tuples and of the crate's enums (design/01 §7.29): a
// table of directions, a list of variants, variants with payloads, tuples
// holding variants, arrays of arrays, and a plain tuple or variant. Each
// is a typed `export const` of its folded value, read as any value of its
// type is.

/// The eight moves of a knight, as a board keeps them.
pub const KNIGHT: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
pub const ROOK: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const BISHOP: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
/// Another const in the value is its value.
pub const LINES: [[(i8, i8); 4]; 2] = [ROOK, BISHOP];
pub const ORIGIN: (u8, u8) = (4, 0);
pub const NAMED: [(char, &str, f64); 2] = [('a', "é", -0.5), ('😀', "", 1e300)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pawn,
    Knight,
    Rook,
    Queen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Square {
    Empty,
    Piece(Kind, bool),
    Marked { piece: Kind, at: (u8, u8) },
}

pub const PROMOTIONS: [Kind; 3] = [Kind::Queen, Kind::Rook, Kind::Knight];
pub const DEFAULT: Kind = Kind::Pawn;
pub const BLANK: [Kind; 4] = [DEFAULT; 4];
pub const BACK: [Square; 4] = [
    Square::Piece(Kind::Rook, true),
    Square::Empty,
    Square::Marked { at: ORIGIN, piece: Kind::Queen },
    Square::Piece(DEFAULT, false),
];
pub const VALUES: [(Kind, u32); 3] = [(Kind::Pawn, 1), (Kind::Knight, 3), (Kind::Queen, 9)];
pub const START: (Kind, (u8, u8)) = (Kind::Rook, (0, 7));
pub const EDGES: [[Square; 2]; 2] = [[Square::Empty; 2], [Square::Piece(Kind::Pawn, true), Square::Empty]];

fn on_board(x: i8) -> bool {
    x >= 0 && x < 8
}

/// The knight's moves from `(r, c)` that stay on an 8 by 8 board, each
/// direction destructured in the `for`.
pub fn knight_moves(r: i8, c: i8) -> Vec<(i8, i8)> {
    let mut out: Vec<(i8, i8)> = Vec::new();
    for (dr, dc) in KNIGHT {
        let (nr, nc) = (r + dr, c + dc);
        if on_board(nr) && on_board(nc) {
            out.push((nr, nc));
        }
    }
    out
}

/// By reference, through `iter()`, and both line tables.
pub fn reach(r: i8, c: i8) -> u32 {
    let mut n = 0u32;
    for table in LINES.iter() {
        for &(dr, dc) in table.iter() {
            let (mut nr, mut nc) = (r + dr, c + dc);
            while on_board(nr) && on_board(nc) {
                n += 1;
                nr += dr;
                nc += dc;
            }
        }
    }
    n
}

/// Indexing, in range and past the end.
pub fn knight(i: usize) -> (i8, i8) {
    KNIGHT[i]
}

pub fn line(i: usize, j: usize) -> i8 {
    let (dr, dc) = LINES[i][j];
    dr * 2 + dc
}

pub fn lens() -> usize {
    KNIGHT.len() + LINES.len() + LINES[1].len() + PROMOTIONS.len()
}

pub fn is_knight_move(d: (i8, i8)) -> bool {
    KNIGHT.contains(&d)
}

pub fn letter(k: Kind) -> char {
    match k {
        Kind::Pawn => 'p',
        Kind::Knight => 'n',
        Kind::Rook => 'r',
        Kind::Queen => 'q',
    }
}

fn letters(ks: &[Kind]) -> String {
    ks.iter().map(|k| letter(*k)).collect()
}

/// A `match` on an element, and the list where a `&[Kind]` goes.
pub fn promotion(i: usize) -> String {
    let first = match PROMOTIONS[i] {
        Kind::Queen => "queen",
        Kind::Rook => "rook",
        _ => "other",
    };
    format!("{first} {} {}", letters(&PROMOTIONS), letters(&BLANK))
}

pub fn promotes_to(k: Kind) -> bool {
    PROMOTIONS.contains(&k)
}

/// Variants with payloads, matched.
pub fn show_square(i: usize) -> String {
    match BACK[i] {
        Square::Empty => String::from("."),
        Square::Piece(k, white) => format!("{}{}", letter(k), if white { "w" } else { "b" }),
        Square::Marked { piece, at } => format!("{}@{},{}", letter(piece), at.0, at.1),
    }
}

pub fn back(i: usize) -> Square {
    BACK[i]
}

pub fn edge(i: usize, j: usize) -> Square {
    EDGES[i][j]
}

/// Tuples holding variants, walked and compared.
pub fn value_of(k: Kind) -> u32 {
    let mut found = 0;
    for (kind, v) in VALUES {
        if kind == k {
            found = v;
        }
    }
    found
}

pub fn plain(k: Kind) -> String {
    let (kind, at) = START;
    let (r, c) = at;
    let same = DEFAULT == k;
    format!("{}{r}{c} {} {} {same}", letter(kind), ORIGIN.0, ORIGIN.1)
}

pub fn named(i: usize) -> (String, f64) {
    let (c, s, x) = NAMED[i];
    (format!("{c}{s}"), x)
}

/// A copy of a const array, written, and pushes of its elements into a
/// `Vec` of the enum.
pub fn edited(k: Kind) -> Vec<Kind> {
    let mut a = PROMOTIONS;
    a[0] = k;
    let mut out: Vec<Kind> = Vec::new();
    out.push(PROMOTIONS[0]);
    for x in a {
        out.push(x);
    }
    out.push(DEFAULT);
    out
}

pub fn all() -> ([Kind; 3], [(i8, i8); 4], (Kind, (u8, u8))) {
    (PROMOTIONS, ROOK, START)
}
