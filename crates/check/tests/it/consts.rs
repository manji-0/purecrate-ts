//! `const` items and `as` on enums: what is folded, and what is refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects, diagnostics};

#[test]
fn consts_fold_from_literals_consts_and_discriminants() {
    assert_clean("pub const A: u32 = 1 << 4;\npub const B: u32 = A * 2 + 1;\npub fn f() -> u32 { B }");
    assert_clean(
        "#[derive(Clone, Copy)]\npub enum E { X = 3, Y }\npub const M: u8 = E::X as u8 | E::Y as u8;\npub fn f() -> u8 { M }",
    );
    assert_clean("pub const S: &str = \"x\";\npub const T: &str = S;\npub fn f(s: &str) -> bool { s == T }");
}

#[test]
fn a_const_in_a_pattern_is_refused() {
    // Rust compares with the const's value; a binding would match anything.
    // A bare `LIMIT =>` arm is already refused as a binding arm; nested, a
    // binding is allowed, so the const must be caught by name.
    assert_rejects(
        "pub const LIMIT: u32 = 3;\npub fn f(x: Option<u32>) -> u32 { match x { Some(LIMIT) => 1, _ => 0 } }",
        "`LIMIT` is a const: matching against a const is not in v0",
    );
    assert_rejects(
        "pub const LIMIT: u32 = 3;\npub fn f(x: Option<u32>) -> u32 { if let Some(LIMIT) = x { 1 } else { 0 } }",
        "`LIMIT` is a const: matching against a const is not in v0",
    );
}

#[test]
fn const_expressions_outside_v0_are_refused() {
    let code = |src: &str| diagnostics(src).into_iter().map(|d| (d.reason.code(), d.message)).collect::<Vec<_>>();
    let found = code("pub fn g() -> u32 { 1 }\npub const A: u32 = g();\npub fn f() -> u32 { A }");
    assert!(found.iter().any(|(c, m)| *c == "check/const-expr" && m.contains("const `A`")), "{found:?}");
    assert_rejects("pub const A: u8 = 200 + 100;\npub fn f() -> u8 { A }", "300 does not fit `u8`");
    assert_rejects("pub const A: u32 = 1 << 32;\npub fn f() -> u32 { A }", "shift by 32 overflows a `u32`");
    assert_rejects(
        "pub const A: u32 = 1;\npub const B: u64 = A;\npub fn f() -> u64 { B }",
        "`A` is a `u32`, where a `u64` is expected",
    );
    assert_rejects(
        "pub struct P { x: i32 }\npub const O: P = P { x: 0 };\npub fn f() -> i32 { 0 }",
        "a const of type `P` is not in v0: `P` has fields that are not `pub`",
    );
}

#[test]
fn as_reads_only_a_fieldless_enums_discriminant() {
    // A widening std's `From` takes has that one spelling.
    assert_rejects("pub fn f(x: u8) -> u32 { x as u32 }", "`u8 as u32` widens, which v0 writes `u32::from(x)`");
    assert_rejects("pub fn f(x: i32) -> i64 { x as i64 }", "`i32 as i64` widens");
    // Unsigned to `usize` within 32 bits holds every value: Rust has no
    // `usize::from(u32)`, so `as` is how Rust writes it.
    assert_clean("pub fn f(a: u8, b: u16, c: u32) -> usize { a as usize + b as usize + c as usize }");
    // Every other pair of integer types wraps, as Rust's `as` does.
    assert_clean("pub fn f(x: u64, y: i32, z: usize) -> (usize, usize, u32) { (x as usize, y as usize, z as u32) }");
    assert_clean("pub fn f(x: u64, y: i64) -> (i64, u64, u8, i8) { (x as i64, y as u64, x as u8, y as i8) }");
    assert_clean("pub fn f(x: u32) -> u32 { x as u32 }");
    assert_rejects("pub enum E { A(i32), B }\npub fn f(e: E) -> u8 { e as u8 }", "`E as u8` is not in v0");
    assert_rejects(
        "#[derive(Clone, Copy)]\npub enum E { A = 1, B = 300 }\npub fn f(e: E) -> u8 { e as u8 }",
        "`E as u8` would not hold `E::B` = 300",
    );
    assert_clean("#[derive(Clone, Copy)]\npub enum E { A = 1, B = 300 }\npub fn f(e: E) -> u16 { e as u16 }");
    assert_rejects(
        "#[repr(u8)]\n#[derive(Clone, Copy)]\npub enum E { A = 255, B }\npub fn f(e: E) -> u8 { e as u8 }",
        "`E::B` would follow the largest `u8`",
    );
}

#[test]
fn consts_share_one_file() {
    // `MAX` and `fn max` are two files; only an item named like the file meets it.
    assert_clean("pub const MAX: u32 = 9;\npub fn max() -> u32 { MAX }");
    assert_rejects(
        "pub const MAX: u32 = 9;\npub fn consts() -> u32 { MAX }",
        "would both be emitted as `consts.ts`, which holds the crate's consts",
    );
}

