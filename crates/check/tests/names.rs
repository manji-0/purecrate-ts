mod common;

use common::{assert_clean, assert_rejects, diagnostics};

const COUNTER: &str = include_str!("../../../examples/counter/src/lib.rs");

#[test]
fn counter_is_clean() {
    assert_clean(COUNTER);
}

#[test]
fn duplicate_flattened_names_point_at_both_items() {
    let src = "pub struct A { pub n: i32 }\npub enum A { X }";
    let found = diagnostics(src);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].item, 1);
    assert_eq!(found[0].also, vec![0]);
    assert!(found[0].message.contains("defined more than once"));
}

#[test]
fn distinct_names_with_the_same_file_are_rejected() {
    assert_rejects(
        "pub struct Step { pub n: i32 }\npub fn step(s: Step) -> Step { s }",
        "`Step` and `step` would both be emitted as `step.ts`",
    );
}

#[test]
fn generated_names_and_files_are_reserved() {
    assert_rejects("pub enum Result { A }", "`Result` is reserved");
    assert_rejects("pub struct Index { pub n: i32 }", "emitted as `index.ts`");
    assert_rejects("pub struct AssertNever { pub n: i32 }", "emitted as `assert-never.ts`");
}

#[test]
fn runtime_globals_are_free_names() {
    // The emitted code reads `Error`, `Number`, `Math` and `BigInt` through
    // `globalThis`, so a crate may define them.
    assert_clean(
        "pub enum Error { NotFound, Number(i32) }\n\
         pub struct Math { pub n: i32 }\n\
         pub fn get(xs: Vec<i32>, i: usize) -> Result<i32, Error> { if i < xs.len() { Ok(xs[i]) } else { Err(Error::NotFound) } }",
    );
    assert_rejects("#[allow(non_camel_case_types)] pub struct globalThis { pub n: i32 }", "`globalThis` is reserved");
}

#[test]
fn proto_cannot_name_an_object_key() {
    let hint = "would set the prototype";
    assert_rejects("pub struct S { pub __proto__: i32 }", hint);
    assert_rejects("pub enum E { A { __proto__: i32 } }", hint);
    assert_rejects("#[allow(non_camel_case_types)] pub enum E { __proto__ }", hint);
    assert_rejects("pub struct S { pub n: i32 }\nimpl S { pub fn __proto__(self) -> S { self } }", hint);
    assert_clean("pub fn f(__proto__: i32) -> i32 { __proto__ }");
}

#[test]
fn ts_reserved_words_are_rejected_where_they_become_identifiers() {
    assert_rejects("pub struct S { pub default: i32 }", "field `default`");
    assert_rejects("pub fn f(new: i32) -> i32 { new }", "parameter `new`");
    assert_rejects("pub fn f(n: i32) -> i32 { let delete = n; delete }", "binding `delete`");
    assert_rejects(
        "pub enum E { A { void: i32 } }\npub fn f(e: E) -> i32 { match e { E::A { void: this } => this } }",
        "binding `this`",
    );
    assert_rejects("pub enum E { A { void: i32 } }", "field `void`");
    assert_rejects("pub fn r#typeof(n: i32) -> i32 { n }", "raw identifier");
}

#[test]
fn reserved_words_are_fine_as_variant_and_method_names() {
    assert_clean(
        "pub enum E { Default, New }\n\
         pub struct S { pub n: i32 }\n\
         impl S { pub fn delete(self) -> S { self } }",
    );
}

#[test]
fn type_keywords_cannot_name_types() {
    assert_rejects("#[allow(non_camel_case_types)] pub struct string { pub n: i32 }", "type keyword");
}

#[test]
fn kind_field_would_overwrite_the_discriminant() {
    assert_rejects("pub enum E { A { kind: i32 } }", "field `kind`, which is the union discriminant");
    assert_clean("pub struct S { pub kind: i32 }");
}

#[test]
fn companion_members_are_distinct() {
    assert_rejects(
        "pub struct S { pub n: i32 }\nimpl S { pub fn of(self) -> S { self } }",
        "collides with the generated companion member `of`",
    );
    assert_rejects(
        "pub enum E { A }\nimpl E { pub fn A(self) -> E { self } }",
        "companion member `A`",
    );
    assert_rejects(
        "pub struct S { pub n: i32 }\nimpl S { pub fn f(self) -> S { self } }\nimpl S { pub fn f(self) -> S { self } }",
        "`S.f` is defined more than once",
    );
}
