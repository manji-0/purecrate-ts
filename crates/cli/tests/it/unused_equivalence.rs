//! Bindings the Rust leaves unused, printed so that the package passes
//! `noUnusedLocals` and `noUnusedParameters` (its own tsconfig sets both),
//! with every value Rust evaluates still evaluated: an unused sum that
//! overflows panics on both sides.

use crate::support;

purecrate_canon::fixture!(mod unused = "fixtures/unused.rs");

#[test]
fn generated_code_with_unused_bindings_matches_rust() {
    support::equivalence("unused", unused::SOURCE, |cases| {
        use unused::Shape;
        let edges = [i32::MIN, -1, 0, 1, i32::MAX];
        grid!(
            cases, [unused::ignored_param, unused::underscored_param, unused::unused_let, unused::write_only];
            a in edges, b in edges
        );
        grid!(cases, unused::unused_closure_param; a in edges);
        grid!(cases, unused::unused_try; flag in [0, 4]);
        for s in [Shape::Circle { r: 2 }, Shape::Rect(3, 4), Shape::Empty] {
            cases.push(case!(unused::unused_arm(s.clone())));
        }
        let opts = [None, Some(0), Some(5)];
        grid!(cases, unused::unused_if_let; a in opts);
        grid!(cases, unused::unused_tuple_arm; a in opts, b in opts);
        grid!(cases, unused::unused_loop_var; n in [0u8, 3]);
        grid!(cases, unused::unused_char; s in ["", "aé😀"]);
    });
}

/// A statement with no effect is left out: an `if` whose sides do nothing,
/// a value only built, `x = x`; a `let mut` no longer written is a `const`.
#[test]
fn a_statement_with_no_effect_is_left_out() {
    let source = "pub struct P { pub a: i32 }\n\
                  #[allow(unused_assignments, unused_variables, unused_must_use)]\n\
                  pub fn kept(a: i32, s: &str) -> String {\n\
                      let mut last = 0i32;\n\
                      if a > 0 { last = 1; }\n\
                      Box::new(P { a });\n\
                      let mut t = String::from(s);\n\
                      t = String::from(&t);\n\
                      t\n\
                  }\n";
    let krate = purecrate_syntax::parse_source("effects", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let kept = &pkg.files.iter().find(|f| f.stem == "kept").expect("kept").source;
    assert!(!kept.contains("if (") && !kept.contains("({") && !kept.contains("let "), "{kept}");
}
