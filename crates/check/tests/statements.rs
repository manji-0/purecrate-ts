mod common;

use common::{assert_clean, assert_rejects};

#[test]
fn let_mut_assignment_and_statements_are_accepted() {
    assert_clean("pub fn f(a: i32) -> i32 { let mut x = a; x += 1; x = x * 2; x }");
    assert_clean("pub fn f(a: i32) -> i32 { let mut n = 0i32; if a > 0 { n += 1; } n }");
    assert_clean(
        "pub fn f(a: Option<i32>) -> i32 { let mut n = 0i32; match a { Some(v) => n = v, None => {} } n }",
    );
    assert_clean("pub fn f(a: i32) -> i32 { let mut x = a; x = if a > 0 { 1 } else { 2 }; x }");
    assert_clean("pub fn f(a: i32) -> i32 { if a > 0 { return 1; } a }");
}

#[test]
fn only_let_mut_bindings_can_be_assigned() {
    assert_rejects(
        "pub fn f(a: i32) -> i32 { let x = a; x = 2; x }",
        "`x` is not `let mut`",
    );
    assert_rejects("pub fn f(a: i32) -> i32 { a = 2; a }", "`a` is not `let mut`");
    assert_rejects(
        "pub fn f(a: i32) -> i32 { let mut x = a; let x = 1i32; x = 2; x }",
        "`x` is not `let mut`",
    );
    assert_rejects("pub fn f() -> i32 { y = 2; 0 }", "`y` is not a parameter or local binding");
}

#[test]
fn assignment_inside_a_larger_expression_is_rejected() {
    assert_rejects(
        "pub fn f(a: i32) -> i32 { let mut x = a; 1i32 + { x = 2; x } }",
        "assignment inside a larger expression",
    );
}

#[test]
fn assigned_value_must_fit_the_binding() {
    assert_rejects("pub fn f(a: i32) -> i32 { let mut x = a; x = true; x }", "bool");
}

#[test]
fn let_mut_needs_a_known_type() {
    assert_rejects(
        "pub fn f() -> Option<i32> { let mut s = None; s = Some(1i32); s }",
        "write `let mut s: T`",
    );
}
