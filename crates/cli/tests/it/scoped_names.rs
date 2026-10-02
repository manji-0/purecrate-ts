//! Locals are numbered only when the name is live in the same JS scope:
//! match arms reuse the Rust name; sequential `let`s in one function number.

use purecrate_check::accept;
use purecrate_pack::assemble;
use purecrate_syntax::parse_source;

fn file(name: &str, source: &str, stem: &str) -> String {
    let krate = parse_source(name, source).expect("parse");
    let typed = accept(&krate).expect("accept");
    let pkg = assemble(&typed);
    pkg.files
        .iter()
        .find(|f| f.stem == stem)
        .unwrap_or_else(|| panic!("{stem}"))
        .source
        .clone()
}

#[test]
fn match_arms_reuse_the_rust_name() {
    let src = file(
        "arms",
        "pub enum Event { Attach(u8), Confirm(u8) }\n\
         pub fn step(event: Event) -> u8 {\n\
             match event {\n\
                 Event::Attach(method) => method,\n\
                 Event::Confirm(method) => method,\n\
             }\n\
         }\n",
        "step",
    );
    assert!(src.contains("const method = event.value"), "{src}");
    assert!(!src.contains("method$"), "{src}");
}

#[test]
fn sequential_lets_in_one_function_are_numbered() {
    let chain = file(
        "seq",
        "pub enum E { Bad }\n\
         pub fn step(n: i32) -> Result<i32, E> { Ok(n + 1) }\n\
         pub fn chain(start: i32) -> Result<i32, E> {\n\
             let n = step(start)?;\n\
             let n = step(n)?;\n\
             let n = step(n)?;\n\
             Ok(n)\n\
         }\n",
        "chain",
    );
    assert!(chain.contains("const n: I32 = "), "{chain}");
    assert!(chain.contains("const n2: I32 = n2Result.value"), "{chain}");
    assert!(chain.contains("const n3: I32 = n3Result.value"), "{chain}");
    let decls: Vec<_> = chain
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("const n = ") || t.starts_with("const n:")
        })
        .collect();
    assert_eq!(
        decls.len(),
        1,
        "n is declared once, then numbered:\n{chain}"
    );
}

#[test]
fn a_shadow_in_one_arm_does_not_renumber_the_next() {
    let step = file(
        "shadow",
        "pub enum Event { A(i32), B(i32) }\n\
         pub fn step(event: Event, n: i32) -> i32 {\n\
             match event {\n\
                 Event::A(n) => n + 1,\n\
                 Event::B(k) => n + k,\n\
             }\n\
         }\n",
        "step",
    );
    assert!(step.contains("const n2 = event.value"), "{step}");
    assert!(step.contains("const k = event.value"), "{step}");
    assert!(!step.contains("k2"), "{step}");
}

#[test]
fn a_let_from_match_does_not_collide_with_the_arm_binding() {
    let src = file(
        "bail",
        "pub fn first_or_bail(x: Option<i32>) -> i32 {\n\
             let v = match x {\n\
                 Some(v) => v,\n\
                 None => return -1,\n\
             };\n\
             v * 2\n\
         }\n",
        "first-or-bail",
    );
    // The exit first, then `v` read from `x`: the arm's `v` is not bound.
    assert!(src.contains("if (x === null) return -1 as I32;"), "{src}");
    assert!(src.contains("const v: I32 = x;"), "{src}");
}
