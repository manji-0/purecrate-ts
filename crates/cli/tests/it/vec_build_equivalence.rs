//! `vec![a, b]` built inside the crate: element types from context, empty
//! and nested lists, struct fields, and left-to-right evaluation (the first
//! overflow is the one reported) agree between Rust and the generated
//! package.

use crate::support;

purecrate_canon::fixture!(mod vec_build = "fixtures/vec_build.rs");

#[test]
fn generated_vec_literals_match_rust() {
    use vec_build::Strength;
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in [Strength::Password, Strength::PasswordAndOtp] {
            cases.push(case!(vec_build::amr(s)));
            cases.push(case!(vec_build::issue(String::from("alice"), s)));
        }
        cases.push(case!(vec_build::wide()));
        for n in [0u8, 1, 2, 255] {
            cases.push(case!(vec_build::upto(n)));
        }
        let edges = [i32::MIN, -1, 0, 1, 2, 46_341, i32::MAX];
        for a in edges {
            for b in edges {
                cases.push(case!(vec_build::nested(a, b)));
                cases.push(case!(vec_build::ordered(a, b)));
            }
        }
        for (a, b) in [('a', 'b'), ('\0', '😀'), ('é', 'é')] {
            cases.push(case!(vec_build::reread(a, b)));
        }
        for x in [None, Some(String::new()), Some(String::from("x"))] {
            for y in [None, Some(String::from("y"))] {
                cases.push(case!(vec_build::optional(x.clone(), y.clone())));
            }
        }
        cases
    });
    support::assert_equivalent("vec_build", vec_build::SOURCE, &cases);
}
