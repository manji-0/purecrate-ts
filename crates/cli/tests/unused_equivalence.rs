//! Bindings the Rust leaves unused, printed so that the package passes
//! `noUnusedLocals` and `noUnusedParameters` (its own tsconfig sets both),
//! with every value Rust evaluates still evaluated: an unused sum that
//! overflows panics on both sides.

#[macro_use]
mod support;

purecrate_canon::fixture!(mod unused = "fixtures/unused.rs");

const SOURCE: &str = unused::SOURCE;

#[test]
fn generated_code_with_unused_bindings_matches_rust() {
    use unused::Shape;
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        let edges = [i32::MIN, -1, 0, 1, i32::MAX];
        for a in edges {
            for b in edges {
                cases.push(case!(unused::ignored_param(a, b)));
                cases.push(case!(unused::underscored_param(a, b)));
                cases.push(case!(unused::unused_let(a, b)));
                cases.push(case!(unused::write_only(a, b)));
            }
            cases.push(case!(unused::unused_closure_param(a)));
        }
        for flag in [0, 4] {
            cases.push(case!(unused::unused_try(flag)));
        }
        for s in [Shape::Circle { r: 2 }, Shape::Rect(3, 4), Shape::Empty] {
            cases.push(case!(unused::unused_arm(s.clone())));
        }
        let opts = [None, Some(0), Some(5)];
        for a in opts {
            cases.push(case!(unused::unused_if_let(a)));
            for b in opts {
                cases.push(case!(unused::unused_tuple_arm(a, b)));
            }
        }
        for n in [0u8, 3] {
            cases.push(case!(unused::unused_loop_var(n)));
        }
        for s in ["", "aé😀"] {
            cases.push(case!(unused::unused_char(s)));
        }
        cases
    });
    support::assert_equivalent("unused", SOURCE, &cases);
}
