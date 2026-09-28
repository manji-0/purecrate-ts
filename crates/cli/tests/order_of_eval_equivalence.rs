//! Hoisting `?` and printing `..base` as a spread keep Rust's left-to-right
//! order: whichever of a panic, an early `Err`, or `None` comes first in
//! Rust comes first in TS, with the same panic message.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod order_of_eval = "fixtures/order_of_eval.rs");

const SOURCE: &str = order_of_eval::SOURCE;

#[test]
fn generated_evaluation_order_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in [1i32, 7, i32::MAX] {
            for z in [0i32, 2] {
                for fail in [false, true] {
                    cases.push(case!(order_of_eval::try_after_div(a, z, 3i32, fail)));
                    cases.push(case!(order_of_eval::try_in_second_arg(a, z, 3i32, fail)));
                    cases.push(case!(order_of_eval::update_before_try(a, z, 4i32, fail)));
                }
                cases.push(case!(order_of_eval::update_field_first(a, z)));
            }
        }
        for i in [0usize, 1, 5] {
            for none in [false, true] {
                cases.push(case!(order_of_eval::try_after_index(vec![10i32, 20], i, 3i32, none)));
            }
        }
        cases
    });
    support::assert_equivalent("order_of_eval", SOURCE, &cases);
}
