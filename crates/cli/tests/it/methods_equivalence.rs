//! Receiver-syntax calls to the crate's own methods (`card.beats(&other)`,
//! chains through fields and newtypes): every case in `fixtures/methods.rs`
//! agrees between Rust and the generated package.

use crate::support;


purecrate_canon::fixture!(mod methods = "fixtures/methods.rs");

const SOURCE: &str = methods::SOURCE;

#[test]
fn generated_method_calls_match_rust() {
    let ranks = [2u8, 10, 11, 13];
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in ranks {
            for b in ranks {
                for a_red in [false, true] {
                    for b_red in [false, true] {
                        cases.push(case!(methods::play(a, a_red, b, b_red)));
                    }
                }
                cases.push(case!(methods::total(a, b)));
            }
        }
        cases.push(case!(methods::total(250u8, 3u8)));
        cases
    });
    support::assert_equivalent("methods", SOURCE, &cases);
}
