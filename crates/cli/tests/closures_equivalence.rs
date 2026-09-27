//! Local closures (captures of immutable bindings, nesting, shadowing, `?`
//! returning from the closure): every case in `fixtures/closures.rs` agrees
//! between Rust and the generated package.

#[macro_use]
mod support;

#[allow(dead_code)]
mod closures {
    include!("fixtures/closures.rs");
}

const SOURCE: &str = include_str!("fixtures/closures.rs");

#[test]
fn generated_closures_match_rust() {
    let xs = [-5i32, -1, 0, 1, 4, 6, 8, 12, 1001, i32::MAX];
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for x in xs {
            cases.push(case!(closures::scaled(x)));
            cases.push(case!(closures::blocky(x)));
            cases.push(case!(closures::nested(x)));
            cases.push(case!(closures::shadowing(x)));
            cases.push(case!(closures::local_wins(x)));
            cases.push(case!(closures::tried(x)));
            cases.push(case!(closures::quartered(x)));
            for y in xs {
                cases.push(case!(closures::add_twice(x, y)));
                cases.push(case!(closures::signs(x, y)));
            }
        }
        cases
    });
    support::assert_equivalent("closures", SOURCE, &cases);
}
