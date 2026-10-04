//! `match` on a tuple (`match (state, event)`): every case in
//! `fixtures/tuple_match.rs` agrees between Rust and the generated package,
//! including evaluation order and panics in the scrutinee.

use crate::support;

purecrate_canon::fixture!(mod tuple_match = "fixtures/tuple_match.rs");

#[test]
fn generated_tuple_matches_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for a in 0u8..6 {
            for b in 0u8..6 {
                for c in 0u8..6 {
                    for d in 0u8..6 {
                        cases.push(case!(tuple_match::run4(a, b, c, d)));
                    }
                }
            }
        }
        for a in [None, Some(0i64), Some(-7), Some(i64::MAX)] {
            for b in [None, Some(0u8), Some(255)] {
                for fail in [false, true] {
                    cases.push(case!(tuple_match::merge(a, b, fail)));
                }
            }
        }
        for x in [0i64, 1, 9, 10, -5, -1, -6, i64::MIN] {
            for y in [0u8, 1, 2, 3, 200, 255] {
                cases.push(case!(tuple_match::grid(x, y, 'q', "x")));
            }
        }
        for c in ['a', 'z', 'A', '\u{e000}', '\u{ffff}', '\u{10000}', '😀'] {
            for s in ["x", "", "xy"] {
                cases.push(case!(tuple_match::grid(0i64, 0u8, c, s)));
            }
        }
        for a in [0u8, 55, 56, 255] {
            for b in [0u8, 127, 128] {
                cases.push(case!(tuple_match::order(a, b)));
            }
        }
        for n in [0u32, 1, 9, 10, 11, 12, 99, u32::MAX] {
            cases.push(case!(tuple_match::digits(n)));
        }
        for hp in [0i32, 3, -3, 4, i32::MIN] {
            for code in 0u8..6 {
                cases.push(case!(tuple_match::hurt(hp, code)));
            }
        }
        for codes in [vec![], vec![0u8], vec![1, 1, 1], vec![0, 4, 5, 1, 2, 3], vec![1, 0, 1, 0]] {
            cases.push(case!(tuple_match::count(codes.clone())));
        }
        cases
    });
    support::assert_equivalent("tuple_match", tuple_match::SOURCE, &cases);
}

/// Every enum element is matched by a `switch` that names every variant and
/// ends in `assertNever`, so TS checks exhaustiveness itself.
#[test]
fn every_enum_element_ends_in_assert_never() {
    let krate = purecrate_syntax::parse_source("tuple_match", tuple_match::SOURCE).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let step = purecrate_pack::assemble(&typed).files.into_iter().find(|f| f.stem == "step").expect("step.ts").source;
    assert!(step.contains("switch (event.kind)"), "{step}");
    assert!(step.contains("return assertNever(event);"), "{step}");
    assert!(step.contains("switch (state.kind)"), "{step}");
    assert!(step.contains("return assertNever(state);"), "{step}");
    for v in ["Idle", "Running", "Paused", "Done"] {
        assert!(step.contains(&format!("case \"{v}\":")), "{v} missing:\n{step}");
    }
}

/// A `match` on a value a case has already narrowed is the arm it takes:
/// `(s, _) => if matches!(s, S::B) ..` under `case "A"` is its `else`, and
/// a case of `A | B` whose sides then differ is a case each.
#[test]
fn a_match_on_a_narrowed_value_is_the_arm_it_takes() {
    let source = "pub enum S { A, B, C }\n\
                  pub enum E { X, Y }\n\
                  pub fn step(s: S, e: E) -> i32 {\n\
                      match (s, e) {\n\
                          (S::C, E::X) => 0,\n\
                          (s, _) => if matches!(s, S::B) { 1 } else { 2 },\n\
                      }\n\
                  }\n";
    let krate = purecrate_syntax::parse_source("narrowed", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let step = &pkg.files.iter().find(|f| f.stem == "step").expect("step").source;
    assert!(!step.contains("as S") && !step.contains("=== \"B\""), "{step}");
    assert!(step.contains("case \"A\":\n      return 2 as I32;\n    case \"B\":\n      return 1 as I32;"), "{step}");
}
