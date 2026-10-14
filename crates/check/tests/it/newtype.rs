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
fn a_generic_newtype_is_built_and_read_at_its_argument() {
    assert_clean(
        "pub struct Wrap<T>(pub T);
         pub fn f(x: u32) -> Wrap<u32> { Wrap(x) }
         pub fn g(w: Wrap<String>) -> String { w.0 }
         pub fn h<T>(x: T) -> Wrap<T> { Wrap(x) }",
    );
    assert_rejects("pub struct Wrap<T>(pub T); pub fn f(x: u32) -> Wrap<String> { Wrap(x) }", "expected `String`");
}

#[test]
fn a_generic_newtype_over_null_like_values_is_rejected() {
    // In a type: `Wrap<Option<u32>>` would be `(U32 | null) & {brand}`, so
    // `null` would not be one.
    assert_rejects(
        "pub struct Wrap<T>(pub T); pub fn f(w: Wrap<Option<u32>>) -> bool { w.0.is_some() }",
        "cannot carry a brand",
    );
    assert_rejects("pub struct Wrap<T>(pub T); pub fn f(w: Wrap<()>) {}", "cannot carry a brand");
    // Built: the construction's instantiation is refused as any generic
    // item's is.
    assert_rejects(
        "pub struct Wrap<T>(pub T); pub fn f(x: Option<u8>) -> bool { Wrap(x).0.is_some() }",
        "takes `Option<u8>` as `T`",
    );
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
