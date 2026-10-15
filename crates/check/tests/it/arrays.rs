//! Fixed-size arrays `[T; N]` (design/01 §7.27): read as a slice, matched
//! by their one length, written only as a local `let mut` array; what
//! would write one in place, a length that is not a literal or a crate
//! const, an element that is `null` in TS, and more than serde's 32
//! elements on a type that derives it are refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects};

#[test]
fn arrays_are_accepted() {
    assert_clean("pub fn f(b: [u8; 4]) -> u8 { b[0] }");
    assert_clean("const N: usize = 2 * 3; pub fn f(b: &[u8; N]) -> usize { b.len() }");
    assert_clean("pub const T: [char; 2] = ['a', 'é']; pub fn f(i: usize) -> char { T[i] }");
    assert_clean("pub const B: &[u8; 3] = b\"abc\"; pub fn f(i: usize) -> u8 { B[i] }");
    assert_clean("pub fn f(x: u16) -> [u16; 3] { [x; 3] }");
    assert_clean("pub fn f(b: [u8; 4]) -> u8 { let [a, _, _, d] = b; a + d }");
    assert_clean("pub fn f(b: [u8; 2]) -> u8 { match b { [0, x] => x, [x, _] => x } }");
    assert_clean("pub fn f(b: [u8; 4]) -> u8 { match b { [x, ..] => x } }");
    assert_clean("pub fn f(a: [u8; 2], b: [u8; 2]) -> u8 { match (a, b) { ([x, _], [_, y]) => x + y } }");
    assert_clean("pub fn f(o: Option<[u8; 2]>) -> u8 { match o { Some([x, _]) => x, None => 0 } }");
    assert_clean("pub fn f(x: u8) -> [u8; 3] { let mut a = [0u8; 3]; a[1] = x; a }");
    assert_clean("pub fn f(a: [u8; 2], b: [u8; 2]) -> bool { a < b && a != b }");
    assert_clean("pub fn f(a: [u8; 3]) -> Vec<u8> { a.iter().map(|x| x / 2).collect() }");
    assert_clean("pub fn f(a: [u8; 3]) -> usize { a.as_slice().len() + (&a[1..]).len() }");
}

#[test]
fn an_array_is_matched_by_its_one_length() {
    assert_rejects(
        "pub fn f(b: [u8; 2]) -> u8 { match b { [0, x] => x, [1, x] => x } }",
        "this match on a slice may leave some value out",
    );
    assert_rejects(
        "pub fn f(b: [u8; 2]) -> u8 { match b { [x, y] if x > y => x } }",
        "match on an array of 2 elements is missing `[_, _]`",
    );
    // A slice's `let` is refutable.
    assert_rejects("pub fn f(b: &[u8]) -> u8 { let [x, _] = b; *x }", "match on a slice is missing `[]`");
    assert_parse_rejects(
        "pub fn f(b: [u8; 2]) -> u8 { let [x, _]: [u8; 2] = b; x }",
        "a `let` of a slice pattern takes no type annotation",
    );
}

#[test]
fn an_array_is_not_written_in_place() {
    assert_rejects("pub fn f(a: [u8; 2]) -> [u8; 2] { a.map(|x| x + 1) }", "`a.map(f)` on an array is not in v0");
    assert_rejects(
        "pub fn f(a: [u8; 2]) -> [u8; 2] { let mut b = a; b.sort(); b }",
        "`.sort()` writes an array in place, which is not in v0",
    );
    assert_rejects(
        "pub fn f(a: [u8; 2]) -> u8 { let mut b = a; b.sort_by(|x, y| y.cmp(x)); b[0] }",
        "`.sort_by()` writes an array in place",
    );
    assert_rejects("pub fn f(a: [u8; 2]) -> u8 { let mut b = a; b.swap(0, 1); b[0] }", "`.swap()` writes an array");
    assert_rejects("pub fn f(a: [u8; 2]) -> Vec<u8> { a.to_vec() }", "write `a.iter().copied().collect()`");
    assert_parse_rejects(
        "pub fn f(a: [[u8; 2]; 2]) -> u8 { let mut b = a; b[0][1] = 3; b[0][0] }",
        "not of a field or an element",
    );
    assert_rejects(
        "pub fn f(a: [u8; 2]) -> u8 { let mut b = a; let g = |i: usize| b[i]; b[0] = 1; g(0) }",
        "a closure cannot capture `let mut b`",
    );
}

#[test]
fn lengths_and_elements_outside_v0_are_refused() {
    assert_parse_rejects("pub fn f<const N: usize>(a: [u8; N]) -> u8 { a[0] }", "const generics are not in v0");
    assert_parse_rejects(
        "pub fn f(n: usize) -> u8 { let a: [u8; n] = [0; 1]; a[0] }",
        "the length of an array is an integer literal or a crate `const`",
    );
    assert_rejects("pub fn f(a: [Option<u8>; 2]) -> bool { a[0].is_some() }", "an array of `Option<u8>` is not in v0");
    assert_rejects("pub fn f(a: [(); 2]) -> usize { a.len() }", "an array of `()` is not in v0");
    assert_rejects("pub const T: [u8; 3] = [1, 2]; pub fn f() -> u8 { T[0] }", "a `[u8; 3]` holds 3 elements, found 2");
    assert_rejects("pub fn f() -> [u8; 3] { [1, 2] }", "expected an array of 3 elements, found one of 2");
}

#[test]
fn serde_reads_arrays_up_to_32() {
    assert_rejects(
        "#[derive(serde::Serialize, serde::Deserialize)] pub struct S { pub a: [u8; 33] }",
        "`S` derives `Serialize` but holds an array of 33 elements; serde implements it for arrays of up to 32",
    );
    assert_rejects(
        "type Big = [u8; 40]; #[derive(serde::Deserialize)] pub enum E { A(Vec<Big>) }",
        "`E` derives `Deserialize` but holds an array of 40 elements",
    );
}
