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
        "or `.0` on a one-field tuple struct, not on `S`",
    );
    assert_rejects("pub struct S(i32); pub fn f(s: S) -> i32 { s.1 }", "`.1` is only in v0 on a tuple");
    assert_rejects("pub fn f(x: (i32, i32)) -> i32 { x.2 }", "a tuple of 2 elements has no `.2`");
    assert_clean("pub fn f(x: (i32, (u8, bool))) -> bool { x.0 > 0 && x.1 .1 }");
}

#[test]
fn method_paths_need_a_defined_method() {
    assert_rejects("pub struct S { pub n: i32 } pub fn f(s: S) -> i32 { S::missing(s) }", "`S.missing` is not defined");
}
