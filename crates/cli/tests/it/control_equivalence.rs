//! Control flow over `Option` and `Result`: every case in
//! `fixtures/control.rs` agrees between Rust and the generated package.

use crate::support;

purecrate_canon::fixture!(mod control = "fixtures/control.rs");

const SOURCE: &str = control::SOURCE;

#[test]
fn generated_control_flow_matches_rust() {
    let cases = support::quietly(|| {
        vec![
            case!(control::or_zero(Some(5i32))),
            case!(control::or_zero(None::<i32>)),
            case!(control::none_first(Some(5i32))),
            case!(control::none_first(None::<i32>)),
            case!(control::doubled(Some(21i32))),
            case!(control::doubled(Some(i32::MAX))),
            case!(control::doubled(None::<i32>)),
            case!(control::is_none(Some(0i32))),
            case!(control::is_none(None::<i32>)),
            case!(control::safe_div(7i32, 2i32)),
            case!(control::safe_div(7i32, 0i32)),
            case!(control::div_or(7i32, 0i32, -9i32)),
            case!(control::div_or(-7i32, 2i32, -9i32)),
            case!(control::failed_numerator(7i32, 0i32)),
            case!(control::failed_numerator(7i32, 1i32)),
            case!(control::sum_both(Some(1i32), Some(2i32))),
            case!(control::sum_both(Some(1i32), None::<i32>)),
            case!(control::sum_both(None::<i32>, Some(2i32))),
            case!(control::bump(None::<i32>)),
            case!(control::bump(Some(i32::MAX))),
            case!(control::first_some(None::<i32>, Some(3i32))),
            case!(control::first_some(Some(0i32), Some(3i32))),
            case!(control::first_some(None::<i32>, None::<i32>)),
            case!(control::quarter(8i32)),
            case!(control::quarter(6i32)),
            case!(control::quarter(7i32)),
            case!(control::eighth_plus(16i32)),
            case!(control::eighth_plus(12i32)),
            case!(control::both_halves(4i32, 6i32)),
            case!(control::both_halves(3i32, 5i32)),
            case!(control::both_halves(4i32, 5i32)),
            case!(control::opt_add(Some(1i32), Some(2i32))),
            case!(control::opt_add(None::<i32>, Some(2i32))),
            case!(control::opt_add(Some(i32::MAX), Some(1i32))),
            case!(control::first_or_bail(Some(4i32))),
            case!(control::first_or_bail(None::<i32>)),
            case!(control::clamp_negative(-3i32)),
            case!(control::clamp_negative(3i32)),
            case!(control::chosen(true, Some(4i32))),
            case!(control::chosen(true, None::<i32>)),
            case!(control::chosen(false, None::<i32>)),
            case!(control::guarded(202i32)),
            case!(control::guarded(203i32)),
            case!(control::guarded(7i32)),
            case!(control::blocky(3i32)),
            case!(control::shadowed(3i32)),
            case!(control::add_twice(1u8, 2u8)),
            case!(control::add_twice(200u8, 30u8)),
            case!(control::count_positive(1i32, -1i32, 3i32)),
            case!(control::count_positive(0i32, 0i32, 0i32)),
            case!(control::latest(Some(1i32), Some(2i32))),
            case!(control::latest(Some(1i32), None::<i32>)),
            case!(control::latest(None::<i32>, None::<i32>)),
            case!(control::reassigned(true, Some(4i32))),
            case!(control::reassigned(false, None::<i32>)),
            case!(control::twice_matched(6i32, 3i32)),
            case!(control::twice_matched(6i32, 0i32)),
            case!(control::checked_first(7i32, 2i32)),
            case!(control::checked_first(7i32, 0i32)),
            case!(control::accumulate(4i32, 6i32)),
            case!(control::accumulate(4i32, 5i32)),
            case!(control::accumulate(3i32, 5i32)),
            case!(control::unit_ok(-1i32)),
            case!(control::unit_ok(1i32)),
            case!(control::unit_some(0i32)),
            case!(control::unit_some(1i32)),
        ]
    });
    support::assert_equivalent("control", SOURCE, &cases);
}
