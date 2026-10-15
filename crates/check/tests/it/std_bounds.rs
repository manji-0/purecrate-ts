//! Std bounds that carry a dictionary (design/01 §7.25): what each accepts,
//! and the instantiations and uses refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects};

#[test]
fn each_std_bound_is_used_through_its_dictionary() {
    assert_clean(
        "use std::cmp::Ordering;\npub fn f<T: Ord + Clone>(xs: &[T], a: &T, b: &T) -> (Option<T>, Ordering, T, bool) { (xs.iter().max().cloned(), a.cmp(b), a.clone().min(b.clone()), Some(a) < Some(b)) }\npub fn g(xs: Vec<u64>) -> Option<u64> { f(&xs, &1, &2).0 }",
    );
    assert_clean("pub fn f<T>(a: T, b: T) -> bool where T: PartialOrd { a < b }\npub fn g() -> bool { f('a', 'b') }");
    assert_clean(
        "use std::fmt;\npub fn f<T: fmt::Display>(x: T) -> String { format!(\"[{}] {x}\", x) }\npub fn g(n: i64, s: &str) -> (String, String, String) { (f(n), f(s), f(true)) }",
    );
    assert_clean("pub fn f(x: impl std::fmt::Display) -> String { x.to_string() }\npub fn g() -> String { f('c') }");
    assert_clean(
        "pub fn f<T: Default>() -> (T, T) { (T::default(), Default::default()) }\npub fn g() -> (u8, u8) { f() }\npub fn h() -> (String, String) { f() }",
    );
    assert_clean("pub struct Bag<T> { pub items: Vec<T> }\nimpl<T: Ord + Clone> Bag<T> { pub fn top(&self) -> Option<T> { self.items.iter().max().cloned() } }\npub fn f(b: Bag<char>) -> Option<char> { b.top() }");
    assert_clean("#[derive(Default)]\npub struct P<T> { pub t: T, pub n: u8 }\npub fn f() -> P<i64> { P::default() }\npub fn g<T: Default>() -> T { T::default() }\npub fn h() -> P<bool> { g() }");
}

#[test]
fn an_instantiation_without_the_member_is_refused() {
    assert_rejects(
        "pub fn lt<T: PartialOrd>(a: &T, b: &T) -> bool { a < b }\npub fn f(a: f64, b: f64) -> bool { lt(&a, &b) }",
        "`f64` has no `cmp` to pass as the dictionary of a `PartialOrd` bound",
    );
    assert_rejects(
        "#[derive(PartialEq, PartialOrd)]\npub struct Fl { pub x: f64 }\npub fn lt<T: PartialOrd>(a: &T, b: &T) -> bool { a < b }\npub fn f(a: Fl, b: Fl) -> bool { lt(&a, &b) }",
        "`Fl` derives `PartialOrd` without `Ord`",
    );
    assert_rejects(
        "pub fn show<T: std::fmt::Display>(x: T) -> String { format!(\"{}\", x) }\npub fn f(x: f64) -> String { show(x) }",
        "`f64` is not passed as the dictionary of a `Display` bound in v0",
    );
    assert_rejects(
        "use std::fmt;\npub struct W(pub String);\nimpl fmt::Display for W { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str(&self.0) } }\npub fn show<T: fmt::Display>(x: T) -> String { x.to_string() }\npub fn f(w: W) -> String { show(w) }",
        "has none translated",
    );
    assert_rejects(
        "pub fn z<T: Default>() -> T { T::default() }\npub fn f() -> Vec<u8> { z() }",
        "`Vec<u8>` is not passed as the dictionary of a `Default` bound in v0",
    );
}

#[test]
fn uses_a_dictionary_cannot_carry_are_refused() {
    assert_rejects("pub fn f<T: Ord>(x: T, lo: T, hi: T) -> T { x.clamp(lo, hi) }", "`.clamp()` on a `T` is not in v0");
    assert_rejects(
        "pub fn f<T: std::fmt::Display>(x: T) -> String { format!(\"{:>8}\", x) }",
        "a width, alignment, or fill on a `T` is not in v0",
    );
    assert_rejects(
        "pub fn f<T: std::fmt::Display>(x: T, n: u8) -> String { format!(\"{} {}\", x, n + 1) }",
        "bind each argument after it with `let` first",
    );
}

#[test]
fn names_the_package_gives_the_std_bounds_are_reserved() {
    assert_rejects(
        "pub struct Display { pub n: u8 }\npub fn f<T: std::fmt::Display>(x: T) -> String { x.to_string() }",
        "`Display` is the type of a `T: Display` bound's dictionary in the generated package",
    );
    assert_rejects(
        "pub fn ord_i32() -> u8 { 0 }\npub fn f<T: Ord>(a: T, b: T) -> bool { a < b }",
        "`ord_i32` is reserved by the generated package for std's dictionaries",
    );
    // Without a std bound, neither name is the package's.
    assert_clean("pub struct Display { pub n: u8 }\npub fn ord_i32() -> u8 { 0 }");
    assert_parse_rejects("pub fn f<T: std::fmt::Write>(x: T) -> T { x }", "the bound `std::fmt::Write` is not in v0");
}
