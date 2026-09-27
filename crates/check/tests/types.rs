mod common;

use common::{assert_clean, assert_rejects};
use purecrate_check::accept;
use purecrate_ir::{Callee, Expr, FloatTy, IntOp, IntTy, Item, Lit};
use purecrate_syntax::parse_source;

fn body_of(source: &str, name: &str) -> Expr {
    let typed = accept(&parse_source("c", source).expect("parse")).expect("accepted");
    typed
        .items
        .into_iter()
        .find_map(|item| match item {
            Item::Fn(f) if f.name.as_str() == name => Some(f.body),
            _ => None,
        })
        .expect("fn")
}

#[test]
fn integer_arithmetic_becomes_checked_calls() {
    let body = body_of("pub fn half(a: i32) -> i32 { a / 2 }", "half");
    let Expr::Call { callee, args } = body else {
        panic!("expected a call, got {body:?}")
    };
    assert_eq!(callee, Callee::Int { ty: IntTy::I32, op: IntOp::Div });
    assert_eq!(args[1], Expr::Lit(Lit::Int { value: 2, ty: Some(IntTy::I32) }));
}

#[test]
fn literals_take_the_type_through_an_alias() {
    let body = body_of("pub type Id = i64;\npub fn next(a: Id) -> Id { a + 1 }", "next");
    let Expr::Call { callee, args } = body else {
        panic!("expected a call, got {body:?}")
    };
    assert_eq!(callee, Callee::Int { ty: IntTy::I64, op: IntOp::Add });
    assert_eq!(args[1], Expr::Lit(Lit::Int { value: 1, ty: Some(IntTy::I64) }));
}

#[test]
fn negative_literal_folds_at_the_minimum() {
    let body = body_of("pub fn m() -> i8 { -128 }", "m");
    assert_eq!(body, Expr::Lit(Lit::Int { value: -128, ty: Some(IntTy::I8) }));
}

#[test]
fn f32_arithmetic_is_rounded() {
    let body = body_of("pub fn h(a: f32) -> f32 { a * 0.5 }", "h");
    assert!(matches!(body, Expr::Call { callee: Callee::Fround, .. }), "{body:?}");
}

#[test]
fn f64_arithmetic_is_branded() {
    let body = body_of("pub fn h(a: f64) -> f64 { a * 0.5 }", "h");
    assert!(matches!(body, Expr::Call { callee: Callee::AsFloat(FloatTy::F64), .. }), "{body:?}");
}

#[test]
fn suffixes_and_annotations_settle_literals() {
    assert_clean("pub fn f() -> i64 { let x = 1i64; x * 2 }");
    assert_clean("pub fn f() -> i64 { let x: i64 = 1; x * 2 }");
    assert_clean("pub fn f(a: i32) -> bool { 0 < a }");
}

#[test]
fn unannotated_literal_binding_is_rejected() {
    assert_rejects("pub fn f() -> i64 { let x = 1; 2 }", "cannot tell the integer type of `1`");
    assert_rejects("pub fn f() -> f64 { let x = 1.5; 2.0 }", "cannot tell the float type of `1.5`");
}

#[test]
fn literal_out_of_range_is_rejected() {
    assert_rejects("pub fn f() -> u8 { 256 }", "literal `256` does not fit in `u8`");
    assert_rejects("pub fn f() -> i8 { -129 }", "literal `-129` does not fit in `i8`");
}

#[test]
fn integer_literal_for_a_float_is_rejected() {
    assert_rejects("pub fn f() -> f64 { 1 }", "where a float is expected");
}

#[test]
fn negating_unsigned_is_rejected() {
    assert_rejects("pub fn f(a: u32) -> u32 { -a }", "cannot negate `u32`");
}

#[test]
fn mismatched_types_are_rejected() {
    assert_rejects("pub fn f(a: i32) -> i64 { a }", "expected `i64`, found `i32`");
}

#[test]
fn comparisons_js_gets_wrong_are_rejected() {
    assert_rejects(
        "pub struct P { pub x: i32 }\npub fn f(a: P, b: P) -> bool { a == b }",
        "equality on `P` is not in v0",
    );
    assert_rejects(
        "pub fn f(a: String, b: String) -> bool { a < b }",
        "ordering on `String` is not in v0",
    );
    assert_clean("pub fn f(a: String, b: String) -> bool { a == b }");
}

#[test]
fn runtime_helper_names_are_reserved() {
    assert_rejects("pub struct Int { pub v: i32 }", "`Int` is reserved");
    assert_rejects("pub struct Math { pub v: i32 }", "`Math` is reserved");
}
