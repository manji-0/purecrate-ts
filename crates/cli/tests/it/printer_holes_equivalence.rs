//! What the printer got wrong while 0.25.0 was made, against Rust.

use crate::support;

purecrate_canon::fixture!(mod printer_holes = "fixtures/printer_holes.rs");

#[test]
fn printer_holes_match_rust() {
    support::equivalence("printer_holes", printer_holes::SOURCE, |cases| {
        for o in [None, Some(4), Some(-2)] {
            for k in [0, 3] {
                cases.push(case!(printer_holes::typed_in_block(o, k)));
            }
            for r in [Ok(2), Ok(9), Err(5)] {
                for c in [true, false] {
                    cases.push(case!(printer_holes::alias_tested_before(7, o, r, c)));
                    cases.push(case!(printer_holes::alias_of_ok_tested_before(-2, o, r, c)));
                }
            }
            for w in [None, Some(i64::MAX)] {
                for c in [true, false] {
                    cases.push(case!(printer_holes::widened_default(o, w, c)));
                }
            }
        }
    });
}
