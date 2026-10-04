use crate::common::{assert_clean, assert_parse_rejects, assert_rejects, diagnostics};

#[test]
fn std_ordering_is_named_every_way_rust_names_it() {
    assert_clean("use std::cmp::Ordering;\npub fn f(a: i32, b: i32) -> Ordering { a.cmp(&b) }");
    assert_clean("use core::cmp::Ordering;\npub fn f(a: &str, b: &str) -> Ordering { a.cmp(b).reverse() }");
    assert_clean("use std::cmp::{Ordering};\npub fn f(a: char, b: char) -> bool { a.cmp(&b) == Ordering::Less }");
    assert_clean("pub fn f(a: u8, b: u8) -> std::cmp::Ordering { a.cmp(&b).then(core::cmp::Ordering::Less) }");
    assert_clean(
        "pub fn f(o: std::cmp::Ordering) -> u8 { match o { std::cmp::Ordering::Less => 0, core::cmp::Ordering::Equal => 1, ::std::cmp::Ordering::Greater => 2 } }",
    );
    assert_clean("pub fn f(a: u64, b: u64) -> bool { matches!(a.cmp(&b), std::cmp::Ordering::Less) }");
    // Only a call: the enum is added all the same.
    assert_clean("pub fn f(a: bool, b: bool) -> bool { a.cmp(&b).is_ge() }");
    assert_clean("use std::cmp::Ordering;\npub fn f(o: Ordering) -> (i8, i32) { (o as i8, o as i32) }");
}

#[test]
fn modules_name_it_too() {
    // A crate module named `cmp` is the crate's, not std's.
    assert_clean(
        "mod cmp {\n    pub fn by(a: i32, b: i32) -> std::cmp::Ordering { a.cmp(&b) }\n}\npub use cmp::by;\npub fn f(o: std::cmp::Ordering) -> bool { o.is_lt() && cmp::by(1, 2).is_lt() }",
    );
    assert_clean("mod a {\n    use std::cmp::Ordering;\n    pub fn by(a: u8, b: u8) -> Ordering { a.cmp(&b) }\n}\npub use a::by;");
    assert_parse_rejects(
        "mod a {\n    pub enum Ordering { X }\n}\nuse std::cmp::Ordering;\npub fn f(o: Ordering) -> bool { o.is_eq() }",
        "the crate defines its own `Ordering` and also names `std::cmp::Ordering`",
    );
}

#[test]
fn methods_and_then_with_take_what_std_takes() {
    let head = "use std::cmp::Ordering;\n";
    assert_clean(&format!(
        "{head}fn tie() -> Ordering {{ Ordering::Equal }}\npub fn f(o: Ordering) -> Ordering {{ o.then_with(tie) }}"
    ));
    assert_clean(&format!("{head}pub fn f(o: Ordering, n: i32) -> Ordering {{ o.then_with(|| n.cmp(&0)) }}"));
    assert_rejects(
        &format!("{head}pub fn f(o: Ordering, n: i32) -> Ordering {{ o.then_with(|x: i32| x.cmp(&n)) }}"),
        "`Ordering::then_with` takes a closure `|| ..` or a function name",
    );
    assert_rejects(
        &format!(
            "{head}pub fn f(o: Ordering, n: Option<i32>) -> Option<Ordering> {{ Some(o.then_with(|| n?.cmp(&0))) }}"
        ),
        "may not use `?` or `return`",
    );
    assert_rejects(
        &format!(
            "{head}fn by(n: i32) -> Ordering {{ n.cmp(&0) }}\npub fn f(o: Ordering) -> Ordering {{ o.then_with(by) }}"
        ),
        "`then_with` calls its function with 0 arguments, which takes 1",
    );
    assert_rejects(
        &format!("{head}pub fn f(o: Ordering) -> Ordering {{ o.then() }}"),
        "`Ordering::then` takes 1 argument(s) after the receiver, got 0",
    );
    assert_rejects(&format!("{head}pub fn f(o: Ordering, p: Ordering) -> Ordering {{ o.max(p) }}"), "`.max()` on `Ordering` is not on the std allow-list; allowed: `is_eq`, `is_ne`, `is_lt`, `is_gt`, `is_le`, `is_ge`, `reverse`, `then`, `then_with`");
    assert_rejects(
        &format!("{head}pub fn f(o: Ordering, p: Ordering) -> Ordering {{ o.cmp(&p) }}"),
        "`.cmp()` on `Ordering` is not on the std allow-list",
    );
    assert_rejects(
        &format!("{head}pub fn f(o: Ordering) -> bool {{ o < Ordering::Equal }}"),
        "ordering on `Ordering` is not in v0",
    );
    assert_rejects(
        "pub fn f(a: i32, b: i32) -> bool { a.cmp(&b, 1).is_lt() }",
        "`cmp` takes 1 argument after the receiver, got 2",
    );
}

