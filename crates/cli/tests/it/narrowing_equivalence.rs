//! Tests TS has decided by narrowing are folded, and only where nothing
//! writes the place.

use crate::support;

purecrate_canon::fixture!(mod narrowing = "fixtures/narrowing.rs");

const SOURCE: &str = narrowing::SOURCE;

#[test]
fn generated_narrowing_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        let results = [Ok(3i32), Err(4i32), Ok(i32::MAX), Err(i32::MIN)];
        for r in results {
            for b in [0i32, 5, i32::MAX] {
                cases.push(case!(narrowing::rematch(r, b)));
                cases.push(case!(narrowing::past_mapped_try(r, b, 2i32)));
            }
            cases.push(case!(narrowing::read_in_map_err(r)));
            for s in [Ok(1u32), Err(2u32)] {
                let r = r.map(|v| v.unsigned_abs()).map_err(|e| e.unsigned_abs() % 100);
                cases.push(case!(narrowing::written_past_mapped_try(r, s)));
            }
        }
        for o in [None, Some(2i32), Some(i32::MAX)] {
            cases.push(case!(narrowing::past_try(o, 5i32)));
            for c in [false, true] {
                cases.push(case!(narrowing::decided_guard(o, c)));
            }
        }
        for x in [None, Some(1u32), Some(u32::MAX)] {
            for y in [None, Some(4u32)] {
                cases.push(case!(narrowing::written_past_try(x, y)));
            }
        }
        for a in [0i32, 7, i32::MAX] {
            for c in [false, true] {
                cases.push(case!(narrowing::bound_constructor(a, c)));
            }
        }
        for r in [Ok(3i32), Err(4i32), Ok(i32::MAX)] {
            for b in [2i32, 50_000] {
                cases.push(case!(narrowing::unread_default(r, b)));
                cases.push(case!(narrowing::known_err_try(r, b)));
                cases.push(case!(narrowing::known_err_other_ok(r)));
            }
            for o in [None, Some(5i32), Some(i32::MIN)] {
                for a in [0i32, 2, i32::MIN] {
                    for c in [false, true] {
                        cases.push(case!(narrowing::decided_declaring(a, 3i32, o, r, c)));
                    }
                }
            }
        }
        for o in [None, Some(5i32)] {
            for n in [0i32, 1, 200] {
                cases.push(case!(narrowing::decided_loop(o, 3i32, n)));
            }
        }
        for r in [Ok(3i32), Err(4i32), Ok(i32::MAX), Err(i32::MIN)] {
            for n in [0i32, 1, 3] {
                cases.push(case!(narrowing::past_break(r, n)));
            }
            for c in [false, true] {
                for b in [2i32, 50_000] {
                    cases.push(case!(narrowing::same_sides(r, c, b)));
                    cases.push(case!(narrowing::decided_scrutinee(r, c, b)));
                    cases.push(case!(narrowing::decided_receiver(r, c, b)));
                }
            }
        }
        for c in [false, true] {
            for d in [false, true] {
                cases.push(case!(narrowing::decided_bools(c, d, 5i32)));
                cases.push(case!(narrowing::decided_bools(c, d, i32::MAX)));
            }
            for k in [-1i32, 0, 3] {
                cases.push(case!(narrowing::past_returning_if(c, k)));
            }
            for n in [0i32, 1, 200] {
                cases.push(case!(narrowing::decided_redeclared(c, n)));
            }
            cases.push(case!(narrowing::equal_constructors(c, 9i32)));
            for o in [None, Some(1i32), Some(50)] {
                cases.push(case!(narrowing::payload_test(o)));
            }
            cases.push(case!(narrowing::unread_as_str("é,a", String::from("x"), vec![1i32])));
            for r in [Ok(3i32), Err(4i32), Ok(i32::MAX), Err(i32::MIN)] {
                for b in [0i32, 5] {
                    cases.push(case!(narrowing::joined_sides(r, c, b)));
                    cases.push(case!(narrowing::narrowed_through_loop(r, b + 3)));
                }
                cases.push(case!(narrowing::decided_try(r, c)));
            }
            for o in [None, Some(1i32)] {
                for n in [0i32, 3] {
                    cases.push(case!(narrowing::decided_and(c, o, n)));
                }
            }
            for a in [0i32, 7] {
                cases.push(case!(narrowing::decided_and_runs_left(c, a)));
            }
        }
        for r in [Ok(3i32), Err(4i32), Ok(i32::MAX)] {
            for b in [2i32, 50_000] {
                cases.push(case!(narrowing::unread_default(r, b)));
                cases.push(case!(narrowing::known_err_try(r, b)));
                cases.push(case!(narrowing::known_err_other_ok(r)));
            }
            for o in [None, Some(5i32), Some(i32::MIN)] {
                for a in [0i32, 2, i32::MIN] {
                    for c in [false, true] {
                        cases.push(case!(narrowing::decided_declaring(a, 3i32, o, r, c)));
                    }
                }
            }
        }
        for o in [None, Some(5i32)] {
            for n in [0i32, 1, 200] {
                cases.push(case!(narrowing::decided_loop(o, 3i32, n)));
            }
        }
        for r in [Ok(3i32), Err(4i32), Ok(i32::MAX), Err(i32::MIN)] {
            for n in [0i32, 1, 3] {
                cases.push(case!(narrowing::past_break(r, n)));
            }
            for c in [false, true] {
                for b in [2i32, 50_000] {
                    cases.push(case!(narrowing::same_sides(r, c, b)));
                    cases.push(case!(narrowing::decided_scrutinee(r, c, b)));
                    cases.push(case!(narrowing::decided_receiver(r, c, b)));
                }
            }
        }
        for c in [false, true] {
            for d in [false, true] {
                cases.push(case!(narrowing::decided_bools(c, d, 5i32)));
                cases.push(case!(narrowing::decided_bools(c, d, i32::MAX)));
            }
            for k in [-1i32, 0, 3] {
                cases.push(case!(narrowing::past_returning_if(c, k)));
            }
            for n in [0i32, 1, 200] {
                cases.push(case!(narrowing::decided_redeclared(c, n)));
            }
            cases.push(case!(narrowing::equal_constructors(c, 9i32)));
            for o in [None, Some(1i32), Some(50)] {
                cases.push(case!(narrowing::payload_test(o)));
            }
            cases.push(case!(narrowing::unread_as_str("é,a", String::from("x"), vec![1i32])));
        }
        cases
    });
    support::assert_equivalent("narrowing", SOURCE, &cases);
}
