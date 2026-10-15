//! `PartialOrd` and `Ord` on the crate's types against Rust: every pair
//! from sets of values that differ in the first field, in a later one only
//! (an equal prefix), in variant, in a written discriminant, in nesting and
//! length (`Option` with `None` first, `Vec` shorter first, a recursive
//! tree through `Box`), with non-ASCII text; a hand-written `Ord` whose
//! ties show which of two equals `max`, `min`, `clamp`, the iterator's
//! `max` / `min`, and a stable `sort` keep; `clamp` panicking where its
//! bounds cross; and an overflow inside a hand-written `cmp`, under `<` and
//! `sort`.

use crate::support;

purecrate_canon::fixture!(mod ordering_derive = "fixtures/ordering_derive.rs");

use ordering_derive::{Bumped, ByLen, Entry, Id, Level, Shape, Small, Solo, Tree, Unit, Version};

fn versions() -> Vec<Version> {
    let v = |major: u32, minor: i64, pre: Option<&str>, build: &[u8]| Version {
        major,
        minor,
        pre: pre.map(String::from),
        build: build.to_vec(),
    };
    vec![
        v(0, 0, None, &[]),
        v(0, 0, Some(""), &[]),
        v(0, 0, Some("a"), &[]),
        v(0, 0, Some("é"), &[]),
        v(0, 0, Some("\u{e000}"), &[]),
        v(0, 0, Some("😀"), &[]),
        v(0, 0, None, &[0]),
        v(0, 0, None, &[0, 0]),
        v(0, 0, None, &[1]),
        v(0, -1, None, &[]),
        v(0, i64::MIN, None, &[]),
        v(0, i64::MAX, None, &[]),
        v(1, 0, None, &[]),
        v(u32::MAX, i64::MIN, Some("z"), &[255]),
    ]
}

fn shapes() -> Vec<Shape> {
    vec![
        Shape::Dot,
        Shape::Circle(i32::MIN),
        Shape::Circle(0),
        Shape::Circle(i32::MAX),
        Shape::Rect { w: 0, h: 255 },
        Shape::Rect { w: 1, h: 0 },
        Shape::Rect { w: 1, h: 1 },
        Shape::Named(String::new(), 'a', false),
        Shape::Named("ab".into(), 'a', true),
        Shape::Named("ab".into(), 'é', false),
        Shape::Named("ab".into(), '😀', false),
        Shape::Named("日本".into(), '\0', true),
    ]
}

fn trees() -> Vec<Tree> {
    let leaf = |n: u16| Tree::Leaf(Id(n));
    let node = |t: Tree, ts: Vec<Tree>| Tree::Node(Box::new(t), ts);
    vec![
        leaf(0),
        leaf(1),
        node(leaf(0), vec![]),
        node(leaf(0), vec![leaf(0)]),
        node(leaf(0), vec![leaf(1)]),
        node(leaf(0), vec![leaf(0), leaf(0)]),
        node(leaf(1), vec![]),
        node(node(leaf(0), vec![]), vec![]),
    ]
}

fn entries() -> Vec<Entry> {
    let e = |level: Level, shape: Option<Shape>, ids: &[u16], pair: (Small, &str)| Entry {
        level,
        shape,
        ids: ids.iter().map(|&n| Id(n)).collect(),
        pair: (pair.0, pair.1.to_string()),
    };
    vec![
        e(Level::Low, None, &[], (Small::A, "")),
        e(Level::Low, Some(Shape::Dot), &[], (Small::A, "")),
        e(Level::Low, Some(Shape::Circle(-1)), &[], (Small::A, "")),
        e(Level::Low, None, &[0], (Small::A, "")),
        e(Level::Low, None, &[], (Small::B, "")),
        e(Level::Low, None, &[], (Small::A, "x")),
        e(Level::Mid, None, &[], (Small::A, "")),
        e(Level::High, None, &[], (Small::A, "")),
    ]
}

fn by_len(text: &str, tag: u8) -> ByLen {
    ByLen { text: text.into(), tag }
}

/// Every pair of `xs`, each side cloned into the call.
macro_rules! pairs {
    ($cases:ident, $m:ident :: $f:ident, $xs:expr) => {
        let xs = $xs;
        for a in &xs {
            for b in &xs {
                $cases.push(case!($m::$f(a.clone(), b.clone())));
            }
        }
    };
}

