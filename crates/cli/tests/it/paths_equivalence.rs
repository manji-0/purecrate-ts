//! Associated consts and paths through modules: consts of every kind read
//! as `Self::N` and `T::N`, a private one, one on a generic type, one read
//! by a crate const and a local one, and calls, types, consts, and variants
//! named through `crate::`, `self::`, `super::`, and a child module, at
//! the bounds where the arithmetic around them overflows.

use crate::support;

purecrate_canon::fixture!(mod paths = "fixtures/paths.rs");

#[test]
fn generated_paths_match_rust() {
    use paths::Shape;
    support::equivalence("paths", paths::SOURCE, |cases| {
        let shapes = [Shape::Dot, Shape::Square(0), Shape::Square(3), Shape::Square(65_535), Shape::Square(65_536)];
        for n in [0u32, 1, 65_535, 65_536, u32::MAX] {
            cases.push(case!(paths::square(n)));
            cases.push(case!(paths::area_of(n)));
            cases.push(case!(paths::doubled(n)));
            cases.push(case!(paths::checked_side(n)));
        }
        for n in [0u32, 1, u32::MAX - 13_107, u32::MAX - 13_106, u32::MAX] {
            cases.push(case!(paths::bump(n)));
        }
        for s in shapes {
            cases.push(case!(paths::perimeter_of(s)));
            cases.push(case!(paths::kind(s)));
            cases.push(case!(paths::is_dot(s)));
        }
        cases.push(case!(paths::perimeter_of(Shape::Square(1_073_741_824))));
        cases.push(case!(paths::sides()));
        cases.push(case!(paths::label_len()));
        for n in [0usize, 3, 4] {
            cases.push(case!(paths::fits(n)));
        }
        let grid = |cells: Vec<Shape>| paths::Grid { cells };
        for g in [grid(vec![]), grid(vec![Shape::Dot]), grid(vec![Shape::Square(3), Shape::Dot])] {
            cases.push(case!(paths::weight(g.clone())));
            cases.push(case!(paths::empty_ok(g)));
        }
        cases.push(case!(paths::weight(grid(vec![Shape::Square(4)]))));
        for s in ["", "a", "é😀"] {
            cases.push(case!(paths::label(s)));
        }
        for x in [0.0f64, -0.0, 3.0, f64::MAX, f64::NAN] {
            cases.push(case!(paths::scaled(x)));
        }
        for xs in [vec![], vec![1u32], vec![1, 2, 3]] {
            cases.push(case!(paths::page_full(xs)));
        }
    });
}
