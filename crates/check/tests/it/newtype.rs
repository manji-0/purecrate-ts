use crate::common::{assert_clean, assert_rejects};

#[test]
fn newtypes_construct_unwrap_and_take_methods() {
    assert_clean(
        "pub struct Id(u32);
         impl Id { pub fn next(&self) -> Self { Self(self.0 + 1) } }
         pub fn f(n: u32) -> u32 { Id::next(&Id(n)).0 }",
    );
    assert_clean("pub struct Tag(String); pub fn f(t: Tag) -> String { t.0 }");
}

#[test]
fn newtypes_over_null_like_values_are_rejected() {
    assert_rejects("pub struct Maybe(Option<i32>);", "cannot carry a brand");
    assert_rejects("pub type O = Option<i32>; pub struct Maybe(O);", "cannot carry a brand");
    assert_rejects("pub struct Nothing(());", "cannot carry a brand");
}

#[test]
fn dot_zero_needs_a_newtype() {
    assert_rejects(
        "pub struct S { pub n: i32 } pub fn f(s: S) -> i32 { s.0 }",
        "only in v0 on a one-field tuple struct",
    );
    assert_rejects("pub fn f(x: (i32, i32)) -> i32 { x.0 }", "not on `(i32, i32)`");
}

#[test]
fn method_paths_need_a_defined_method() {
    assert_rejects("pub struct S { pub n: i32 } pub fn f(s: S) -> i32 { S::missing(s) }", "`S.missing` is not defined");
}
