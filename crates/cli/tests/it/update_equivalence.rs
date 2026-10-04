//! Struct update (`Pos { x: p.x + d, ..p }`): omitted fields come from the
//! base, and a failing `?` in an explicit field leaves before the base.

use crate::support;

purecrate_canon::fixture!(mod update = "fixtures/update.rs");

#[test]
fn generated_struct_update_matches_rust() {
    support::equivalence("update", update::SOURCE, |cases| {
        let coords = [-2i32, 0, 3, 40];
        for x in coords {
            for y in coords {
                for z in coords {
                    cases.push(case!(update::shift_x(x, y, z, 5)));
                    cases.push(case!(update::copy(x, y, z)));
                    cases.push(case!(update::over(x, y, z, 0, 0)));
                    cases.push(case!(update::over(x, y, z, 1, 2)));
                    cases.push(case!(update::over(x, y, z, 0, 2)));
                }
            }
        }
    });
}
