//! `examples/order`, written inside the constraints of design/02, agrees
//! between Rust and the generated package on every four-command run, as the
//! summary and as the whole final `Order`.

use crate::support;


purecrate_canon::fixture!(mod order = "../../../examples/order/src/lib.rs", "fixtures/order_driver.rs");

const SOURCE: &str = order::SOURCE;

#[test]
fn generated_order_lifecycle_matches_rust() {
    let codes = 0u8..10;
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in codes.clone() {
            for b in codes.clone() {
                for c in codes.clone() {
                    for d in codes.clone() {
                        cases.push(case!(order::run4(a, b, c, d)));
                        cases.push(case!(order::trace4(a, b, c, d)));
                    }
                }
            }
        }
        for code in 0u8..3 {
            cases.push(case!(order::open_with(code)));
        }
        cases
    });
    for reached in [
        "Ok(1000450)",
        "Ok(2000200)",
        "Ok(4000200)",
        "Ok(5000000)",
        "Err(OrderError::QtyZero)",
        "Err(OrderError::UnknownSku)",
        "Err(OrderError::Empty)",
        "Err(OrderError::AmountMismatch { expected: Yen(",
        "Err(OrderError::EmptyTracking)",
        "Err(OrderError::InvalidTransition)",
        "Err(OrderError::EmptySku)",
        "Err(OrderError::NegativeAmount)",
        "Ok(1000300)",
    ] {
        assert!(cases.iter().any(|c| c.rust.starts_with(reached)), "no run reaches {reached}");
    }
    support::assert_equivalent("order", SOURCE, &cases);
}
