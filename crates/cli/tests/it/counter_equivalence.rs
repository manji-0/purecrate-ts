//! v0 acceptance: the generated counter package returns the same `State` as the
//! Rust source for every `Event`, and panics where Rust does: at the ends of
//! `i32`, as a debug build does.

use crate::support;

purecrate_canon::fixture!(mod counter = "../../../examples/counter/src/lib.rs");

#[test]
fn generated_counter_matches_rust_step() {
    support::equivalence("counter", counter::SOURCE, |cases| {
        for n in (-3..=3).chain([-1000, 1000, i32::MIN, i32::MIN + 1, i32::MAX - 1, i32::MAX]) {
            for event in [counter::Event::Inc, counter::Event::Dec, counter::Event::Reset] {
                cases.push(case!(counter::step(counter::State { n }, event)));
            }
        }
    });
}

/// The header's rules: one step either way, `Reset` to zero from anywhere,
/// and a panic past either end of `i32` (the equivalence check above holds
/// the TS to the same panics).
#[test]
fn the_counter_panics_past_the_ends_of_i32() {
    let n = |state: counter::State| state.n;
    assert_eq!(n(counter::step(counter::State { n: i32::MAX - 1 }, counter::Event::Inc)), i32::MAX);
    assert_eq!(n(counter::step(counter::State { n: i32::MIN + 1 }, counter::Event::Dec)), i32::MIN);
    assert_eq!(n(counter::step(counter::State { n: i32::MAX }, counter::Event::Dec)), i32::MAX - 1);
    assert_eq!(n(counter::step(counter::State { n: i32::MIN }, counter::Event::Reset)), 0);
    let cases = support::cases(|cases| {
        cases.push(case!(counter::step(counter::State { n: i32::MAX }, counter::Event::Inc)));
        cases.push(case!(counter::step(counter::State { n: i32::MIN }, counter::Event::Dec)));
    });
    assert_eq!(cases[0].rust, "panic(attempt to add with overflow)");
    assert_eq!(cases[1].rust, "panic(attempt to subtract with overflow)");
}
