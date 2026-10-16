//! Unsuffixed literals a later use types: a `let` of a `match` (with a
//! diverging arm, a `panic!`, and a negative literal) or an `if`, and an
//! array of literals walked by a `for` as indexes and as offsets, at the
//! edges where the inferred type overflows.

use crate::support;

purecrate_canon::fixture!(mod literal_uses = "fixtures/literal_uses.rs");

#[test]
fn generated_literal_uses_match_rust() {
    support::equivalence("literal_uses", literal_uses::SOURCE, |cases| {
        use literal_uses::Piece;
        for p in [Piece::Pawn, Piece::Knight, Piece::King] {
            grid!(cases, literal_uses::weight; p in [p], x in [0, 1, 55, 56, 252, 255]);
            grid!(cases, literal_uses::step; p in [p], by in [0, 1, -1, 63, 64, -64, -65, i8::MIN, i8::MAX]);
        }
        grid!(cases, literal_uses::pick; c in [true, false], base in [0, 1, i64::MIN, i64::MAX, -9_000_000_000]);
        grid!(cases, literal_uses::rank; n in [0, 1, 9, 10, 214_748_364, u32::MAX]);
        let u16s: [&[u16]; 4] = [&[], &[1, 2], &[1, 2, 3], &[65535, 0, 9, 4]];
        for v in u16s {
            cases.push(case!(literal_uses::gather(v)));
        }
        grid!(cases, literal_uses::around; at in [0, i16::MIN, i16::MIN + 299, i16::MIN + 300, i16::MAX, 32_467]);
    });
}
