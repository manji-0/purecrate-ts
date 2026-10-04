//! `//` lines above a statement, a block's tail, a `match` arm, or a crate
//! `const`, after the code on a statement's last line, and before a block's
//! `}`, are printed where that code goes (design/03 §1).

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

    // Across a blank line, for the next statement.

    let c = b + 1; // and one after its code
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
    assert!(src.contains("  // Across a blank line, for the next statement.\n  // and one after its code\n  const c = "), "{src}");
    assert!(src.contains("  // The result.\n  return c;\n"), "{src}");
}

#[test]
fn comments_on_arms_before_a_closing_brace_and_on_consts() {
    let src = emitted(
        "pub enum K { A, B }
pub fn f(k: K) -> Result<i32, i32> {
    match k {
        // A is fine.
        K::A => Ok(1),
        K::B => Err(2), // B is not
    }
}
",
        "f",
    );
    assert!(src.contains("case \"A\":\n      // A is fine.\n      return Result.ok("), "{src}");
    assert!(src.contains("case \"B\":\n      // B is not\n      return Result.err("), "{src}");
    let src = emitted(
        "pub fn f(xs: Vec<u8>) -> u8 {
    let mut t = 0u8;
    for x in &xs {
        t += *x;
        // Nothing else per item.
    }
    t
}
",
        "f",
    );
    assert!(src.contains("t = Int.u8.add(t, x);\n\n    // Nothing else per item.\n  }"), "{src}");
    let src = emitted(
        "// The longest state accepted.
pub const MAX_STATE: u32 = 512;
pub fn f() -> u32 { MAX_STATE }
",
        "consts",
    );
    assert!(src.contains("// The longest state accepted.\nexport const MAX_STATE"), "{src}");
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

#[test]
fn a_comment_stays_one_line_in_js() {
    // Rust reads a lone CR, U+2028, and U+2029 inside a `//` comment; JS
    // ends the line there, so the rest would be code.
    for sep in ["\r", "\u{2028}", "\u{2029}"] {
        let src = emitted(
            &format!(
                "// k{sep}export const Z = 1;\npub const N: u32 = 1;\npub fn f(x: u32) -> u32 {{\n    // note{sep}if (x === 1) return 42 as U32;\n    let y = x + N;\n    y\n}}\n"
            ),
            "f",
        );
        assert!(src.contains("// note if (x === 1) return 42 as U32;\n"), "{src:?}");
        assert!(!src.contains(sep), "{src:?}");
        let consts = emitted(
            &format!("// k{sep}export const Z = 1;\npub const N: u32 = 1;\npub fn f() -> u32 {{ N }}\n"),
            "consts",
        );
        assert!(consts.contains("// k export const Z = 1;\n") && !consts.contains(sep), "{consts:?}");
    }
}
