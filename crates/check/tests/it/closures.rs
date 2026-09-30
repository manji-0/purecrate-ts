
use crate::common::{assert_clean, assert_rejects};

#[test]
fn local_closures_over_immutable_bindings_are_accepted() {
    assert_clean("pub fn f(x: i32) -> i32 { let add = |a: i32, b: i32| a + b; add(x, 1) }");
    assert_clean("pub fn f(x: i32) -> i32 { let k: i32 = 2; let g = |v: i32| v * k; g(x) }");
    assert_clean("pub fn f(x: i32) -> i32 { let mut y = x; y = y + 1; let g = |y: i32| y * 2; g(y) }");
    assert_clean(
        "pub fn h(v: i32) -> Option<i32> { Some(v) }
         pub fn f(x: i32) -> Option<i32> { let g = |v: i32| -> Option<i32> { let w = h(v)?; Some(w + 1) }; g(x) }",
    );
}

#[test]
fn capturing_a_mutable_binding_is_rejected() {
    assert_rejects(
        "pub fn f(x: i32) -> i32 { let mut y = x; let g = |v: i32| v + y; y = 3; g(y) }",
        "cannot capture `let mut y`",
    );
    assert_rejects(
        "pub fn f(x: i32) -> i32 { let mut y = x; let g = |v: i32| { y = v; v }; g(y) }",
        "cannot capture `let mut y`",
    );
}

#[test]
fn closures_need_known_parameter_and_exit_types() {
    assert_rejects(
        "pub fn f(x: i32) -> i32 { let g = |v| v + 1; g(x) }",
        "the type of closure parameter `v` is not known here",
    );
    assert_rejects(
        "pub fn h(v: i32) -> Option<i32> { Some(v) }
         pub fn f(x: i32) -> Option<i32> { let g = |v: i32| { let w = h(v)?; Some(w) }; g(x) }",
        "a closure with `?` or `return` needs its return type",
    );
}

#[test]
fn calling_a_closure_checks_its_signature() {
    assert_rejects(
        "pub fn f(x: i32) -> i32 { let g = |v: i32| v; g(x, x) }",
        "closure `g` takes 1 argument(s), got 2",
    );
    assert_rejects(
        "pub fn f(x: i32) -> bool { let g = |v: i32| v; g(x) }",
        "expected `bool`, found `i32`",
    );
}
