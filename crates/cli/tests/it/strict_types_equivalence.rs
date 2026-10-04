//! The generated package type-checks under `tsc --strict` where TS cannot
//! infer the type Rust knows: `Ok`/`Err` built without a declared type,
//! tuples, and variant literals from different branches.

use crate::support;

purecrate_canon::fixture!(mod strict_types = "fixtures/strict_types.rs");

const SOURCE: &str = strict_types::SOURCE;

#[test]
fn generated_types_check_and_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for c in [false, true] {
            cases.push(case!(strict_types::try_on_if(c)));
            cases.push(case!(strict_types::variant_if(c, 5i32)));
            cases.push(case!(strict_types::option_if(c, 5i32)));
        }
        cases.push(case!(strict_types::match_on_ok(3i32)));
        cases.push(case!(strict_types::first(3i32)));
        cases
    });
    support::assert_equivalent("strict_types", SOURCE, &cases);
}
