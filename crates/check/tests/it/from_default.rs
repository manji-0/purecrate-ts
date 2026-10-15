//! `impl From<S> for X` (with `?`) and `Default` (design/01 §7.25): what is
//! accepted, and each form refused with its reason.

use crate::common::*;

const E: &str = "#[derive(Debug, PartialEq)] pub enum E { A, B(u8) }\n";

fn with_e(rest: &str) -> String {
    format!("{E}{rest}")
}

#[test]
fn from_impls_and_their_calls_are_clean() {
    assert_clean(&with_e(
        "impl From<u8> for E { fn from(n: u8) -> Self { E::B(n) } }\n\
         impl From<bool> for E { fn from(b: bool) -> E { if b { E::A } else { E::B(0) } } }\n\
         impl TryFrom<u16> for E { type Error = u16; fn try_from(n: u16) -> Result<Self, Self::Error> { if n < 256 { Ok(E::B(n as u8)) } else { Err(n) } } }\n\
         pub fn half(n: u8) -> Result<u8, u8> { if n % 2 == 0 { Ok(n / 2) } else { Err(n) } }\n\
         pub fn f(n: u8, b: bool) -> Result<u8, E> { let h = half(n)?; let _a = E::from(b); let _c: E = From::from(n); let _d: E = b.into(); Ok(h) }",
    ));
}

#[test]
fn a_source_without_a_name_is_refused() {
    let refused = [
        ("impl<T> From<T> for E { fn from(_t: T) -> Self { E::A } }", "a generic `impl From`"),
        ("impl From<Option<u8>> for E { fn from(_o: Option<u8>) -> Self { E::A } }", "an `Option` source"),
        ("impl From<(u8, u8)> for E { fn from(p: (u8, u8)) -> Self { E::B(p.0) } }", "a tuple source"),
        ("impl From<&str> for E { fn from(_s: &str) -> Self { E::A } }", "a reference source"),
        ("impl From<Vec<u8>> for E { fn from(_v: Vec<u8>) -> Self { E::A } }", "a source with type arguments"),
    ];
    for (src, needle) in refused {
        assert_parse_rejects(&with_e(src), needle);
    }
}

#[test]
fn from_has_only_its_one_method() {
    assert_parse_rejects(
        &with_e("impl From<u8> for E { fn from(n: u8) -> Self { E::B(n) } const N: u8 = 1; }"),
        "`impl From` has only `fn from(s: S) -> Self`",
    );
}

#[test]
fn members_that_meet_are_refused() {
    assert_rejects(
        &with_e(
            "pub struct Char {}\n\
             impl From<char> for E { fn from(_c: char) -> Self { E::A } }\n\
             impl From<Char> for E { fn from(_c: Char) -> Self { E::A } }",
        ),
        "`impl From<char>` and `impl From<Char>` for `E` would both be the member `E.fromChar`",
    );
    assert_rejects(
        &with_e(
            "impl E { pub fn from_bool(_b: bool) -> E { E::A } }\n\
             impl From<bool> for E { fn from(_b: bool) -> Self { E::A } }",
        ),
        "method `E.from_bool` and the member of `impl From<bool>` for `E` would both be `E.fromBool`",
    );
}

#[test]
fn from_on_a_foreign_type_into_and_conversion_bounds_are_refused() {
    assert_parse_rejects(
        &with_e("impl From<E> for String { fn from(_e: E) -> String { String::new() } }"),
        "`impl From` for `String`, a type outside the crate, is not in v0",
    );
    assert_parse_rejects(
        &with_e("impl Into<u8> for E { fn into(self) -> u8 { 0 } }"),
        "`impl Into` is not in v0: write `impl From<S> for X`",
    );
    assert_parse_rejects(&with_e("pub fn f<T: Into<E>>(t: T) -> E { t.into() }"), "the bound `Into<E>` is not in v0");
    assert_parse_rejects(
        &with_e("pub fn f<T>(t: T) -> E where T: From<u8> { E::A }"),
        "the bound `From<u8>` is not in v0",
    );
}

