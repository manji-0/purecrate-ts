//! `==` on derived `PartialEq` matches Rust on every pair from a set of
//! values that differ in variant, field, length, nesting, and float edge.

use crate::support;

purecrate_canon::fixture!(mod deep_eq = "fixtures/deep_eq.rs");

use deep_eq::{Json, Point, Shape, Tagged};

fn jsons() -> Vec<Json> {
    vec![
        Json::Null,
        Json::Bool(true),
        Json::Bool(false),
        Json::Num(0.0),
        Json::Num(-0.0),
        Json::Num(f64::NAN),
        Json::Num(1.5),
        Json::Str(String::new()),
        Json::Str("é".into()),
        Json::Arr(vec![]),
        Json::Arr(vec![Json::Null]),
        Json::Arr(vec![Json::Null, Json::Null]),
        Json::Arr(vec![Json::Arr(vec![Json::Num(1.0)])]),
        Json::Arr(vec![Json::Arr(vec![Json::Num(2.0)])]),
        Json::Obj(vec![]),
        Json::Obj(vec![("a".into(), Json::Null)]),
        Json::Obj(vec![("b".into(), Json::Null)]),
        Json::Obj(vec![("a".into(), Json::Null), ("b".into(), Json::Bool(true))]),
        Json::Obj(vec![("b".into(), Json::Bool(true)), ("a".into(), Json::Null)]),
    ]
}

fn points() -> Vec<Point> {
    vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: -0.0, y: 0.0 },
        Point { x: 1.0, y: 0.0 },
        Point { x: 0.0, y: 1.0 },
        Point { x: f64::NAN, y: 0.0 },
    ]
}

fn shapes() -> Vec<Shape> {
    let p = Point { x: 1.0, y: 2.0 };
    vec![
        Shape::Dot,
        Shape::Circle { at: p.clone(), r: 1.0 },
        Shape::Circle { at: p.clone(), r: 2.0 },
        Shape::Poly(vec![], None),
        Shape::Poly(vec![p.clone()], None),
        Shape::Poly(vec![p.clone()], Some(String::new())),
        Shape::Poly(vec![p.clone(), p], Some("x".into())),
    ]
}

#[test]
fn deep_eq_matches_rust() {
    support::equivalence("deep_eq", deep_eq::SOURCE, |cases| {
        for a in jsons() {
            for b in jsons() {
                cases.push(case!(deep_eq::json_eq(a.clone(), b.clone())));
                cases.push(case!(deep_eq::pairs_eq((1, a.clone()), (1, b.clone()))));
            }
            cases.push(case!(deep_eq::position_of(jsons(), a.clone())));
            cases.push(case!(deep_eq::pairs_eq((1, a.clone()), (2, a.clone()))));
        }
        for a in points() {
            for b in points() {
                cases.push(case!(deep_eq::points_eq(a.clone(), b.clone())));
                cases.push(case!(deep_eq::options_eq(Some::<Point>(a.clone()), Some::<Point>(b.clone()))));
                cases.push(case!(deep_eq::results_eq(Ok::<Point, String>(a.clone()), Ok::<Point, String>(b.clone()))));
            }
            cases.push(case!(deep_eq::options_eq(Some(a.clone()), None::<Point>)));
            cases.push(case!(deep_eq::results_eq(Ok::<Point, String>(a.clone()), Err::<Point, String>(String::new()))));
        }
        cases.push(case!(deep_eq::options_eq(None::<Point>, None::<Point>)));
        for a in shapes() {
            for b in shapes() {
                cases.push(case!(deep_eq::shapes_eq(a.clone(), b.clone())));
            }
        }
        let tagged = |id: u64, label: Option<&str>, shapes: Vec<Shape>| Tagged {
            id,
            label: label.map(String::from),
            shapes,
            unit: (),
        };
        let ts = [
            tagged(1, None, vec![]),
            tagged(2, None, vec![]),
            tagged(1, Some(""), vec![]),
            tagged(1, None, vec![Shape::Dot]),
        ];
        for a in &ts {
            for b in &ts {
                cases.push(case!(deep_eq::tagged_eq(a.clone(), b.clone())));
            }
        }
        cases.push(case!(deep_eq::lists_eq(jsons(), jsons())));
        cases.push(case!(deep_eq::lists_eq(jsons(), Vec::<Json>::new())));
    });
}
