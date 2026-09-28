//! `examples/order`, written inside the constraints of design/07, agrees
//! between Rust and the generated package on every four-command run.

#[macro_use]
mod support;

#[allow(dead_code)]
mod order {
    include!("../../../examples/order/src/lib.rs");
    include!("fixtures/order_driver.rs");
}

const EXAMPLE: &str = include_str!("../../../examples/order/src/lib.rs");
const DRIVER: &str = include_str!("fixtures/order_driver.rs");

impl support::Show for order::OrderError {
    fn show(&self) -> String {
        match self {
            order::OrderError::QtyZero => "QtyZero".into(),
            order::OrderError::UnknownSku => "UnknownSku".into(),
            order::OrderError::Empty => "Empty".into(),
            order::OrderError::AmountMismatch { .. } => "AmountMismatch".into(),
            order::OrderError::EmptyTracking => "EmptyTracking".into(),
            order::OrderError::InvalidTransition => "InvalidTransition".into(),
        }
    }
}

/// The text each command code reads: a SKU for line commands, a tracking
/// number for `Ship`. Code 7 ships with an empty one.
fn text(code: u8) -> String {
    match code {
        1 => "b",
        7 => "",
        8 => "T1",
        _ => "a",
    }
    .to_string()
}

#[test]
fn generated_order_lifecycle_matches_rust() {
    let codes = 0u8..10;
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in codes.clone() {
            for b in codes.clone() {
                for c in codes.clone() {
                    for d in codes.clone() {
                        cases.push(case!(order::run4(a, text(a), b, text(b), c, text(c), d, text(d))));
                    }
                }
            }
        }
        cases
    });
    for reached in [
        "Ok(1000450)",
        "Ok(2000200)",
        "Ok(4000200)",
        "Ok(5000000)",
        "Err(QtyZero)",
        "Err(UnknownSku)",
        "Err(Empty)",
        "Err(AmountMismatch)",
        "Err(EmptyTracking)",
        "Err(InvalidTransition)",
    ] {
        assert!(cases.iter().any(|c| c.rust == reached), "no run reaches {reached}");
    }
    let source = format!("{EXAMPLE}\n{DRIVER}");
    support::assert_equivalent("order", &source, &cases);
}