#[test]
fn a_try_without_a_from_keeps_its_message() {
    assert_rejects(
        &with_e(
            "pub fn half(n: u8) -> Result<u8, u8> { if n % 2 == 0 { Ok(n / 2) } else { Err(n) } }\n\
             pub fn f(n: u8) -> Result<u8, E> { let h = half(n)?; Ok(h) }",
        ),
        "`?` on an error of type `u8` in a function returning `Result<u8, E>`; the error types must match, or `E` must have an `impl From<u8>`",
    );
}

#[test]
fn a_conversion_with_no_impl_or_no_target_is_refused() {
    let from_u8 = "impl From<u8> for E { fn from(n: u8) -> Self { E::B(n) } }\n";
    assert_rejects(
        &with_e(&format!("{from_u8}pub fn f(n: u16) -> E {{ E::from(n) }}")),
        "`E::from`: `E` has no `impl From<u16> for E`; it converts from `u8`",
    );
    assert_rejects(
        &with_e(&format!("{from_u8}pub fn f(n: u8) -> bool {{ let e = n.into(); e == E::A }}")),
        "the target of `.into()` is not known here",
    );
    assert_rejects(
        &with_e(&format!("{from_u8}pub fn f(n: u8) -> u32 {{ n.into() }}")),
        "`.into()` into `u32` is not in v0",
    );
    assert_rejects(&with_e("pub fn f(n: u8) -> E { E::from(n) }"), "`E::from` needs an `impl From<S> for E`");
}

#[test]
fn derived_and_written_defaults_are_clean() {
    assert_clean(
        "#[derive(Default)] pub enum Mode { Off, #[default] On }\n\
         #[derive(Default)] pub struct S { pub n: i64, pub s: String, pub v: Vec<u8>, pub o: Option<u32>, pub m: Mode, pub t: (u8, bool) }\n\
         pub struct W { pub n: u8 }\n\
         impl Default for W { fn default() -> Self { W { n: 7 } } }\n\
         pub fn f(n: i64) -> S { S { n, ..Default::default() } }\n\
         pub fn g() -> W { let w: W = Default::default(); let _x = W::default(); w }",
    );
}

#[test]
fn a_field_without_a_default_is_refused() {
    assert_rejects(
        "use uuid::Uuid;\n#[derive(Default)] pub struct S { pub id: Uuid }",
        "`#[derive(Default)]`: field `id` is a `Uuid`, which has no default in v0",
    );
    assert_rejects(
        "pub struct T { pub n: u8 }\n#[derive(Default)] pub struct S { pub t: T }",
        "`#[derive(Default)]`: field `t` is a `T`, which has no default in v0",
    );
}

#[test]
fn default_shapes_outside_the_subset_are_refused() {
    assert_parse_rejects(
        "#[derive(Default)] pub struct P<T> { pub t: T }",
        "`#[derive(Default)]` on `P`, which has type parameters, is not in v0",
    );
    assert_parse_rejects(
        "#[derive(Default)] pub enum M { A, B }",
        "`#[derive(Default)]` on the enum `M` needs one `#[default]` variant without fields",
    );
    assert_parse_rejects(
        "pub struct W {}\nimpl Default for W { fn default() -> Self { W {} } fn other() -> u8 { 0 } }",
        "`impl Default` has only `fn default() -> Self`",
    );
    assert_parse_rejects(
        "pub struct W {}\nimpl Default for Vec<W> { fn default() -> Self { Vec::new() } }",
        "`impl Default` for `Vec`, a type outside the crate, is not in v0",
    );
    assert_rejects(
        "pub fn f() -> u8 { let _n = Default::default(); 0 }",
        "the type of `Default::default()` is not known here",
    );
    assert_rejects(
        "pub struct W {}\npub fn f() -> W { W::default() }",
        "`W::default` needs `#[derive(Default)]` on `W`",
    );
    assert_rejects(
        "use uuid::Uuid;\npub fn f() -> Uuid { Default::default() }",
        "`Default::default()` of `Uuid`: the type has no default in v0",
    );
}
