//! `==` through an `Option`, a `Vec`, a slice, an array, a tuple, and a
//! `BTreeMap` compared by each part's own type (design/01 §5.1) matches
//! Rust's on every pair of a set of values: floats at NaN, `-0.0`, and
//! `0.0`, `None` against each `Some`, lists that differ in length or only
//! in a later element, nested lists of `Option`s, maps that differ in a
//! key or only in a value, enums of every variant shape, a `T`, a `Result`,
//! and a set (still `Eq.deep`), what a narrowed `Option` compares to, and
//! calls that panic (each side once, the left first).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::support;

purecrate_canon::fixture!(mod parts_eq = "fixtures/parts_eq.rs");

use parts_eq::{Color, Point, Shape, Slot};

fn points() -> Vec<Point> {
    vec![
        Point { x: 0.0, y: 0 },
        Point { x: -0.0, y: 0 },
        Point { x: f64::NAN, y: 0 },
        Point { x: 1.5, y: 0 },
        Point { x: 1.5, y: -1 },
    ]
}

fn options<T: Clone>(xs: &[T]) -> Vec<Option<T>> {
    std::iter::once(None).chain(xs.iter().cloned().map(Some)).collect()
}

fn lists() -> Vec<Vec<Point>> {
    let p = points();
    vec![vec![], vec![p[0]], vec![p[1]], vec![p[2]], vec![p[0], p[3]], vec![p[0], p[4]], vec![p[3], p[0]]]
}

fn shapes() -> Vec<Shape> {
    let p = points();
    vec![
        Shape::Dot(p[0]),
        Shape::Dot(p[1]),
        Shape::Dot(p[2]),
        Shape::Line { from: p[0], to: p[3] },
        Shape::Line { from: p[0], to: p[4] },
        Shape::Path(vec![]),
        Shape::Path(vec![p[3]]),
        Shape::Tagged(None, vec![None]),
        Shape::Tagged(Some("a".into()), vec![None]),
        Shape::Tagged(None, vec![Some(p[3])]),
        Shape::Tagged(None, vec![Some(p[2])]),
    ]
}

