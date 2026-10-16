//! `==` and `!=` on the crate's enums, printed as a test of `kind` where
//! the variant decides, match Rust on every pair of variants: against a unit
//! variant on either side, two values of an all-unit enum, references,
//! fields and elements, calls that panic (each side once, the left first),
//! narrowed sides, and the forms that stay `Eq.deep` (a payload, one
//! variant, `Option`, a generic `T`).

use crate::support;

purecrate_canon::fixture!(mod enum_eq = "fixtures/enum_eq.rs");

use enum_eq::{Dir, Only, Piece, Slot, Tile};

const DIRS: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

fn tiles() -> Vec<Tile> {
    vec![Tile::Empty, Tile::Wall, Tile::Coin(3), Tile::Coin(4), Tile::Door { open: true }, Tile::Door { open: false }]
}

fn slots() -> Vec<Slot<u8>> {
    vec![Slot::Free, Slot::Taken(2), Slot::Taken(3)]
}

#[test]
fn enum_eq_matches_rust() {
    support::equivalence("enum_eq", enum_eq::SOURCE, |cases| {
        for a in DIRS {
            cases.push(case!(enum_eq::is_east(a)));
            cases.push(case!(enum_eq::not_east_left(a)));
            cases.push(case!(enum_eq::at_home(a)));
            cases.push(case!(enum_eq::narrowed(a)));
            cases.push(case!(enum_eq::option_test(Some(a))));
            for b in DIRS {
                cases.push(case!(enum_eq::same(a, b)));
                cases.push(case!(enum_eq::differ(a, b)));
                cases.push(case!(enum_eq::through_refs(&a, &b)));
                cases.push(case!(enum_eq::generic_dirs(a, b)));
                cases.push(case!(enum_eq::both_narrowed(a, b)));
                cases.push(case!(enum_eq::after_return(a, b)));
            }
            for x in [0u8, 1, 2, 5, 8] {
                for y in [0u8, 3, 9] {
                    cases.push(case!(enum_eq::calls(a, x, y)));
                }
            }
        }
        cases.push(case!(enum_eq::option_test(None::<Dir>)));
        cases.push(case!(enum_eq::literals()));
        for c in tiles() {
            cases.push(case!(enum_eq::tile_narrowed(c)));
            for n in [3u8, 4] {
                cases.push(case!(enum_eq::tile_tests(c, n)));
            }
            for d in tiles() {
                cases.push(case!(enum_eq::tiles_same(c, d)));
            }
        }
        for x in [0u8, 1, 2, 9, 10] {
            cases.push(case!(enum_eq::tile_calls(x)));
        }
        cases.push(case!(enum_eq::only_tests(Only::One, Only::One)));
        for s in slots() {
            for t in slots() {
                cases.push(case!(enum_eq::slot_tests(s.clone(), t)));
            }
        }
        let pieces: Vec<Piece> = DIRS.iter().zip(tiles()).map(|(&kind, at)| Piece { kind, at }).collect();
        for p in &pieces {
            for q in &pieces {
                cases.push(case!(enum_eq::piece_tests(p, q)));
            }
        }
        let cells = tiles();
        let dirs = [Dir::East, Dir::West, Dir::East, Dir::North, Dir::North, Dir::South];
        for i in 0..6 {
            cases.push(case!(enum_eq::board_tests(&cells, &dirs, i)));
        }
        cases.push(case!(enum_eq::board_tests(&cells, &dirs, 6)));
        cases.push(case!(enum_eq::runs(&dirs)));
        cases.push(case!(enum_eq::runs(&[Dir::West, Dir::West, Dir::North, Dir::North, Dir::West, Dir::North])));
        cases.push(case!(enum_eq::runs(&[] as &[Dir])));
        cases.push(case!(enum_eq::count_free(&cells)));
        cases.push(case!(enum_eq::count_free(&[Tile::Empty, Tile::Empty])));
        for a in [0u8, 1, 2] {
            for b in [0u8, 1, 2] {
                cases.push(case!(enum_eq::ordering_narrowed(a, b)));
            }
        }
    });
}
