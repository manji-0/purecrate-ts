//! A trailing `_` arm and binding-free `A | B` arms (design/02 §3.5): every
//! case in `fixtures/rest.rs` agrees between Rust and the generated package,
//! on enums with unit, tuple and struct variants, on `Option` and `Result`,
//! in statement and value position, and nested.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod rest = "fixtures/rest.rs");

const SOURCE: &str = rest::SOURCE;

#[test]
fn generated_rest_arms_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in 0i32..4 {
            cases.push(case!(rest::area(a)));
            cases.push(case!(rest::corners(a)));
            cases.push(case!(rest::stops(a)));
            cases.push(case!(rest::next(a)));
            for b in 0i32..4 {
                cases.push(case!(rest::nested(a, b)));
                cases.push(case!(rest::matched(a, b)));
            }
        }
        for x in [-3i32, -2, 0, 1, 2, 5, 8] {
            cases.push(case!(rest::options(x)));
        }
        cases
    });
    support::assert_equivalent("rest", SOURCE, &cases);
}
