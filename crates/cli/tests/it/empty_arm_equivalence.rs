//! An empty arm where the value is returned returns: it neither falls into
//! the next `case` nor runs the arm after it. A statement that never falls
//! through has no unreachable `return` after it.

use crate::support;

purecrate_canon::fixture!(mod empty_arm = "fixtures/empty_arm.rs");

const SOURCE: &str = empty_arm::SOURCE;

#[test]
fn generated_empty_arms_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for n in [0u8, 2, 255] {
            for s in ["", "x"] {
                cases.push(case!(empty_arm::unit_cases(empty_arm::M::A, s, n)));
            }
            cases.push(case!(empty_arm::unit_cases(empty_arm::M::B, "", n)));
            cases.push(case!(empty_arm::unit_cases(empty_arm::M::C, "", n)));
            for c in [false, true] {
                cases.push(case!(empty_arm::unit_if(c, n)));
            }
            cases.push(case!(empty_arm::all_return(Some(u32::from(n)))));
            cases.push(case!(empty_arm::spin(u32::from(n))));
        }
        cases.push(case!(empty_arm::all_return(None::<u32>)));
        cases
    });
    support::assert_equivalent("empty_arm", SOURCE, &cases);
}