#[test]
fn a_local_const_is_a_let_at_the_top_of_its_block() {
    assert_clean("pub fn f(x: u32) -> u32 { let y = x * K; const K: u32 = 3; y }");
    assert_parse_rejects(
        "pub fn f(x: Option<u32>) -> u32 { const LIMIT: u32 = 3; match x { Some(LIMIT) => 1, _ => 0 } }",
        "`LIMIT` is a const: matching against a const is not in v0",
    );
    assert_parse_rejects(
        "pub fn f() -> u32 { const K: u32 = 3; let K = 4; K }",
        "`K` is a const: matching against a const is not in v0",
    );
    assert_parse_rejects(
        "pub fn f() -> u32 { fn g() -> u32 { 1 } g() }",
        "items inside blocks other than `const` are not in v0",
    );
}

/// A `&[u8]` const is a byte string; a `u128` literal stops below 2^127, as
/// literals are held as `i128`.
#[test]
fn byte_strings_and_wide_literals() {
    assert_clean("const A: &[u8] = b\"0123\";\nconst B: &[u8] = b\"\\xff\\x00\";\npub fn f(i: usize) -> (u8, u8) { (A[i], B[i]) }");
    assert_rejects(
        "const A: &[u8] = &[1, 2];\npub fn f() -> u8 { A[0] }",
        "a `&[u8]` const is a byte string `b\"..\"` in v0",
    );
    assert_clean("pub fn f(x: u128) -> u128 { x ^ 170141183460469231731687303715884105727 }");
    assert_parse_rejects(
        "pub fn f() -> u128 { 170141183460469231731687303715884105728 }",
        "is 2^127 or more, which v0 does not hold",
    );
}

/// 128-bit consts fold in `i128`: a `u128` at 2^127 or more is refused, not
/// a crash.
#[test]
fn wide_consts_fold_or_say_why_not() {
    assert_clean(
        "const R: u128 = (1 << 80) - 1;\nconst S: i128 = !0 << 100;\npub fn f(x: u128) -> (u128, i128) { (x & R, S) }",
    );
    for src in ["const R: u128 = !0 >> 48;", "const R: u128 = 1 << 127;", "const R: u128 = (1 << 126) * 2;"] {
        assert_rejects(&format!("{src}\npub fn f() -> u128 {{ R }}"), "a `u128` const of 2^127 or more is not in v0");
    }
}

/// A negative literal at a signed minimum folds, as rustc takes it; `MIN /
/// -1` is an overflow, not a crash.
#[test]
fn signed_minimums_fold() {
    assert_clean("const A: i8 = -128;\nconst B: i128 = -170141183460469231731687303715884105727 - 1;\npub fn f() -> (i8, i128) { (A, B) }");
    assert_rejects("const Q: i128 = (1 << 127) / (0 - 1);\npub fn f() -> i128 { Q }", "overflow");
    assert_rejects("const Q: usize = 1 << 60;\npub fn f() -> usize { Q }", "where a `usize` const stops in v0");
}

/// A crate const may be a tuple or one of the crate's enums, alone or in
/// arrays, each part a const value (design/01 §7.29); `()`, an `Option`,
/// std's `Ordering`, a generic enum, and an associated const of these stay
/// refused.
#[test]
fn const_tuples_and_enums() {
    const E: &str = "#[derive(Clone, Copy, PartialEq)]\npub enum K { A, B(u8, (i8, char)), C { at: (u8, u8) } }\n";
    assert_clean(&format!(
        "{E}pub const D: [(i8, i8); 2] = [(1, 0), (0, -1)];\npub const KS: [K; 3] = [K::A, K::B(2, (-1, 'x')), K::C {{ at: S }}];\n\
         pub const S: (u8, u8) = (4, 0);\npub const F: K = K::A;\npub const G: [[(K, bool); 1]; 2] = [[(F, true)], [(K::A, false); 1]];\n\
         pub const H: [[(i8, i8); 2]; 2] = [D, D];\n\
         pub fn f(i: usize) -> i8 {{ let mut t = 0; for (a, b) in D {{ t += a - b; }} for &(a, _) in D.iter() {{ t += a; }} t + D[i].0 + H[i][i].1 }}\n\
         pub fn g(i: usize) -> bool {{ KS.contains(&F) && KS[i] == G[1][0].0 && S.0 == 4 && matches!(KS[i], K::B(_, _)) }}"
    ));
    let refused = |src: &str, why: &str| assert_rejects(&format!("{E}{src}\npub fn f() -> u8 {{ 0 }}"), why);
    refused("pub const U: () = ();", "a `()` const is a literal or another const in v0");
    refused("pub const O: Option<u8> = Some(1);", "a const of type `Option<u8>` is not in v0");
    refused(
        "use std::cmp::Ordering;\npub const O: [Ordering; 1] = [Ordering::Less];",
        "a const of type `Ordering` is not in v0",
    );
    refused(
        "#[derive(Clone, Copy)]\npub enum W<T> { V(T) }\npub const O: W<u8> = W::V(1);",
        "a const of type `W<u8>` is not in v0",
    );
    refused("pub const O: [(u8, u8); 1] = [(1, 300)];", "300 does not fit `u8`");
    refused("pub const fn first() -> K { K::A }\npub const O: K = first();", "a `K` const is one of its variants");
    refused("pub const O: (u8, K) = (1, if true { K::A } else { K::A });", "a `K` const is one of its variants");
    refused(
        "pub const O: u8 = 1;\npub const P: (u8, u8) = (O, O);\npub const Q: (u8, i8) = P;",
        "`P` is a `(u8, u8)`, where a `(u8, i8)` is expected",
    );
    refused("pub struct T {}\nimpl T { pub const O: (u8, u8) = (1, 2); }", "a const of type `(u8, u8)` is not in v0");
}

