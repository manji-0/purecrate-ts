//! Match guards: order, a guard evaluated only when its pattern matched,
//! tuple scrutinees evaluated once, binding arms `n if ..`, overflow in a
//! guard, and `matches!` with a guard.

use crate::support;


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
        for rate in [Rate::Standard, Rate::Reduced, Rate::Exempt] {
            for amount in [0, 5, 6] {
                cases.push(case!(guards::wild_after(rate, amount)));
            }
        }
        for n in [0, 4, 5, 9, 10, 255] {
            for strict in [true, false] {
                cases.push(case!(guards::overlap(n, strict)));
            }
        }
        for state in [State::Open, State::Paid, State::Closed] {
            for n in [0, 50, 55, 56, 255] {
                cases.push(case!(guards::guarded_overflow(state, n)));
            }
        }
        for a in [true, false] {
            for b in [true, false] {
                for n in [0, 3, 4] {
                    cases.push(case!(guards::flags(a, b, n)));
                }
            }
        }
        for x in [None, Some(0), Some(7), Some(8)] {
            cases.push(case!(guards::only_in_guard(x)));
        }
        for s in ["", "a", "ab", "abc", "abcdefghi", "éé"] {
            cases.push(case!(guards::classify(s)));
        }
        cases
    });
    support::assert_equivalent("guards", SOURCE, &cases);
}

/// Random states, events, and values through the guarded decision trees.
#[test]
fn random_guards_match_rust() {
    use guards::{Event, Rate, State};
    let mut rng = support::Rng::new(0x0006_a2d5);
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for _ in 0..500 {
            let state = rng.pick(&[State::Open, State::Paid, State::Closed]);
            let rate = rng.pick(&[Rate::Standard, Rate::Reduced, Rate::Exempt]);
            let amount = rng.edgy(64, true) as i64;
            let event = match rng.below(3) {
                0 => Event::Pay { amount },
                1 => Event::Refund { amount },
                _ => Event::Cancel,
            };
            let n = rng.edgy(8, false) as u8;
            let (a, b) = (rng.below(2) == 0, rng.below(2) == 0);
            let (k, m) = (rng.below(6) as u32, rng.edgy(32, true) as i32);
            cases.push(case!(guards::step(state, event)));
            cases.push(case!(guards::tax(rate, amount)));
            cases.push(case!(guards::wild_after(rate, amount)));
            cases.push(case!(guards::overlap(n % 16, a)));
            cases.push(case!(guards::guarded_overflow(state, n)));
            cases.push(case!(guards::flags(a, b, k)));
            cases.push(case!(guards::bucket(m)));
        }
        cases
    });
    support::assert_equivalent("guards_random", SOURCE, &cases);
}

/// A field or payload a lowered `match` reads binds the arm's own name
/// directly, also when a guard reads it first: no `$f` or `$v` copy.
#[test]
fn a_lowered_match_binds_the_arms_names() {
    let source = "pub enum Shape { Circle(i32), Rect { w: i32, h: i32 } }\n\
                  pub fn area(s: Shape, big: bool) -> i32 {\n\
                      match s {\n\
                          Shape::Circle(r) if big => r * r * 3,\n\
                          Shape::Circle(r) => r,\n\
                          Shape::Rect { w, h } => w * h,\n\
                      }\n\
                  }\n\
                  pub fn first(o: Option<i32>, limit: i32) -> i32 {\n\
                      match o { Some(n) if n < limit => n, Some(_) => limit, None => 0 }\n\
                  }\n";
    let krate = purecrate_syntax::parse_source("binds", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    for stem in ["area", "first"] {
        let src = &pkg.files.iter().find(|f| f.stem == stem).expect(stem).source;
        assert!(!src.contains("$f") && !src.contains("$v"), "{src}");
    }
    let area = &pkg.files.iter().find(|f| f.stem == "area").expect("area").source;
    assert!(area.contains("const r = s.content[0];") && area.contains("const w = s.w;"), "{area}");
}
