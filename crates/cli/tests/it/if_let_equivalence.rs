//! `if let` and `let`-`else` on variants (in `Some` too), literals, and
//! ranges, with `mut` and several names, `continue` and `break`; a tuple's
//! `.0` / `.1`, nested.

use crate::support;

purecrate_canon::fixture!(mod if_let = "fixtures/if_let.rs");

#[test]
fn generated_if_let_and_let_else_match_rust() {
    support::equivalence("if_let", if_let::SOURCE, |cases| {
        use if_let::Shape::{self, Circle, Empty, Pair, Rect};
        let shapes = [
            Circle(0),
            Circle(3),
            Circle(40_000),
            Rect { w: 2, h: 3 },
            Rect { w: 65_536, h: 65_536 },
            Pair(7, 9),
            Pair(255, 255),
            Pair(0, 1),
            Empty,
        ];
        for s in shapes {
            cases.push(case!(if_let::radius(s)));
            cases.push(case!(if_let::area(s)));
            cases.push(case!(if_let::is_empty(s)));
            cases.push(case!(if_let::inner(Some(s))));
            cases.push(case!(if_let::inner_match(Some(s))));
            cases.push(case!(if_let::dims(s)));
            cases.push(case!(if_let::pair_sum(s)));
            cases.push(case!(if_let::only_empty(s)));
            cases.push(case!(if_let::checked(3, s)));
            cases.push(case!(if_let::checked(255, s)));
        }
        cases.push(case!(if_let::inner(None::<Shape>)));
        cases.push(case!(if_let::inner_match(None::<Shape>)));
        grid!(cases, if_let::small; n in [0u8, 5, 9, 10, 100, 254, 255]);
        grid!(cases, if_let::first_char; s in ["", "abc", "ABC", "aé{z", "z"]);
        grid!(cases, if_let::parse_pair; s in ["", "1", "1,2", ",2", "1,", "x,2", "4294967295,4294967294", "1,4294967295"]);
        let lists: Vec<Vec<Shape>> = vec![
            vec![],
            vec![Circle(1), Empty, Circle(2)],
            vec![Empty, Circle(0), Circle(5)],
            vec![Circle(4_294_967_295), Circle(1)],
            vec![Pair(1, 2), Rect { w: 4, h: 5 }, Rect { w: 6, h: 7 }],
        ];
        for xs in lists {
            cases.push(case!(if_let::sum_circles(xs.clone())));
            cases.push(case!(if_let::first_rect(xs.clone())));
        }
        for xs in [vec![], vec![3u8], vec![1, 9, 9, 2], vec![255, 0, 255]] {
            cases.push(case!(if_let::widest(xs.clone())));
        }
        grid!(cases, if_let::nested; a in [0u8, 200], b in [false, true], s in ["", "héllo"]);
        for xs in [vec![], vec![(2u8, 3u8)], vec![(255, 255), (1, 0)]] {
            cases.push(case!(if_let::pairs(xs.clone())));
        }
    });
}