#[test]
fn derived_and_written_ord_match_rust() {
    support::equivalence("ordering_derive", ordering_derive::SOURCE, |cases| {
        pairs!(cases, ordering_derive::ops_version, versions());
        pairs!(cases, ordering_derive::ops_shape, shapes());
        pairs!(cases, ordering_derive::ops_level, [Level::High, Level::Low, Level::Mid]);
        pairs!(cases, ordering_derive::ops_entry, entries());
        pairs!(cases, ordering_derive::ops_tree, trees());
        pairs!(cases, ordering_derive::ops_id, [Id(0), Id(1), Id(u16::MAX)]);
        cases.push(case!(ordering_derive::ops_unit(Unit {}, Unit {})));
        pairs!(
            cases,
            ordering_derive::ops_solo,
            [Solo::Only(0, String::new()), Solo::Only(0, "a".into()), Solo::Only(1, String::new())]
        );
        let texts = [by_len("", 0), by_len("é", 1), by_len("ab", 2), by_len("😀", 3), by_len("abcd", 4)];
        pairs!(cases, ordering_derive::ops_by_len, texts.clone());
        pairs!(cases, ordering_derive::ops_bumped, [Bumped { n: 0 }, Bumped { n: 254 }, Bumped { n: 255 }]);
        cases.push(case!(ordering_derive::sort_bumped(vec![Bumped { n: 3 }, Bumped { n: 1 }, Bumped { n: 2 }])));
        cases.push(case!(ordering_derive::sort_bumped(vec![Bumped { n: 3 }, Bumped { n: 255 }])));
        cases.push(case!(ordering_derive::sort_bumped(Vec::<Bumped>::new())));
        let three = [Shape::Dot, Shape::Circle(0), Shape::Rect { w: 0, h: 0 }];
        for x in shapes() {
            for lo in &three {
                for hi in &three {
                    cases.push(case!(ordering_derive::clamp_shape(x.clone(), lo.clone(), hi.clone())));
                }
            }
        }
        // Two of one length are equal: which one comes back shows std's rule.
        for x in &texts {
            for lo in [by_len("é", 5), by_len("xyz", 6)] {
                for hi in [by_len("ab", 7), by_len("", 8)] {
                    cases.push(case!(ordering_derive::clamp_by_len(x.clone(), lo.clone(), hi.clone())));
                }
            }
        }
        let mut all = versions();
        all.reverse();
        cases.push(case!(ordering_derive::sorted(all.clone())));
        cases.push(case!(ordering_derive::sorted(Vec::<Version>::new())));
        let ties =
            vec![by_len("ab", 0), by_len("", 1), by_len("😀", 2), by_len("xy", 3), by_len("", 4), by_len("é", 5)];
        cases.push(case!(ordering_derive::sorted_by_len(ties.clone())));
        cases.push(case!(ordering_derive::extremes(ties.clone())));
        cases.push(case!(ordering_derive::extremes(Vec::<ByLen>::new())));
        cases.push(case!(ordering_derive::extremes(vec![by_len("a", 9), by_len("b", 9)])));
        let mut rev = entries();
        rev.reverse();
        cases.push(case!(ordering_derive::sorted_by_key(rev.clone())));
        cases.push(case!(ordering_derive::extreme_shapes(shapes())));
        cases.push(case!(ordering_derive::extreme_shapes(Vec::<Shape>::new())));
        let options = [None, Some(Shape::Dot), Some(Shape::Circle(1)), Some(Shape::Named("é".into(), 'x', true))];
        let lists: [Vec<Id>; 4] = [vec![], vec![Id(0)], vec![Id(0), Id(0)], vec![Id(1)]];
        for a in &options {
            for b in &options {
                for xs in &lists {
                    for ys in &lists {
                        cases.push(case!(ordering_derive::compound(a.clone(), b.clone(), xs.clone(), ys.clone())));
                    }
                }
            }
        }
        pairs!(
            cases,
            ordering_derive::tuples,
            [(Level::Low, None), (Level::Low, Some(Id(0))), (Level::Mid, None), (Level::Mid, Some(Id(9)))]
        );
        let strs = |xs: &[Option<&str>]| xs.iter().map(|x| x.map(String::from)).collect::<Vec<_>>();
        let lists = [strs(&[]), strs(&[None]), strs(&[Some("")]), strs(&[Some("a"), None]), strs(&[Some("a")])];
        for a in [None, Some(0u8), Some(255)] {
            for b in [None, Some(0u8)] {
                for s in &lists {
                    for t in &lists {
                        cases.push(case!(ordering_derive::options(a, b, s.clone(), t.clone())));
                    }
                }
            }
        }
    });
}
