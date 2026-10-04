//! Struct patterns in arms and `matches!` agree with Rust, and a field
//! pattern that binds inside a refutable one is refused.

use crate::support;

purecrate_canon::fixture!(mod pats = "fixtures/struct_patterns.rs");

#[test]
fn struct_patterns_match_rust() {
    use pats::{Kind, Method, Owner, Status};
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        let mut all = vec![Status::Done];
        for kind in [Kind::Card, Kind::Bank] {
            for id in [0u32, 4] {
                let method = Method { kind, id };
                all.push(Status::Processing { method });
                for age in [10u8, 18, 99, 120] {
                    all.push(Status::Held { owner: Owner { method, age } });
                }
            }
        }
        for s in all {
            cases.push(case!(pats::cancelable(s)));
            cases.push(case!(pats::picked(s)));
            cases.push(case!(pats::is_card(&s)));
            for go in [false, true] {
                cases.push(case!(pats::step(s, go)));
            }
        }
        for xs in [vec![], vec![1u32], vec![5]] {
            for b in [1u32, 20] {
                cases.push(case!(pats::shadowed(pats::Bag { a: 2, xs: xs.clone() }, b)));
            }
        }
        cases
    });
    support::assert_equivalent("struct_patterns", pats::SOURCE, &cases);
}

#[test]
fn a_binding_inside_a_refutable_field_is_refused() {
    let src = "pub enum K { A(u8), B }\npub struct M { pub kind: K }\npub fn f(m: M) -> u8 { match m { M { kind: K::A(x) } => x, _ => 0 } }\n";
    let krate = purecrate_syntax::parse_source("refused", src);
    let refused = match krate {
        Err(e) => format!("{e:?}"),
        Ok(k) => format!("{:?}", purecrate_check::accept(&k).err()),
    };
    assert!(refused.contains("binds nothing"), "{refused}");
}
