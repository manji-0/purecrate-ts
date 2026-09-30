//! Match guards: order, a guard evaluated only when its pattern matched,
//! tuple scrutinees evaluated once, binding arms `n if ..`, overflow in a
//! guard, and `matches!` with a guard.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod guards = "fixtures/guards.rs");

const SOURCE: &str = guards::SOURCE;

#[test]
fn generated_guards_match_rust() {
    use guards::{Event, Rate, State};
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for rate in [Rate::Standard, Rate::Reduced, Rate::Exempt] {
            for amount in [i64::MIN, -5, 0, 9_999, 10_000, i64::MAX / 10, i64::MAX] {
                cases.push(case!(guards::tax(rate, amount)));
            }
        }
        for state in [State::Open, State::Paid, State::Closed] {
            for event in [
                Event::Pay { amount: 1 },
                Event::Pay { amount: 0 },
                Event::Refund { amount: 5 },
                Event::Refund { amount: 2_000_000 },
                Event::Cancel,
            ] {
                cases.push(case!(guards::step(state, event.clone())));
            }
        }
        for rate in [Rate::Standard, Rate::Reduced, Rate::Exempt] {
            for amount in [0, 1, i32::MAX] {
                cases.push(case!(guards::only_when_matched(rate, amount)));
            }
        }
        for n in [i32::MIN, -1, 0, 1, 50, 51, i32::MAX / 2 + 1, i32::MAX] {
            cases.push(case!(guards::bucket(n)));
        }
        for xs in [vec![], vec![None, Some(3), Some(10), Some(255)]] {
            cases.push(case!(guards::guard_runs(xs.clone())));
        }
        for x in [None, Some(10), Some(12), Some(13), Some(u32::MAX - 1)] {
            cases.push(case!(guards::big_even(x)));
        }
        for s in ["", "a", "ab", "abc", "abcdefghi", "éé"] {
            cases.push(case!(guards::classify(s)));
        }
        cases
    });
    support::assert_equivalent("guards", SOURCE, &cases);
}
