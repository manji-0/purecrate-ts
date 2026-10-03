//! `//` lines directly above a statement or a block's tail are printed above
//! what that statement becomes (design/03 §1).

use purecrate_check::accept;
use purecrate_pack::assemble_with;
use purecrate_syntax::parse_source;

fn emitted(source: &str, stem: &str) -> String {
    let krate = parse_source("comments", source).expect("parses");
    let typed = accept(&krate).expect("in the subset");
    assemble_with(&typed, None)
        .files
        .into_iter()
        .find(|f| f.stem == stem)
        .unwrap_or_else(|| panic!("missing {stem}"))
        .source
}

#[test]
fn comments_above_statements_are_kept() {
    let src = emitted(
        "pub fn f(a: i32) -> i32 {
    // Twice, so it is even.
    //
    // Then one more.
    let b = a * 2;

    // Not this one: a blank line ends it.

    let c = b + 1; // nor a comment after code
    // The result.
    c
}
",
        "f",
    );
    assert!(
        src.contains("  // Twice, so it is even.\n  //\n  // Then one more.\n  const b = "),
        "{src}"
    );
    assert!(src.contains("  // The result.\n  return c;\n"), "{src}");
    assert!(!src.contains("Not this one") && !src.contains("nor a comment"), "{src}");
}

#[test]
fn a_comment_in_an_expression_is_left_out() {
    let src = emitted(
        "pub fn f(a: i32) -> i32 {
    let b = 1 + {
        // Only a value here.
        a
    };
    b
}
",
        "f",
    );
    assert!(!src.contains("Only a value"), "{src}");
    assert!(src.contains("=> Int.i32.add(1 as I32, a);"), "{src}");
}

#[test]
fn comments_above_a_tail_open_the_body() {
    let src = emitted(
        "pub fn f(a: i32) -> bool {
    // Exact match.
    a == 1
}
",
        "f",
    );
    assert!(src.contains("=> {\n  // Exact match.\n  return a === 1;\n};"), "{src}");
}
