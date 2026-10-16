//! Crate consts of tuples and of the crate's enums (design/01 §7.29): a
//! direction table destructured in a `for` by value and by reference,
//! arrays of arrays, indexing in range and past the end, `len`,
//! `contains`, a `match` on an element, the list where a `&[K]` goes,
//! variants with payloads, tuples holding variants, a plain tuple and
//! variant, a written copy, and consts as a result.

use crate::support;

purecrate_canon::fixture!(mod const_values = "fixtures/const_values.rs");

use const_values::Kind;

const KINDS: [Kind; 4] = [Kind::Pawn, Kind::Knight, Kind::Rook, Kind::Queen];

#[test]
fn generated_const_values_match_rust() {
    support::equivalence("const_values", const_values::SOURCE, |cases| {
        grid!(cases, [const_values::knight_moves, const_values::reach]; r in [0i8, 3, 7, 120], c in [0i8, 1, 7, -128]);
        grid!(cases, const_values::knight; i in [0usize, 7, 8, usize::MAX >> 11]);
        grid!(cases, const_values::line; i in [0usize, 1, 2], j in [0usize, 3, 4]);
        cases.push(case!(const_values::lens()));
        grid!(cases, const_values::is_knight_move; d in [(1i8, 2i8), (2, -1), (2, 2), (-1, 2), (0, 0)]);
        grid!(cases, [const_values::promotion, const_values::show_square, const_values::back]; i in [0usize, 2, 3, 4]);
        grid!(cases, const_values::edge; i in [0usize, 1, 2], j in [0usize, 1, 2]);
        grid!(cases, [const_values::promotes_to, const_values::value_of, const_values::plain, const_values::edited]; k in KINDS);
        grid!(cases, const_values::named; i in [0usize, 1, 2]);
        cases.push(case!(const_values::all()));
    });
}
