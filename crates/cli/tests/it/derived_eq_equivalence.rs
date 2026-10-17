//! The derived `eq` printed per type matches Rust's `==` on every pair of a
//! set of values: floats at NaN, `-0.0`, and `0.0`, structs that differ in
//! the first field or only in a later one, a recursive tree, generic types,
//! `Option` and `Vec` fields, a struct with no field, a newtype, a type
//! with its own method `eq`, calls that panic (each side once, the left
//! first), and `matches!` on an enum of one variant read from a call.

use crate::support;

purecrate_canon::fixture!(mod derived_eq = "fixtures/derived_eq.rs");

use derived_eq::{Cents, Color, Either, Money, Nothing, Pair, Pen, Point, Stroke, Tree};

fn points() -> Vec<Point> {
    vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: -0.0, y: 0.0 },
        Point { x: 0.0, y: -0.0 },
        Point { x: f64::NAN, y: 0.0 },
        Point { x: 1.5, y: f32::NAN },
        Point { x: 1.5, y: 2.5 },
    ]
}

fn pens() -> Vec<Pen> {
    let at = Point { x: 1.0, y: 2.0 };
    vec![
        Pen { color: Color::Red, width: 1, at },
        Pen { color: Color::Blue, width: 1, at },
        Pen { color: Color::Red, width: 2, at },
        Pen { color: Color::Red, width: 1, at: Point { x: 1.0, y: 3.0 } },
        Pen { color: Color::Red, width: 1, at: Point { x: f64::NAN, y: 2.0 } },
    ]
}

fn strokes() -> Vec<Stroke> {
    let pen = pens()[0].clone();
    let base = Stroke { pen: pen.clone(), path: vec![], label: None, big: 0, wide: 0, done: () };
    vec![
        base.clone(),
        Stroke { pen: pens()[1].clone(), ..base.clone() },
        Stroke { path: vec![Point { x: 0.0, y: 0.0 }], ..base.clone() },
        Stroke { path: vec![Point { x: -0.0, y: 0.0 }], ..base.clone() },
        Stroke { label: Some(String::new()), ..base.clone() },
        Stroke { label: Some("é".into()), ..base.clone() },
        Stroke { big: i64::MIN, ..base.clone() },
        Stroke { wide: u128::MAX, ..base },
    ]
}

fn trees() -> Vec<Tree> {
    let leaf = || Box::new(Tree::Leaf);
    vec![
        Tree::Leaf,
        Tree::Node(leaf(), 1, leaf()),
        Tree::Node(leaf(), 2, leaf()),
        Tree::Node(Box::new(Tree::Node(leaf(), 1, leaf())), 1, leaf()),
        Tree::Node(leaf(), 1, Box::new(Tree::Node(leaf(), 1, leaf()))),
        Tree::Named { name: "a".into(), child: None },
        Tree::Named { name: "a".into(), child: Some(leaf()) },
        Tree::Named { name: "b".into(), child: Some(leaf()) },
    ]
}

fn eithers() -> Vec<Either<u8, String>> {
    vec![Either::Left(3), Either::Left(4), Either::Right("3".into()), Either::Right(String::new()), Either::Neither]
}

#[test]
fn derived_eq_matches_rust() {
    support::equivalence("derived_eq", derived_eq::SOURCE, |cases| {
        for a in points() {
            for b in points() {
                cases.push(case!(derived_eq::points_eq(a, b)));
                let p = |l: Point, r: Point, n: u32| Pair { left: l, right: r, n };
                cases.push(case!(derived_eq::pairs_eq(p(a, b, 1), p(a, b, 1))));
                cases.push(case!(derived_eq::pairs_eq(p(a, a, 1), p(b, a, 1))));
                cases.push(case!(derived_eq::pairs_eq(p(a, b, 1), p(a, b, 2))));
            }
        }
        for a in &pens() {
            cases.push(case!(derived_eq::pens_in(&pens(), a)));
            cases.push(case!(derived_eq::pens_in(&pens()[1..], a)));
        }
        for a in &strokes() {
            for b in &strokes() {
                cases.push(case!(derived_eq::strokes_eq(a, b)));
            }
        }
        for a in &trees() {
            for b in &trees() {
                cases.push(case!(derived_eq::trees_eq(a, b)));
            }
        }
        for a in eithers() {
            for b in eithers() {
                cases.push(case!(derived_eq::eithers_eq(a.clone(), b)));
            }
        }
        cases.push(case!(derived_eq::nothing_eq(Nothing {}, Nothing {})));
        for a in [Cents(0), Cents(-1), Cents(i64::MAX)] {
            for b in [Cents(0), Cents(-1), Cents(i64::MAX)] {
                cases.push(case!(derived_eq::cents_eq(a, b)));
            }
        }
        let money = |cents: i64, currency: &str| Money { cents, currency: currency.into() };
        for a in &[money(1, "EUR"), money(2, "EUR"), money(1, "USD")] {
            for b in &[money(1, "EUR"), money(2, "EUR"), money(1, "USD")] {
                cases.push(case!(derived_eq::money_eq(a, b)));
            }
        }
        for x in [0u8, 1, 2, 10] {
            for y in [0u8, 2, 11] {
                cases.push(case!(derived_eq::pen_calls(x, y)));
            }
            cases.push(case!(derived_eq::only_calls(x)));
        }
    });
}
