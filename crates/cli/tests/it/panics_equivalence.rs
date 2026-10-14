//! `panic!`, `unreachable!`, `assert!`, and `Option::unwrap` / `expect`
//! throw Rust's message: an `assert!` with no message prints its condition
//! as rustc does, a message is built only when the panic happens, and
//! `expect`'s argument runs on `Some` too.

use crate::support;

purecrate_canon::fixture!(mod panics = "fixtures/panics.rs");

#[test]
fn generated_panics_match_rust() {
    support::equivalence("panics", panics::SOURCE, |cases| {
        use panics::Light::{Amber, Green, Red};
        grid!(cases, panics::half; n in [0u32, 1, 2, 7, u32::MAX - 1, u32::MAX]);
        grid!(cases, panics::within; n in [0i32, 5, -5, 9, -9, i32::MIN], cap in [0i32, 9, 10, i32::MAX]);
        grid!(cases, panics::lazy_message; n in [0u8, 199, 200, 254, 255]);
        let lists = [vec![], vec![0u8], vec![1], vec![2, 7], vec![9, 1, 1], vec![255, 7], vec![200, 1, 2, 3]];
        for xs in lists {
            for k in [0usize, 1, 3, 9] {
                cases.push(case!(panics::shown(xs.clone(), k)));
            }
        }
        grid!(cases, panics::calls; a in [0i64, 1, -1, 3, i64::MIN, i64::MAX], b in [0i64, 1, 2, -3, i64::MAX]);
        grid!(cases, panics::pick; k in [0u8, 1, 2, 3, 4, 6, 7, 255]);
        grid!(cases, panics::odd; n in [0u64, 1, 2, u64::MAX]);
        grid!(cases, panics::next; l in [Red, Amber, Green], stop in [false, true]);
        for xs in [vec![], vec![1i32, 2], vec![3, -4, 5], vec![i32::MAX, 1], vec![-1]] {
            cases.push(case!(panics::checked_sum(xs.clone())));
        }
        grid!(cases, panics::must; o in [None, Some(0i32), Some(i32::MAX)]);
        for o in [None, Some(String::new()), Some("é".to_string())] {
            for why in ["", "why: {}", "naïve 😀"] {
                cases.push(case!(panics::must_say(o.clone(), why)));
            }
        }
        grid!(cases, panics::must_literal; o in [None, Some(0u8), Some(255)]);
        grid!(cases, panics::must_built; o in [None, Some(3u8)], n in [0u8, 254, 255]);
    });
}
