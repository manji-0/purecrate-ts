//! Crate consts of the crate's open structs (design/01 §7.29): a plain
//! struct and one nested in another, read by field and passed on, arrays
//! of structs indexed in range and past the end, walked by value and by
//! reference, a `match` on an element's fields, `==` and `contains`
//! through the derived `eq`, a struct of a non-ASCII `&str`, a `char`
//! outside the BMP, and a float, structs in a tuple and in a variant's
//! payload, a written copy of a const's array, and consts as a result.

use crate::support;

purecrate_canon::fixture!(mod const_structs = "fixtures/const_structs.rs");

use const_structs::{Kind, Piece, Point};

const POINTS: [Point; 4] =
    [Point { row: 0, col: 0 }, Point { row: -8, col: 7 }, Point { row: 3, col: 4 }, Point { row: 7, col: -8 }];
const IDX: [usize; 4] = [0, 2, 3, usize::MAX >> 11];

#[test]
fn generated_const_structs_match_rust() {
    support::equivalence("const_structs", const_structs::SOURCE, |cases| {
        cases.push(case!(const_structs::origin()));
        cases.push(case!(const_structs::corner_sum()));
        grid!(cases, [const_structs::piece_at, const_structs::piece_kind, const_structs::reach]; i in IDX);
        grid!(cases, const_structs::describe; i in [0usize, 1, 2], j in [0usize, 1, 2]);
        grid!(cases, const_structs::is_corner; p in POINTS);
        let pawn = Piece { kind: Kind::Pawn, at: Point { row: 0, col: 0 }, steps: [(1, 0), (2, 0)] };
        let rook = Piece { kind: Kind::Rook, at: Point { row: -8, col: 7 }, steps: [(0, 1), (0, 1)] };
        let other = Piece { kind: Kind::Rook, at: Point { row: -8, col: 7 }, steps: [(0, 1), (1, 0)] };
        grid!(cases, const_structs::same_piece; p in [pawn, rook, other]);
        grid!(cases, [const_structs::label_at, const_structs::show_square, const_structs::board]; i in IDX);
        cases.push(case!(const_structs::start()));
        grid!(cases, const_structs::moved; dr in [0i8, -1, 127]);
        cases.push(case!(const_structs::all()));
    });
}
