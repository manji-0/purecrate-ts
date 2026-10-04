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
        cases
    });
    support::assert_equivalent("narrowing", SOURCE, &cases);
}
