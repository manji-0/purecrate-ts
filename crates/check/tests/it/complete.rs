use crate::common;
use crate::common::{assert_clean, assert_rejects};

#[test]
fn a_binding_whose_type_is_not_known_is_rejected() {
    assert_rejects(
        "pub fn half(a: i32, b: i32) -> i32 {
             let r = Ok(a); let s = Ok(b);
             match r { Ok(v) => match s { Ok(w) => v / w, Err(e) => e }, Err(e) => e }
         }",
        "the type of `let r` is not known; write `let r: T`",
    );
    assert_rejects(
        "pub fn f(c: bool, e: i32) -> Result<i32, i32> {
             let d = if c { Ok(5i32) } else { Err(e) };
             Ok(d? + 1)
         }",
        "the type of `let d` is not known",
    );
    assert_clean(
        "pub fn half(a: i32, b: i32) -> i32 {
             let r: Result<i32, i32> = Ok(a); let s: Result<i32, i32> = Ok(b);
             match r { Ok(v) => match s { Ok(w) => v / w, Err(e) => e }, Err(e) => e }
         }",
    );
}

#[test]
fn a_matched_value_whose_type_is_not_known_is_rejected() {
    let hint = "the type of the matched value is not known";
    assert_rejects("pub fn f(a: i32) -> i32 { match Ok::<i32, i32>(a) { Ok(v) => v, Err(e) => e } }", hint);
    assert_rejects("pub fn f(a: i32) -> i32 { match Ok(a) { Ok(v) => v, Err(_) => 0 } }", hint);
    assert_clean("pub fn f(a: i32) -> i32 { let r: Result<i32, i32> = Ok(a); match r { Ok(v) => v, Err(e) => e } }");
}

#[test]
fn only_the_binding_is_reported_not_what_uses_it() {
    let found = common::messages("pub fn f(a: i32) -> i32 { let r = Ok(a); match r { Ok(v) => v + 1, Err(e) => e } }");
    assert_eq!(found.len(), 1, "{found:#?}");
}

#[test]
fn some_of_an_option_is_rejected() {
    assert_rejects(
        "pub fn get(o: Option<i32>) -> Option<i32> { o }
         pub fn f(o: Option<i32>) -> i32 { let y = Some(get(o)); match y { Some(_) => 1, None => 2 } }",
        "is `Option<Option<_>>`",
    );
}
