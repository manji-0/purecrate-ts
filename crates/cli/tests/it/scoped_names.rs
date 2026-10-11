//! Locals are numbered only when the name is live in the same JS scope:
//! match arms reuse the Rust name; sequential `let`s in one function number.

use crate::support;

fn file(name: &str, source: &str, stem: &str) -> String {
    support::emitted(name, source, stem)
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
    assert!(chain.contains("const n = "), "{chain}");
    assert!(chain.contains("const n2 = n2Result.value"), "{chain}");
    assert!(chain.contains("const n3 = n3Result.value"), "{chain}");
    let decls: Vec<_> = chain
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("const n = ") || t.starts_with("const n:")
        })
        .collect();
    assert_eq!(decls.len(), 1, "n is declared once, then numbered:\n{chain}");
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
    assert!(src.contains("const v = x;"), "{src}");
}

#[test]
fn a_local_takes_the_name_of_a_function_its_body_does_not_read() {
    let source = "pub fn total(n: i32) -> i32 { n * 2 }\n\
                  pub fn keep(total: i32) -> i32 { total + 1 }\n\
                  pub fn calls(n: i32) -> i32 { let total = total(n); total + 1 }\n";
    let keep = file("names", source, "keep");
    assert!(keep.contains("(total: I32): I32"), "{keep}");
    assert!(!keep.contains("total2"), "{keep}");
    // A call reads the import, so the local beside it is numbered.
    let calls = file("names", source, "calls");
    assert!(calls.contains("const total2 = total(n);"), "{calls}");
}

#[test]
fn an_option_unwrapped_into_its_own_name_is_read_narrowed() {
    let src = file(
        "unwrap",
        "pub fn twice(n: Option<i32>) -> i32 {\n\
             let n = match n { Some(n) => n, None => return 0 };\n\
             n * 2\n\
         }\n",
        "twice",
    );
    // No `const n2 = n`: past the exit, TS has narrowed `n`.
    assert!(src.contains("if (n === null) return 0 as I32;"), "{src}");
    assert!(src.contains("Int.i32.mul(n, 2 as I32)") && !src.contains("n2"), "{src}");
}

/// A `const` of a call, a test, or a place that is no union states no type;
/// a literal and a `let mut` keep theirs, and a variant a `match` tests is
/// cast to its enum.
#[test]
fn a_const_states_no_type_its_value_already_has() {
    let source = "pub enum Light { Red, Green }\n\
                  pub struct P { pub a: u32 }\n\
                  fn light_of(n: u32) -> Light { if n == 0 { Light::Red } else { Light::Green } }\n\
                  pub fn run(p: P) -> u32 {\n\
                      let a = p.a;\n\
                      let l = light_of(a);\n\
                      let red = matches!(l, Light::Red);\n\
                      let g = Light::Green;\n\
                      let mut n = a + 1;\n\
                      if red && matches!(g, Light::Green) { n += 1; }\n\
                      n\n\
                  }\n";
    let run = file("consts", source, "run");
    for line in
        ["const a = p.a;", "const l = lightOf(a);", "const red = l.kind === \"Red\";", "let n: U32 = Int.u32.add("]
    {
        assert!(run.contains(line), "{line}:\n{run}");
    }
    // `g` is tested by a `match`: annotated, TS would narrow it to the
    // variant it was given, so the cast keeps the union.
    assert!(run.contains("const g = { kind: \"Green\" } as Light;"), "{run}");
}
