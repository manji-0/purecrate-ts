//! v0 acceptance: the generated counter package returns the same `State` as the
//! Rust source for every `Event`.

use crate::support;

purecrate_canon::fixture!(mod counter = "../../../examples/counter/src/lib.rs");

#[test]
fn generated_counter_matches_rust_step() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for n in (-3..=3).chain([-1000, 1000]) {
            for event in [counter::Event::Inc, counter::Event::Dec, counter::Event::Reset] {
                cases.push(case!(counter::step(counter::State { n }, event)));
            }
        }
        cases
    });
    support::assert_equivalent("counter", counter::SOURCE, &cases);
}
