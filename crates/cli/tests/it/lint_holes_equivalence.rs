//! Shapes whose output oxlint refused, against Rust.

use crate::support;

purecrate_canon::fixture!(mod lint_holes = "fixtures/lint_holes.rs");

#[test]
fn lint_holes_match_rust() {
    support::equivalence("lint_holes", lint_holes::SOURCE, |cases| {
        for (a, b) in [(5, 0), (5, 3), (i32::MAX, 1), (i32::MIN, -1), (-7, 9)] {
            for c in [true, false] {
                cases.push(case!(lint_holes::unread_tests(a, b, c)));
                cases.push(case!(lint_holes::reads_itself(a, b, c)));
                cases.push(case!(lint_holes::sides_run(a, b, c)));
                cases.push(case!(lint_holes::turned_round(a, c)));
                cases.push(case!(lint_holes::else_guard(a, c)));
                cases.push(case!(lint_holes::closure_param_decided(a, b, c)));
                for o in [None, Some(4)] {
                    for r in [Ok(2), Err(3)] {
                        cases.push(case!(lint_holes::keeps_itself(a, o, r, c)));
                    }
                }
            }
            cases.push(case!(lint_holes::emptied_loop(a, b)));
            cases.push(case!(lint_holes::param_unread(a, b)));
            for r in [Ok(2), Err(3)] {
                cases.push(case!(lint_holes::decided_return(r, a)));
                cases.push(case!(lint_holes::unread_try(a, b, r)));
                cases.push(case!(lint_holes::unread_try_last(a, r)));
            }
        }
        for (x, n) in [(1.5, 3i64), (-0.0, -9), (f64::NAN, i64::MAX), (f64::INFINITY, i64::MIN)] {
            cases.push(case!(lint_holes::literal_to_float(x, n)));
            cases.push(case!(lint_holes::small_big_float(n)));
            for m in [0u32, 7, u32::MAX] {
                cases.push(case!(lint_holes::unread_float(x, 2.0, m, vec![1, 2])));
                for c in [true, false] {
                    cases.push(case!(lint_holes::choice_cast(c, m)));
                }
            }
        }
        for b in [0u8, 91, 255] {
            cases.push(case!(lint_holes::byte_widened(b)));
        }
        for k in [0, 255, -1, i32::MAX] {
            cases.push(case!(lint_holes::literal_cast(k)));
        }
        for a in [3, i32::MAX, -9] {
            for o in [None, Some(4), Some(-7)] {
                for r in [Ok(2), Err(3), Err(i32::MAX)] {
                    for c in [true, false] {
                        cases.push(case!(lint_holes::keeps_on_a_side(a, o, r, c)));
                    }
                    cases.push(case!(lint_holes::emptied_named_loop(a, r)));
                }
            }
            for c in [true, false] {
                cases.push(case!(lint_holes::loop_value2(a, c)));
            }
        }
        for (i, k) in [(0usize, 0), (1, 5), (7, -1)] {
            for c in [true, false] {
                cases.push(case!(lint_holes::guards_itself(vec![3, 9], i, k, c)));
            }
        }
        for (a, z) in [(1.5f32, 0.75f32), (2.0, 3.0), (f32::NAN, 1.0), (0.0, 1.0)] {
            cases.push(case!(lint_holes::choice_of_numbers(a, z)));
        }
        let px = |c: u8| lint_holes::Px { c, f: 1.5 };
        cases.push(case!(lint_holes::deep_literal(vec![px(3), px(4), px(3)])));
        for k in [0, i32::MAX] {
            cases.push(case!(lint_holes::unread_collect(vec![px(3), px(200)], k)));
            cases.push(case!(lint_holes::unread_collect(vec![px(3)], k)));
        }
    });
}
