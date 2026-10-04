//! `examples/order`, written inside the constraints of design/02, agrees
//! between Rust and the generated package on every four-command run, as the
//! summary and as the whole final `Order`.

use crate::support;

purecrate_canon::fixture!(mod order = "../../../examples/order/src/lib.rs", "fixtures/order_driver.rs");

#[test]
fn generated_order_lifecycle_matches_rust() {
    let codes = 0u8..10;
    let cases = support::cases(|cases| {
        grid!(
            cases, [order::run4, order::trace4];
            a in codes.clone(), b in codes.clone(), c in codes.clone(), d in codes.clone()
        );
        grid!(cases, order::open_with; code in 0u8..3);
        grid!(cases, order::edge; code in 0u8..4);
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
        "Err(OrderError::PriceMismatch)",
        "Err(OrderError::Overflow)",
    ] {
        assert!(cases.iter().any(|c| c.rust.starts_with(reached)), "no run reaches {reached}");
    }
    support::assert_equivalent("order", order::SOURCE, &cases);
}
