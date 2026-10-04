//! A state machine written with `let mut`, compound assignment, statement
//! `match`/`if`, and early `return`: every four-event run of
//! `fixtures/vending.rs` agrees between Rust and the generated package.

use crate::support;

purecrate_canon::fixture!(mod vending = "fixtures/vending.rs");

#[test]
fn generated_vending_machine_matches_rust() {
    support::equivalence("vending", vending::SOURCE, |cases| {
        let codes = 0u8..6;
        grid!(cases, vending::run4; a in codes.clone(), b in codes.clone(), c in codes.clone(), d in codes.clone());
    });
}