#[test]
fn cmp_is_refused_where_js_has_no_order_to_follow() {
    assert_rejects(
        "pub fn f(a: f64, b: f64) -> bool { a.cmp(&b).is_lt() }",
        "`.cmp()` on `f64` is not in v0: floats are not `Ord`",
    );
    assert_rejects(
        "pub fn f(a: f64, b: f64) -> bool { a.partial_cmp(&b).is_some() }",
        "`.partial_cmp()` on `f64` is not in v0",
    );
    assert_rejects(
        "pub fn f(a: i32, b: i32) -> bool { a.partial_cmp(&b).is_some() }",
        "`.partial_cmp()` on `i32` is not in v0",
    );
    assert_rejects(
        "pub fn f(a: (i32, i32), b: (i32, i32)) -> bool { a.cmp(&b).is_lt() }",
        "`.cmp()` on `(i32, i32)` is not in v0",
    );
    assert_rejects("pub fn f(a: Vec<f64>, b: Vec<f64>) -> bool { a.cmp(&b).is_lt() }", "floats are not `Ord`");
    assert_rejects("pub fn f(a: Vec<(u8, u8)>, b: Vec<(u8, u8)>) -> bool { a.cmp(&b).is_lt() }", "not of `(u8, u8)`");
    assert_rejects(
        "pub fn f(a: Option<u8>, b: Option<u8>) -> bool { a.cmp(&b).is_lt() }",
        "`.cmp()` on `Option<u8>` is not in v0",
    );
    assert_rejects(
        "#[derive(PartialEq, Eq, PartialOrd, Ord)]\npub struct V { pub n: u32 }\npub fn f(a: V, b: V) -> bool { a.cmp(&b).is_lt() }",
        "`.cmp()` on `V` is not in v0: `Ord` on the crate's own types is not modeled",
    );
    assert_rejects(
        "pub struct V { pub n: u32 }\npub fn f(a: V, b: V) -> bool { a < b }",
        "ordering on `V` is not in v0",
    );
    assert_rejects(
        "pub fn f(a: bool) -> Option<u8> { a.then(|| 1u8) }",
        "`.then()` on `bool` is not on the std allow-list; allowed: `cmp`",
    );
    assert_parse_rejects("pub struct V { pub n: u32 }\nimpl PartialOrd for V {}", "trait impls are not in v0");
    assert_parse_rejects("pub struct V { pub n: u32 }\nimpl Ord for V {}", "trait impls are not in v0");
}

#[test]
fn only_the_forms_that_keep_ordering_std_are_taken() {
    assert_parse_rejects(
        "use std::cmp::Ordering;\npub enum Ordering { Before, After }",
        "the crate defines its own `Ordering` and also names `std::cmp::Ordering`",
    );
    assert_parse_rejects(
        "pub struct Ordering { pub n: i32 }\npub fn f(a: i32) -> std::cmp::Ordering { a.cmp(&0) }",
        "the crate defines its own `Ordering`",
    );
    assert_parse_rejects(
        "use std::cmp::Ordering::*;\npub fn f() -> i32 { 0 }",
        "`use std::cmp::Ordering::*` is not in v0",
    );
    assert_parse_rejects(
        "use std::cmp::Ordering::{Less, Greater};\npub fn f() -> i32 { 0 }",
        "importing the variants of `std::cmp::Ordering` is not in v0",
    );
    assert_parse_rejects("use std::cmp::*;\npub fn f() -> i32 { 0 }", "`use std::cmp::*` is not in v0");
    assert_parse_rejects("use std::cmp::Ordering as Order;\npub fn f() -> i32 { 0 }", "renames std's `Ordering`");
    assert_parse_rejects(
        "use std::cmp;\npub fn f(a: i32, b: i32) -> cmp::Ordering { a.cmp(&b) }",
        "`cmp::Ordering` is not in v0; write `use std::cmp::Ordering;`",
    );
    assert_parse_rejects(
        "use std::cmp;\npub fn f(a: i32, b: i32) -> bool { matches!(a.cmp(&b), cmp::Ordering::Less) }",
        "`cmp::Ordering` is not in v0",
    );
}

