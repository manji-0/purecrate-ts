//! A trailing `_` arm and binding-free `A | B` arms (design/02 §3.5): every
//! case in `fixtures/rest.rs` agrees between Rust and the generated package,
//! on enums with unit, tuple and struct variants, on `Option` and `Result`,
//! in statement and value position, and nested.

use crate::support;

purecrate_canon::fixture!(mod rest = "fixtures/rest.rs");

#[test]
fn generated_rest_arms_match_rust() {
    support::equivalence("rest", rest::SOURCE, |cases| {
        grid!(cases, [rest::area, rest::corners, rest::stops, rest::next]; a in 0i32..4);
        grid!(cases, [rest::nested, rest::matched]; a in 0i32..4, b in 0i32..4);
        grid!(cases, rest::options; x in [-3i32, -2, 0, 1, 2, 5, 8]);
    });
}
