//! The shapes of `fixtures/review_holes.rs` match Rust; `verify.sh` lints
//! and lays out their output as the examples' is.

use crate::support;

purecrate_canon::fixture!(mod review_holes = "fixtures/review_holes.rs");

#[test]
fn review_holes_match_rust() {
    use review_holes::{E, P};
    support::equivalence("review_holes", review_holes::SOURCE, |cases| {
        for ps in [vec![], vec![P::A], vec![P::A, P::A, P::B, P::A]] {
            cases.push(case!(review_holes::counted(ps)));
        }
        let words = |ws: &[&str]| ws.iter().map(|w| w.to_string()).collect::<Vec<String>>();
        for a in [false, true] {
            for (x, y) in [(vec![], vec![]), (vec!["a"], vec!["a"]), (vec!["a"], vec!["b"]), (vec!["é"], vec![])] {
                cases.push(case!(review_holes::same(a, words(&x), words(&y))));
            }
            for n in [0u64, 1, 2, 4, 6, 7, 8, u64::MAX] {
                cases.push(case!(review_holes::mixed(a, n)));
                cases.push(case!(review_holes::either(a, n)));
            }
        }
        for n in [0u64, 1, 2, 10, 11, 29] {
            cases.push(case!(review_holes::digits(n)));
        }
        for xs in [vec![], vec![7u32, 8]] {
            cases.push(case!(review_holes::zeros(xs.clone())));
            cases.push(case!(review_holes::guarded(E::A, xs.clone())));
            cases.push(case!(review_holes::guarded(E::B, xs)));
        }
        cases.push(case!(review_holes::guarded(E::A, vec![1, 2, 3, 4])));
        for (e, c) in [(None, true), (Some(7i32), true), (Some(7), false), (Some(-1), true)] {
            cases.push(case!(review_holes::narrowed(e, c, 3)));
        }
        for b in [vec![], vec![b'A'], vec![b'B'], vec![b'8', 1], vec![b'w']] {
            cases.push(case!(review_holes::zero_low_bits(b)));
        }
        use review_holes::{Msg, Stage};
        for (st, ms) in [
            (Stage::Waiting { count: 4, done: false }, Msg::A),
            (Stage::Waiting { count: 4, done: true }, Msg::A),
            (Stage::Waiting { count: 4, done: true }, Msg::C(7)),
            (Stage::Waiting { count: 4, done: false }, Msg::C(7)),
            (Stage::Idle, Msg::B),
            (Stage::Idle, Msg::C(7)),
        ] {
            cases.push(case!(review_holes::unshadowed(st, ms)));
        }
        for (x, b) in [(63u32, 0u32), (1, 1), (2, 3)] {
            cases.push(case!(review_holes::wild(x, b)));
        }
        cases.push(case!(review_holes::scan(vec![1, 9, 2], vec![1, 5, 2, 3], 9)));
        cases.push(case!(review_holes::scan(Vec::<u32>::new(), vec![1], 9)));
        for c in [false, true] {
            cases.push(case!(review_holes::strictness(vec![1000000001u32], vec![2000000002u32], c)));
            cases.push(case!(review_holes::strictness(vec![3000000003u32], vec![2000000002u32], c)));
        }
        for x in [None, Some(3u32), Some(u32::MAX)] {
            cases.push(case!(review_holes::tuple_let(x)));
        }
        for s in [0i64, 59, 60, 61, -1] {
            cases.push(case!(review_holes::clamp(s)));
        }
    });
}
