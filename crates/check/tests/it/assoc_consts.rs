//! Associated consts (`impl T { const N: u32 = 3; }`): folded as crate
//! consts are, members of the type's companion, and what is refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects, diagnostics};

#[test]
fn associated_consts_fold_and_are_read_by_path() {
    assert_clean(
        "pub struct T {}\nimpl T { pub const N: u32 = 3; const M: u32 = Self::N * 2; pub fn m() -> u32 { Self::M } }\npub fn f() -> u32 { T::N + T::m() }",
    );
    // A crate const and a local one may read one, and one may read a crate const.
    assert_clean(
        "pub const BASE: u8 = 7;\npub enum E { A }\nimpl E { pub const TOP: u8 = BASE + 1; }\npub const ALL: u8 = E::TOP * 2;\npub fn f() -> u8 { const HALF: u8 = E::TOP / 2; ALL + HALF }",
    );
    assert_clean(
        "pub struct T {}\nimpl T { pub const S: &'static str = \"é\"; pub const B: bool = true; pub const C: char = 'x'; pub const F: f64 = -0.5; }\npub fn f() -> (usize, bool, char, f64) { (T::S.len(), T::B, T::C, T::F) }",
    );
    // On a generic type: its value cannot depend on the parameter.
    assert_clean(
        "pub struct Page<T> { pub items: Vec<T> }\nimpl<T: Clone> Page<T> { pub const SIZE: usize = 10; pub fn full(&self) -> bool { self.items.len() >= Self::SIZE } }\npub fn f() -> usize { Page::<u8>::SIZE }",
    );
}

#[test]
fn an_associated_const_folds_with_the_crate_rules() {
    assert_rejects(
        "pub struct T {}\nimpl T { pub const N: u8 = 200 + 100; }\npub fn f() -> u8 { T::N }",
        "300 does not fit `u8`",
    );
    assert_rejects(
        "pub struct T {}\nimpl T { pub const N: u32 = 1; pub const W: u64 = Self::N; }\npub fn f() -> u64 { T::W }",
        "`T::N` is a `u32`, where a `u64` is expected",
    );
    assert_rejects(
        "pub struct P { pub x: i32 }\nimpl P { pub const ORIGIN: P = P { x: 0 }; }\npub fn f() -> i32 { 0 }",
        "a const of type `P` is not in v0",
    );
    let found = diagnostics(
        "pub struct T {}\nimpl T { pub fn g() -> u32 { 1 } pub const N: u32 = T::g(); }\npub fn f() -> u32 { T::N }",
    );
    assert!(found.iter().any(|d| d.reason.code() == "check/const-expr"), "{found:?}");
}

#[test]
fn an_associated_const_in_a_pattern_is_refused() {
    // Rust compares with the value; the IR has no such pattern.
    assert_parse_rejects(
        "pub struct T {}\nimpl T { pub const N: u32 = 3; }\npub fn f(x: u32) -> u32 { match x { T::N => 1, _ => 0 } }",
        "`T::N` is a const: matching against a const is not in v0",
    );
    assert_parse_rejects(
        "pub struct T {}\nimpl T { pub const N: u32 = 3; }\npub fn f(x: Option<u32>) -> bool { matches!(x, Some(T::N)) }",
        "`T::N` is a const",
    );
}

#[test]
fn an_associated_const_does_not_meet_a_companion_member() {
    assert_rejects(
        "pub enum E { A, B }\nimpl E { pub const A: u32 = 1; }\npub fn f() -> u32 { 0 }",
        "const `E.A` collides with the generated companion member `A`",
    );
    assert_rejects(
        "pub struct T { pub n: u32 }\n#[allow(non_upper_case_globals)]\nimpl T { pub const of: u32 = 1; }\npub fn f() -> u32 { 0 }",
        "const `T.of` collides with the generated companion member `of`",
    );
}

#[test]
fn a_trait_s_const_is_a_member_of_each_implementor() {
    // design/01 §7.25: the impl's value, or the trait's default.
    assert_clean(
        "pub struct T {}\npub struct U {}\npub trait Tr { const N: u32; const M: u32 = 4; }\nimpl Tr for T { const N: u32 = 1; }\nimpl Tr for U { const N: u32 = 2; const M: u32 = 5; }\npub fn f() -> u32 { T::N + T::M + U::N + U::M }",
    );
    assert_parse_rejects(
        "pub struct T {}\nimpl std::ops::Add for T { type Output = T; fn add(self, o: T) -> T { o } }\nimpl T { const N: u32 = 1; }\npub fn f() -> u32 { T::N }",
        "trait impls are not in v0",
    );
}
