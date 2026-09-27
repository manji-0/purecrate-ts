//! A state machine written with `let mut`, compound assignment, statement
//! `match`/`if`, and early `return`: every four-event run of
//! `fixtures/vending.rs` agrees between Rust and the generated package.

#[macro_use]
mod support;

#[allow(dead_code)]
mod vending {
    include!("fixtures/vending.rs");
}

const SOURCE: &str = include_str!("fixtures/vending.rs");

impl support::Show for vending::Fault {
    fn show(&self) -> String {
        match self {
            vending::Fault::SoldOut => "SoldOut".into(),
            vending::Fault::Short(n) => format!("Short({n})"),
            vending::Fault::Full => "Full".into(),
        }
    }
}

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
    support::assert_equivalent("vending", SOURCE, &cases);
}
