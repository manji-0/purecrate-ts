//! `vec![a, b]` built inside the crate: element types from context, empty
//! and nested lists, struct fields, and left-to-right evaluation (the first
//! overflow is the one reported) agree between Rust and the generated
//! package.

use crate::support;

purecrate_canon::fixture!(mod vec_build = "fixtures/vec_build.rs");

#[test]
fn generated_vec_literals_match_rust() {
    support::equivalence("vec_build", vec_build::SOURCE, |cases| {
        use vec_build::Strength;
        for s in [Strength::Password, Strength::PasswordAndOtp] {
            cases.push(case!(vec_build::amr(s)));
            cases.push(case!(vec_build::issue(String::from("alice"), s)));
        }
        cases.push(case!(vec_build::wide()));
        grid!(cases, vec_build::upto; n in [0u8, 1, 2, 255]);
        let edges = [i32::MIN, -1, 0, 1, 2, 46_341, i32::MAX];
        grid!(cases, [vec_build::nested, vec_build::ordered]; a in edges, b in edges);
        for (a, b) in [('a', 'b'), ('\0', '😀'), ('é', 'é')] {
            cases.push(case!(vec_build::reread(a, b)));
        }
        for x in [None, Some(String::new()), Some(String::from("x"))] {
            for y in [None, Some(String::from("y"))] {
                cases.push(case!(vec_build::optional(x.clone(), y.clone())));
            }
        }
    });
}
