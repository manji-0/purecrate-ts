
use crate::common::assert_clean;

#[test]
fn box_is_the_inner_value() {
    assert_clean("pub fn f(n: i32) -> i32 { let b = Box::new(n); *b }");
    assert_clean("use std::sync::Arc; pub fn f(n: i32) -> i32 { let a = Arc::new(n); *a }");
    assert_clean(
        "pub enum Ast { Num(i32), Add(Box<Ast>, Box<Ast>) }
         pub fn add(a: Ast, b: Ast) -> Ast { Ast::Add(Box::new(a), Box::new(b)) }
         pub fn calc(e: Ast) -> i32 { match e { Ast::Num(n) => n, Ast::Add(a, b) => calc(*a) + calc(*b) } }",
    );
}

