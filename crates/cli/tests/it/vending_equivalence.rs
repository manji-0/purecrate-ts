//! A state machine written with `let mut`, compound assignment, statement
//! `match`/`if`, and early `return`: every four-event run of
//! `fixtures/vending.rs` agrees between Rust and the generated package.

use crate::support;

purecrate_canon::fixture!(mod vending = "fixtures/vending.rs");

#[test]
fn generated_vending_machine_matches_rust() {
    let codes = 0u8..6;
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in codes.clone() {
            for b in codes.clone() {
                for c in codes.clone() {
                    for d in codes.clone() {
                        cases.push(case!(vending::run4(a, b, c, d)));
                    }
                }
            }
        }
        cases
    });
    support::assert_equivalent("vending", vending::SOURCE, &cases);
}
