//! Type parameters: generic structs, enums, `impl<T>` blocks, and
//! functions, each instantiated at more than one type, `==` on generic
//! values, a recursive generic enum through `Box`, struct update on a
//! generic struct, and a generic struct held by a concrete one.

use crate::support;

purecrate_canon::fixture!(mod generics = "fixtures/generics.rs");

#[test]
fn generated_generics_match_rust() {
    support::equivalence("generics", generics::SOURCE, |cases| {
        for xs in [vec![], vec![1u32], vec![3, 1, 2]] {
            for total in [0u32, 5, u32::MAX] {
                cases.push(case!(generics::pages(xs.clone(), total)));
            }
        }
        let words: Vec<Vec<String>> = vec![vec![], vec!["é".into(), "a".into()]];
        for xs in [vec![], vec![-1i64, i64::MAX]] {
            for ws in words.clone() {
                cases.push(case!(generics::both(xs.clone(), ws)));
            }
        }
        for a in [0u8, 255] {
            for s in ["", "😀"] {
                cases.push(case!(generics::pairs(a, s.to_string())));
            }
        }
        for e in
            [generics::Either::Left(4i32), generics::Either::Left(i32::MAX), generics::Either::Right("abc".to_string())]
        {
            cases.push(case!(generics::side(e)));
        }
        for xs in [vec![], vec![5i32], vec![1, 2, 3, 4, 5]] {
            cases.push(case!(generics::grow(xs)));
        }
        let page = |items: Vec<u32>, total: u32| generics::Page { items, total };
        let names =
            |items: Vec<&str>| generics::Page { items: items.into_iter().map(String::from).collect(), total: 2 };
        for n in [None, Some(names(vec![])), Some(names(vec!["x", "y"]))] {
            cases.push(case!(generics::summarize(page(vec![1, 2], 9), n)));
        }
    });
}
