//! 0.18.0's std methods: `Option`'s and `Result`'s combinators, with each
//! value argument evaluated on both sides and each function only on its
//! own; `get`, `first`, `last`, and `contains` (NaN, `-0.0`, structs) on a
//! `Vec`; `fold`, and `rev` and `zip` on a source, in `for` too, with
//! overflow at the first item Rust reaches.

use crate::support;

purecrate_canon::fixture!(mod std_more = "fixtures/std_more.rs");

#[test]
fn generated_std_methods_match_rust() {
    support::equivalence("std_more", std_more::SOURCE, |cases| {
        use std_more::Point;
        let opts = [None, Some(0i32), Some(3), Some(4), Some(44), Some(-8), Some(200), Some(i32::MIN), Some(i32::MAX)];
        for o in opts {
            cases.push(case!(std_more::chain(o)));
            cases.push(case!(std_more::lazy_fn(o)));
            cases.push(case!(std_more::keep_big(o)));
            cases.push(case!(std_more::big(o)));
            cases.push(case!(std_more::small_or_none(o)));
            for d in [0i32, 7, i32::MAX] {
                cases.push(case!(std_more::lazy(o, d)));
                cases.push(case!(std_more::either(o, d)));
                cases.push(case!(std_more::either_lazy(o, d)));
            }
        }
        grid!(cases, std_more::need; o in [None, Some(0u8), Some(254), Some(255)]);
        grid!(
            cases, std_more::other;
            o in [None, Some(1u8)], p in [None, Some(2u8), Some(255)], n in [0u8, 1, 200]
        );
        let texts = ["", "0", "7", "9", "10", "4294967295", "4294967296", "-1", "x", "é"];
        for s in texts {
            cases.push(case!(std_more::parsed(s)));
            cases.push(case!(std_more::parsed_or(s)));
            cases.push(case!(std_more::parsed_else(s)));
        }
        let results = [Ok(0u8), Ok(4), Ok(255), Err(String::new()), Err("abcd".to_string()), Err("ab".to_string())];
        for r in results.clone() {
            cases.push(case!(std_more::which(r.clone())));
            cases.push(case!(std_more::err_of(r.clone())));
            cases.push(case!(std_more::recover(r.clone())));
            for n in [0u16, 1, u16::MAX] {
                cases.push(case!(std_more::mapped_or(r.clone(), n)));
            }
        }
        for xs in [vec![], vec![5u16], vec![1, 2, 3]] {
            for i in [0usize, 1, 2, 3, usize::MAX] {
                cases.push(case!(std_more::at(xs.clone(), i)));
            }
        }
        for xs in [vec![], vec!["a".to_string()], vec!["a".to_string(), "é😀".to_string()]] {
            cases.push(case!(std_more::ends(xs.clone())));
        }
        for xs in [vec![], vec![9u8], vec![9, 8, 7]] {
            cases.push(case!(std_more::second_or(xs.clone())));
        }
        let p = |x: i32, l: &str| Point { x, label: l.to_string() };
        let points = [vec![], vec![p(1, "a")], vec![p(1, "a"), p(-3, "")]];
        for ps in points.clone() {
            cases.push(case!(std_more::last_x(ps.clone())));
            for q in [p(1, "a"), p(1, "b"), p(-3, "")] {
                cases.push(case!(std_more::has_point(ps.clone(), q)));
            }
        }
        for xs in [vec![], vec![0i64, -5, i64::MAX]] {
            for n in [0i64, -5, 1, i64::MAX] {
                cases.push(case!(std_more::has(xs.clone(), n)));
            }
        }
        for xs in [vec![], vec![0.0f64, f64::NAN, 1.5], vec![-0.0, f64::INFINITY]] {
            for n in [0.0f64, -0.0, f64::NAN, 1.5, f64::INFINITY] {
                cases.push(case!(std_more::has_float(xs.clone(), n)));
            }
        }
        for xs in [vec![], vec!["".to_string(), "é".to_string()]] {
            for s in ["", "é", "e"] {
                cases.push(case!(std_more::has_text(xs.clone(), s.to_string())));
            }
        }
        let lists = [vec![], vec![1u32], vec![1, 2, 3], vec![u32::MAX], vec![1 << 30, 1 << 30, 5]];
        for xs in lists {
            for start in [0u32, 7, u32::MAX] {
                cases.push(case!(std_more::total(xs.clone(), start)));
            }
        }
        for s in ["", "a1b2", "123", "18446744073709551615", "18446744073709551616", "٣4"] {
            cases.push(case!(std_more::digits(s)));
            cases.push(case!(std_more::back_text(s)));
            cases.push(case!(std_more::back_bytes(s)));
        }
        for s in ["é😀a", "\u{301}e"] {
            cases.push(case!(std_more::back_text(s)));
            cases.push(case!(std_more::back_bytes(s)));
        }
        let ints = [vec![], vec![1i32], vec![1, 60, 2], vec![i32::MAX, 1], vec![1, i32::MAX], vec![51, i32::MIN]];
        for xs in ints.clone() {
            cases.push(case!(std_more::backwards(xs.clone())));
            cases.push(case!(std_more::last_big(xs.clone())));
            cases.push(case!(std_more::indexed(xs.clone())));
            for ys in ints.clone() {
                cases.push(case!(std_more::dot(xs.clone(), ys.clone())));
            }
        }
        grid!(cases, std_more::same; a in ["", "abc", "aéc😀", "abd"], b in ["", "abc", "aXc😀z"]);
        for xs in [vec![], vec![1u8, 2], vec![3, 0, 4], vec![255, 255, 255, 255]] {
            cases.push(case!(std_more::walk(xs.clone())));
        }
        let texts: Vec<Vec<String>> =
            vec![vec![], vec!["x".into()], vec!["x".into(), "é".into(), "".into(), "z".into()]];
        for a in [vec![], vec![1u8, 2, 3]] {
            for b in texts.clone() {
                cases.push(case!(std_more::pairs(a.clone(), b)));
            }
        }
        for a in [vec![], vec![1u8, 255]] {
            for b in [vec![255u8], vec![1, 2, 3]] {
                cases.push(case!(std_more::zip_owned(a.clone(), b)));
            }
        }
    });
}