/// A crate const may be one of the crate's open structs, in a tuple, a
/// variant's payload, an array, or another struct, each field a const
/// value (design/01 §7.29); a closed struct, a newtype, a generic struct,
/// a struct update, a call, an `Option` field, and an associated const of
/// a struct stay refused.
#[test]
fn const_structs() {
    const S: &str = "#[derive(Clone, Copy, PartialEq)]\npub struct P { pub x: i8, pub y: i8 }\n\
                     #[derive(Clone, Copy, PartialEq)]\npub enum K { A, B(P), C { at: P } }\n\
                     #[derive(Clone, Copy, PartialEq)]\npub struct Q { pub k: K, pub at: P, pub steps: [(i8, i8); 2] }\n";
    assert_clean(&format!(
        "{S}pub const O: P = P {{ y: 0, x: -1 }};\npub const T: (P, u8) = (O, 1);\n\
         pub const KS: [K; 3] = [K::A, K::B(O), K::C {{ at: P {{ x: 1, y: 2 }} }}];\n\
         pub const QS: [Q; 2] = [Q {{ steps: [(1, 0); 2], k: K::B(O), at: O }}, Q {{ k: K::A, at: P {{ x: 3, y: 3 }}, steps: [(0, 1), (1, 1)] }}];\n\
         pub const G: [[P; 2]; 1] = [[O; 2]];\n\
         pub fn f(i: usize) -> i8 {{ let mut t = O.x + T.0.y + G[0][1].x; for q in QS {{ t += q.at.x; }} for p in G[0].iter() {{ t += p.y; }} t + QS[i].steps[1].0 }}\n\
         pub fn g(p: P) -> bool {{ p == O || G[0].contains(&p) || QS[0].k == K::B(p) }}"
    ));
    let refused = |src: &str, why: &str| assert_rejects(&format!("{S}{src}\npub fn f() -> u8 {{ 0 }}"), why);
    refused("pub const O: P = P { x: 1, y: 128 };", "128 does not fit `i8`");
    refused(
        "pub struct C { pub x: i8, y: i8 }\npub const O: [C; 1] = [C { x: 0, y: 0 }];",
        "a const of type `C` is not in v0: `C` has fields that are not `pub`, so its TS value is branded and only its own functions make one; make every field `pub`, or build the value in a function",
    );
    refused(
        "#[derive(Clone, Copy)]\npub struct Id(pub u32);\npub const O: (Id, u8) = (Id(1), 2);",
        "a const of type `Id` is not in v0: a newtype is made by its constructor, which is no literal; keep the inner value as the const and wrap it where it is used",
    );
    refused(
        "pub struct W<T> { pub v: T }\npub const O: W<u8> = W { v: 1 };",
        "a const of type `W<u8>` is not in v0: a generic struct is no const value; declare one without type parameters, or build the value in a function",
    );
    refused(
        "pub struct W<T> { pub v: T }\npub const O: [W<u8>; 1] = [W { v: 1 }];",
        "a const of type `W<u8>` is not in v0: a generic struct",
    );
    refused(
        "pub const O: P = P { x: 0, y: 0 };\npub const R: P = P { x: 1, ..O };",
        "`P { .., ..base }` is not a const value in v0: write every field",
    );
    refused(
        "impl P { pub const fn new() -> P { P { x: 0, y: 0 } } }\npub const O: P = P::new();",
        "a `P` const is a literal of its fields, `P { a: .., b: .. }`",
    );
    refused(
        "pub struct M { pub x: Option<u8> }\npub const O: M = M { x: Some(1) };",
        "a const of type `Option<u8>` is not in v0",
    );
    refused("impl P { pub const O: P = P { x: 0, y: 0 }; }", "a const of type `P` is not in v0");
}