fn map<V: Clone>(pairs: &[(&str, V)]) -> BTreeMap<String, V> {
    pairs.iter().rev().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

#[test]
fn parts_eq_matches_rust() {
    support::equivalence("parts_eq", parts_eq::SOURCE, |cases| {
        let bytes = options(&[0u8, 3, 255]);
        for &a in &bytes {
            for &b in &bytes {
                for x in [0u8, 3] {
                    cases.push(case!(parts_eq::options_u8(a, b, x)));
                }
                cases.push(case!(parts_eq::narrowed(a, b)));
                cases.push(case!(parts_eq::looped(a, b.unwrap_or(2))));
            }
        }
        let floats = options(&[0.0, -0.0, f64::NAN, 1.5]);
        for &a in &floats {
            for &b in &floats {
                cases.push(case!(parts_eq::options_f64(a, b)));
            }
        }
        for (a, b, c) in
            [(None, None, None), (Some(-1i64), Some(u128::MAX), Some('é')), (Some(i64::MIN), Some(0u128), Some('e'))]
        {
            cases.push(case!(parts_eq::options_wide(a, b, c)));
        }
        let texts = options(&[String::new(), "x".to_string(), "é".to_string()]);
        for a in &texts {
            for b in &texts {
                for c in [None, Some("x"), Some("")] {
                    cases.push(case!(parts_eq::options_text(a.clone(), b.clone(), c)));
                }
            }
        }
        for a in options(&[true, false]) {
            cases.push(case!(parts_eq::options_bool(a)));
        }
        let ps = options(&points());
        let colors = options(&[Color::White, Color::Black]);
        for &a in &ps {
            for &b in &ps {
                cases.push(case!(parts_eq::options_of_types(a, b, colors[1], colors[2])));
            }
        }
        for &c in &colors {
            for &d in &colors {
                cases.push(case!(parts_eq::options_of_types(ps[0], ps[0], c, d)));
            }
        }
        for c in options(&[Ordering::Less, Ordering::Equal, Ordering::Greater]) {
            for (a, b) in [(1u8, 2u8), (2, 2), (3, 2)] {
                cases.push(case!(parts_eq::options_of_ordering(a, b, c)));
            }
        }
        for a in &lists() {
            for b in &lists() {
                cases.push(case!(parts_eq::lists(a, b.clone())));
                cases.push(case!(parts_eq::generic(a.clone(), b.clone(), 1)));
            }
        }
        let p = points();
        let grids: Vec<Vec<Vec<Option<Point>>>> = vec![
            vec![],
            vec![vec![]],
            vec![vec![None]],
            vec![vec![Some(p[0])]],
            vec![vec![Some(p[1])]],
            vec![vec![Some(p[2])]],
            vec![vec![Some(p[0])], vec![]],
            vec![vec![Some(p[0]), None]],
        ];
        for a in &grids {
            for b in &grids {
                cases.push(case!(parts_eq::nested(a, b)));
            }
        }
        let fs: Vec<Vec<f64>> = vec![vec![], vec![0.0], vec![-0.0], vec![f64::NAN], vec![0.0, 1.0]];
        let arrays: Vec<[f32; 2]> = vec![[0.0, 1.0], [-0.0, 1.0], [f32::NAN, 1.0], [0.0, 2.0]];
        for (i, a) in fs.iter().enumerate() {
            for (j, b) in fs.iter().enumerate() {
                cases.push(case!(parts_eq::floats(a.clone(), b.clone(), arrays[i % 4], arrays[j % 4])));
            }
        }
        let words: Vec<Vec<String>> =
            vec![vec![], vec!["a".into()], vec!["a".into(), "b".into()], vec!["a".into(), "c".into()]];
        for a in &words {
            for b in &words {
                cases.push(case!(parts_eq::texts(a, b)));
            }
        }
        let (w, k) = (Color::White, Color::Black);
        for a in [[w, w, w], [w, k, w], [k, w, w]] {
            for b in [[w, w, w], [w, k, w]] {
                cases.push(case!(parts_eq::colors(a, b)));
            }
        }
        for a in &p {
            for b in &p {
                for (m, n) in [(1u8, 1u8), (1, 2)] {
                    cases.push(case!(parts_eq::tuples((m, *a), (n, *b), 1)));
                }
            }
        }
        let parts: Vec<(f64, Option<Point>, Vec<Color>)> = vec![
            (0.0, None, vec![]),
            (-0.0, None, vec![]),
            (f64::NAN, None, vec![]),
            (0.0, Some(p[0]), vec![]),
            (0.0, Some(p[2]), vec![]),
            (0.0, None, vec![w]),
            (0.0, None, vec![k]),
        ];
        for a in &parts {
            for b in &parts {
                cases.push(case!(parts_eq::tuple_parts(a.clone(), b.clone())));
            }
        }
        for a in &shapes() {
            for b in &shapes() {
                cases.push(case!(parts_eq::shapes(a, b)));
            }
        }
        let maps = vec![
            map(&[]),
            map(&[("a", p[0])]),
            map(&[("a", p[1])]),
            map(&[("a", p[2])]),
            map(&[("b", p[0])]),
            map(&[("a", p[0]), ("b", p[3])]),
        ];
        for a in &maps {
            for b in &maps {
                cases.push(case!(parts_eq::maps(a, b)));
            }
        }
        let lists_by_key: Vec<BTreeMap<u32, Vec<f64>>> = vec![
            BTreeMap::new(),
            [(1, vec![])].into_iter().collect(),
            [(1, vec![0.0])].into_iter().collect(),
            [(1, vec![-0.0])].into_iter().collect(),
            [(1, vec![f64::NAN])].into_iter().collect(),
            [(2, vec![0.0])].into_iter().collect(),
        ];
        for a in &lists_by_key {
            for b in &lists_by_key {
                cases.push(case!(parts_eq::map_lists(a, b)));
            }
        }
        let sets: Vec<BTreeSet<String>> =
            vec![BTreeSet::new(), ["a".to_string()].into(), ["a".to_string(), "b".to_string()].into()];
        for a in &sets {
            for b in &sets {
                cases.push(case!(parts_eq::sets(a, b)));
            }
        }
        let results: Vec<Result<Point, String>> = vec![Ok(p[0]), Ok(p[1]), Ok(p[2]), Err(String::new())];
        for a in &results {
            for b in &results {
                cases.push(case!(parts_eq::results(a.clone(), b.clone())));
            }
        }
        let slots = options(&[Slot { value: 2u8, n: 1 }, Slot { value: 2, n: 2 }, Slot { value: 3, n: 1 }]);
        for a in &slots {
            for b in &slots {
                cases.push(case!(parts_eq::slots(a.clone(), b.clone())));
            }
        }
        for x in [0u8, 1, 3, 4, 10] {
            for y in [0u8, 1, 4, 11] {
                cases.push(case!(parts_eq::calls(x, y)));
            }
        }
    });
}