#[test]
fn a_crates_own_ordering_is_left_alone() {
    assert_clean("pub enum Ordering { Before, After }\npub fn f(o: Ordering) -> Ordering { match o { Ordering::Before => Ordering::After, Ordering::After => Ordering::Before } }");
    // It hides std's, which `cmp` gives.
    assert_rejects(
        "pub enum Ordering { Before, After }\npub fn f(a: i32) -> bool { a.cmp(&0).is_lt() }",
        "`cmp` gives `std::cmp::Ordering`, which the crate's own `Ordering` hides",
    );
    // A crate `Less` stays the crate's.
    assert_clean(
        "use std::cmp::Ordering;\npub enum Size { Less(u8), More }\nuse Size::*;\npub fn f(s: Size, o: Ordering) -> u8 { match s { Less(n) => n + if o.is_lt() { 1 } else { 0 }, Size::More => 0 } }",
    );
}

#[test]
fn ordering_has_no_wire_form() {
    assert_rejects(
        "use std::cmp::Ordering;\n#[derive(serde::Serialize)]\npub struct Pair { pub by: Ordering }",
        "`Pair` derives `Serialize` but holds a `std::cmp::Ordering`, which has no serde form",
    );
    assert_rejects(
        "use std::cmp::Ordering;\npub type Ords = Vec<Ordering>;\n#[derive(serde::Deserialize)]\npub enum Step { Compared(Option<Ords>) }",
        "`Step` derives `Deserialize` but holds a `std::cmp::Ordering`",
    );
    let found =
        diagnostics("use std::cmp::Ordering;\n#[derive(serde::Serialize)]\npub struct Pair { pub by: Ordering }");
    assert_eq!(found.iter().map(|d| d.reason.code()).collect::<Vec<_>>(), ["item/serde-derive"]);
    // Without a serde derive a type has no wire form, so it may hold one.
    assert_clean("use std::cmp::Ordering;\npub struct Pair { pub by: Ordering }");
    assert_clean("use std::cmp::Ordering;\npub fn f(xs: Vec<u8>) -> Vec<Ordering> { vec![xs[0].cmp(&xs[1])] }");
}

#[test]
fn a_serde_derive_needs_it_on_what_the_type_holds() {
    assert_rejects(
        "#[derive(serde::Serialize, serde::Deserialize)]\npub struct Order { pub line: Line }\n#[derive(serde::Serialize)]\npub struct Line { pub n: i32 }",
        "`Order` derives `Deserialize` but holds `Line`, which does not",
    );
    assert_rejects(
        "pub type Lines = Vec<Line>;\n#[derive(serde::Serialize)]\npub enum Order { Open(Lines) }\npub struct Line { pub n: i32 }",
        "`Order` derives `Serialize` but holds `Line`, which does not",
    );
    // Not `assert_clean`: rustc here has no serde.
    let both = "#[derive(serde::Serialize, serde::Deserialize)]\npub struct Order { pub line: Line }\n#[derive(serde::Serialize, serde::Deserialize)]\npub struct Line { pub n: i32 }";
    assert!(diagnostics(both).is_empty(), "{:#?}", diagnostics(both));
}

#[test]
fn equality_on_ordering_is_its_variant() {
    assert_clean(
        "use std::cmp::Ordering;\npub fn f(a: Ordering, b: Ordering) -> bool { a == b || a != Ordering::Greater }",
    );
    assert_rejects("pub enum M { A, B }\npub fn f(a: M, b: M) -> bool { a == b }", "equality on `M` is not in v0");
}
