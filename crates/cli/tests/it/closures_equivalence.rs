//! Local closures (captures of immutable bindings, nesting, shadowing, `?`
//! returning from the closure): every case in `fixtures/closures.rs` agrees
//! between Rust and the generated package.

use crate::support;

purecrate_canon::fixture!(mod closures = "fixtures/closures.rs");

#[test]
fn generated_closures_match_rust() {
    support::equivalence("closures", closures::SOURCE, |cases| {
        let xs = [-5i32, -1, 0, 1, 4, 6, 8, 12, 1001, i32::MAX];
        grid!(
            cases,
            [
                closures::scaled,
                closures::blocky,
                closures::nested,
                closures::shadowing,
                closures::local_wins,
                closures::tried,
                closures::quartered,
            ];
            x in xs
        );
        grid!(cases, [closures::add_twice, closures::signs]; x in xs, y in xs);
    });
}
