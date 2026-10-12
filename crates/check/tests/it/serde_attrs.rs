//! The serde a server puts on the same types (design/04 §3, §5, §3.6):
//! derives pass, `#[serde(try_from = "T")]` needs its `impl TryFrom<T>`,
//! `rename`, `rename_all`, `tag`, and `content` pass where serde can read
//! and write what they name, `Display` and `Error` impls are skipped, other
//! serde attributes and trait impls are rejected.

use crate::common::{assert_clean, assert_rejects, messages};

const ID: &str = "#[derive(Debug)] pub enum E { Bad }\n\
                  impl std::fmt::Display for E { fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(\"bad\") } }\n\
                  #[derive(Debug)] pub struct Dbg { pub n: i32 }\n";

fn with(rest: &str) -> String {
    format!("{ID}{rest}")
}

/// Plain rustc (the helper's) has no serde, so only the subset checks run;
/// `cli`'s `serde_derives_compile_under_check` covers rustc with the stand-in.
#[test]
fn try_from_with_its_impl_is_clean() {
    assert_no_diagnostics(&with(
        "#[serde(try_from = \"String\")] pub struct Id(String);\n\
         impl TryFrom<String> for Id { type Error = E; fn try_from(s: String) -> Result<Self, Self::Error> { if s.is_empty() { Err(E::Bad) } else { Ok(Id(s)) } } }\n\
         pub fn make(s: String) -> Result<Id, E> { Id::try_from(s) }",
    ));
}

fn assert_no_diagnostics(src: &str) {
    let found = messages(src);
    assert!(found.is_empty(), "unexpected diagnostics: {found:#?}");
}

#[test]
fn try_from_needs_a_matching_impl() {
    assert_rejects(
        &with("#[serde(try_from = \"String\")] pub struct Id(String);"),
        "`#[serde(try_from = \"String\")]` on `Id` needs `impl TryFrom<String> for Id`",
    );
    assert_rejects(
        &with(
            "#[serde(try_from = \"i32\")] pub struct Id(String);\n\
             impl TryFrom<String> for Id { type Error = E; fn try_from(s: String) -> Result<Self, Self::Error> { Ok(Id(s)) } }",
        ),
        "needs `impl TryFrom<i32> for Id`",
    );
}

#[test]
fn skipped_and_rejected_trait_impls() {
    assert_clean(&with("impl std::error::Error for E {}\npub fn f(e: E) -> E { e }"));
}

#[test]
fn other_serde_attributes_and_traits_are_rejected() {
    let err = purecrate_syntax::parse_source("c", "#[serde(deny_unknown_fields)] pub struct S { pub a: i32 }")
        .expect_err("deny_unknown_fields");
    assert!(err.message.contains("the wire module models `rename`, `rename_all`, `tag`"), "{}", err.message);
    let err =
        purecrate_syntax::parse_source("c", "pub struct S { #[serde(default)] pub a: i32 }").expect_err("field attr");
    assert!(err.message.contains("`#[serde(default)]` changes the JSON"), "{}", err.message);
    let err = purecrate_syntax::parse_source(
        "c",
        "pub struct S(i32);\nimpl From<i32> for S { fn from(n: i32) -> Self { S(n) } }",
    )
    .expect_err("From");
    assert!(err.message.contains("except `Display`, `Error` (skipped) and `TryFrom<T>`"), "{}", err.message);
}

#[test]
fn names_and_tags_serde_can_read_are_clean() {
    assert_no_diagnostics(
        "#[serde(rename_all = \"camelCase\")] pub struct P { pub first_name: i32, #[serde(rename = \"y\")] pub last: i32 }\n\
         pub struct Q {}\n\
         #[serde(tag = \"type\")] pub enum S { A, B { x: i32 }, C(P), D(Q) }\n\
         #[serde(tag = \"t\", content = \"c\")] pub enum T { A, B(i32, i32), C(Option<i32>), D { x: i32 } }\n\
         pub fn f(s: S, t: T) -> (S, T) { (s, t) }",
    );
}

#[test]
fn names_and_tags_serde_cannot_read_are_rejected() {
    for (src, message) in [
        (
            "pub struct S { #[serde(rename = \"b\")] pub a: i32, pub b: i32 }",
            "two fields of `S` are named `b` on the wire",
        ),
        (
            "#[serde(rename_all = \"lowercase\")] pub enum E { Ab, AB }",
            "two variants of `E` are named `ab` on the wire",
        ),
        ("#[serde(tag = \"t\")] pub enum E { A(i32, i32) }", "`E::A` is a tuple variant"),
        ("#[serde(tag = \"t\")] pub enum E { A(i32) }", "`E::A` holds `i32`"),
        ("#[serde(tag = \"t\")] pub enum E { A(()) }", "`E::A` holds `()`"),
        ("pub struct N(i32);\n#[serde(tag = \"t\")] pub enum E { A(N) }", "`E::A` holds `N`"),
        ("#[serde(tag = \"t\")] pub enum E { A { t: i32 } }", "has a field named `t` on the wire"),
        ("#[serde(tag = \"t\", content = \"t\")] pub enum E { A }", "names its tag and its content both `t`"),
    ] {
        assert!(
            messages(&format!("{src}\npub fn f(e: Box<i32>) -> i32 {{ *e }}")).iter().any(|m| m.contains(message)),
            "{src}: wanted {message}, got {:?}",
            messages(&format!("{src}\npub fn f(e: Box<i32>) -> i32 {{ *e }}"))
        );
    }
}
