//! Hoisting `?` and printing `..base` as a spread keep Rust's left-to-right
//! order: whichever of a panic, an early `Err`, or `None` comes first in
//! Rust comes first in TS, with the same panic message.

use crate::support;

purecrate_canon::fixture!(mod order_of_eval = "fixtures/order_of_eval.rs");

#[test]
fn generated_evaluation_order_matches_rust() {
    support::equivalence("order_of_eval", order_of_eval::SOURCE, |cases| {
        for a in [1i32, 7, i32::MAX] {
            for z in [0i32, 2] {
                for fail in [false, true] {
                    cases.push(case!(order_of_eval::try_after_div(a, z, 3i32, fail)));
                    cases.push(case!(order_of_eval::try_in_second_arg(a, z, 3i32, fail)));
                    cases.push(case!(order_of_eval::update_before_try(a, z, 4i32, fail)));
                }
                cases.push(case!(order_of_eval::update_field_first(a, z)));
            }
            for fail in [false, true] {
                cases.push(case!(order_of_eval::mapped_try_operand(a, 3i32, fail)));
                cases.push(case!(order_of_eval::try_in_ok_or(a, 3i32, fail)));
                cases.push(case!(order_of_eval::try_in_ok_or(a, -a, fail)));
                cases.push(case!(order_of_eval::two_mapped_tries(3i32, fail, false)));
                cases.push(case!(order_of_eval::two_mapped_tries(3i32, false, fail)));
                cases.push(case!(order_of_eval::try_in_default(None::<i32>, a, fail)));
                cases.push(case!(order_of_eval::try_in_default(Some(a), 3i32, fail)));
                cases.push(case!(order_of_eval::try_in_range_end(a, 3i32, fail)));
                for v in [0i32, 3] {
                    cases.push(case!(order_of_eval::try_in_ok_or_arg(a, v, fail)));
                    cases.push(case!(order_of_eval::try_in_ok_or_arg_operand(a, v, fail)));
                }
            }
        }
        for a in [i32::MIN, 0, 1, 9] {
            for fail in [false, true] {
                cases.push(case!(order_of_eval::mapped_try_in_ok_or_arg(a, 3i32, fail)));
                cases.push(case!(order_of_eval::mapped_try_in_ok_or_arg(i32::MAX, a, fail)));
            }
            cases.push(case!(order_of_eval::unwrap_or_in_ok_or(a)));
            cases.push(case!(order_of_eval::ok_or_scrutinee(Some(a), 4i32)));
            cases.push(case!(order_of_eval::ok_or_scrutinee(None::<i32>, a)));
            cases.push(case!(order_of_eval::annotated_scrutinee(a)));
            cases.push(case!(order_of_eval::folded_param(a, 2i32)));
        }
        grid!(cases, order_of_eval::reassigned_scrutinee; x in [None, Some(0u32), Some(9)]);
        for o in [None, Some(4i32), Some(i32::MIN)] {
            for fail in [false, true] {
                for v in [0i32, 3, i32::MIN] {
                    cases.push(case!(order_of_eval::try_in_ok_or_scrutinee(o, v, fail)));
                    cases.push(case!(order_of_eval::try_in_ok_or_test(o, v, fail)));
                }
            }
        }
        for r in [Ok(1i32), Err(2i32), Ok(i32::MAX)] {
            for c in [false, true] {
                cases.push(case!(order_of_eval::twice_named(r, 5i32, c)));
            }
        }
        for i in [0usize, 1, 5] {
            for none in [false, true] {
                cases.push(case!(order_of_eval::try_after_index(vec![10i32, 20], i, 3i32, none)));
            }
        }
    });
}
