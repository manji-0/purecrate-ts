//! A local `Vec` grown by `push`, and `clone`, agree with Rust; the caller's
//! arrays come back unchanged; a closure that reads a grown local and a
//! push through a field are refused.

use crate::support;

purecrate_canon::fixture!(mod grow = "fixtures/grow.rs");

const SOURCE: &str = grow::SOURCE;

#[test]
fn grown_vecs_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for n in [0u32, 1, 5] {
            cases.push(case!(grow::evens(n)));
        }
        for xs in [vec![], vec![3u32], vec![1, 2, 3]] {
            cases.push(case!(grow::appended(xs.clone(), 9u32)));
            cases.push(case!(grow::through(xs.clone())));
            for again in [false, true] {
                cases.push(case!(grow::restarted(xs.clone(), again)));
                cases.push(case!(grow::chosen(xs.clone(), again)));
            }
        }
        for xs in [vec![], vec![1u8, 2, 3], vec![200, 50, 10]] {
            cases.push(case!(grow::running(xs)));
        }
        for s in [None, Some(String::from("abc"))] {
            cases.push(case!(grow::copied_name(&s)));
            cases.push(case!(grow::name_len(&s)));
        }
        cases
    });
    support::assert_equivalent("grow", SOURCE, &cases);
}

fn refused(src: &str) -> String {
    match purecrate_syntax::parse_source("refused", src) {
        Err(e) => format!("{e:?}"),
        Ok(k) => format!("{:?}", purecrate_check::accept(&k).err()),
    }
}

#[test]
fn a_closure_over_a_grown_local_and_a_field_push_are_refused() {
    let closure = refused(
        "pub fn f() -> usize { let mut v: Vec<u8> = vec![]; v.push(1); let n = |i: usize| v.len() + i; n(1) }\n",
    );
    assert!(closure.contains("cannot capture `let mut v`"), "{closure}");
    let field = refused(
        "pub struct S { pub items: Vec<u8> }\npub fn f(s: S) -> S { let mut s = s; s.items.push(1); s }\n",
    );
    assert!(field.contains("not a field or an element"), "{field}");
}
