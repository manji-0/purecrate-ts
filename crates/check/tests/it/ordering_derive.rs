//! `PartialOrd` and `Ord` on the crate's types (design/01 §7.25): what is
//! taken, and each form refused with the reason and the alternative.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects, diagnostics};

const ORD: &str = "#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]";

#[test]
fn a_derived_or_written_ord_orders_the_type() {
    assert_clean(&format!(
        "{ORD}\npub struct V {{ pub n: u32, pub s: String }}\npub fn f(a: V, b: V) -> (bool, V, V) {{ (a < b, a.clone().max(b.clone()), a.clamp(b.clone(), b)) }}"
    ));
    // `Box` is its content, in Rust's order as here.
    assert_clean(&format!(
        "{ORD}\npub enum T {{ Leaf(u8), Node(Box<T>, Option<Box<T>>) }}\npub fn f(v: Vec<T>) -> Option<T> {{ let mut v = v; v.sort(); v.into_iter().max() }}"
    ));
    assert_clean(
        "use std::cmp::Ordering;\n#[derive(PartialEq, Eq)]\npub struct W { pub n: u8 }\nimpl Ord for W { fn cmp(&self, other: &Self) -> Ordering { other.n.cmp(&self.n) } }\nimpl PartialOrd for W { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) } }\npub fn f(a: W, b: W) -> bool { a >= b }",
    );
    // A derived `Ord` beside a hand-written `PartialOrd` that is `cmp`'s.
    assert_clean(&format!(
        "use std::cmp::Ordering;\n#[derive(Clone, PartialEq, Eq, Ord)]\npub struct W {{ pub n: u8 }}\nimpl PartialOrd for W {{ fn partial_cmp(&self, o: &Self) -> Option<Ordering> {{ Some(self.cmp(o)) }} }}\n{ORD}\npub struct H {{ pub w: Option<W>, pub ws: Vec<(W, char)> }}\npub fn f(a: H, b: H) -> bool {{ a < b }}"
    ));
}

#[test]
fn ordering_a_type_without_ord_is_refused() {
    assert_rejects(
        "#[derive(PartialEq, PartialOrd)]\npub struct P { pub x: f64 }\npub fn f(a: P, b: P) -> bool { a < b }",
        "ordering on `P` is not in v0: `P` derives `PartialOrd` without `Ord`",
    );
    assert_rejects(
        "#[derive(PartialEq, Eq, PartialOrd)]\npub struct P { pub x: u8 }\npub fn f(a: P, b: P) -> P { a.max(b) }",
        "`.max()` on `P` orders by `cmp`, which it has not: `P` derives `PartialOrd` without `Ord`",
    );
    assert_rejects(
        "#[derive(PartialEq, Eq)]\npub struct P { pub x: u8 }\npub fn f(v: Vec<P>) -> Vec<P> { let mut v = v; v.sort(); v }",
        "`sort` orders by `cmp`, and `P` has none: `P` has no `Ord`",
    );
    assert_rejects(
        "#[derive(PartialEq, Eq)]\npub struct P { pub x: u8 }\npub fn f(v: Vec<P>) -> Option<P> { v.into_iter().min() }",
        "`min` orders by `cmp`, and `P` has none",
    );
}

#[test]
fn a_field_without_an_order_is_named() {
    for (field, shown) in [
        ("Result<u8, String>", "`Result<u8, String>`, which v0 does not order"),
        ("BTreeMap<String, u8>", "`BTreeMap<String, u8>`, which v0 does not order"),
        ("BTreeSet<String>", "`BTreeSet<String>`, which v0 does not order"),
    ] {
        assert_rejects(
            &format!("use std::collections::{{BTreeMap, BTreeSet}};\n{ORD}\npub struct P {{ pub id: u8, pub x: {field} }}\npub fn f(a: P, b: P) -> bool {{ a < b }}"),
            &format!("`P` derives `Ord`, but its field `x` holds {shown}"),
        );
    }
    // Through another type, which is then not ordered either.
    assert_rejects(
        &format!(
            "{ORD}\npub enum Q {{ A(Vec<Result<u8, u8>>) }}\n{ORD}\npub struct P {{ pub q: Option<Q> }}\npub fn f(a: P, b: P) -> bool {{ a < b }}"
        ),
        "its field `q` holds `Q`, which is not ordered: `Q` derives `Ord`, but its field `Q::A.0` holds `Result<u8, u8>`",
    );
    // Two types that hold each other: each reason is spelled out once.
    assert_rejects(
        &format!(
            "{ORD}\npub struct A {{ pub b: Option<Box<B>>, pub r: Result<u8, u8> }}\n{ORD}\npub struct B {{ pub a: Vec<A> }}\npub fn f(x: B, y: B) -> bool {{ x < y }}"
        ),
        "`B` derives `Ord`, but its field `a` holds `A`, which is not ordered: `A` derives `Ord`, but its field `b` holds `B`, which is not ordered",
    );
    // A type that is never ordered may hold anything.
    assert_clean(&format!("{ORD}\npub struct P {{ pub x: Result<u8, u8> }}\npub fn f(a: P) -> P {{ a }}"));
}

#[test]
fn a_generic_type_is_ordered_with_its_parameters_dictionaries() {
    assert_clean(&format!(
        "{ORD}\npub struct Page<T> {{ pub items: Vec<T> }}\npub fn f(a: Page<u8>, b: Page<u8>) -> bool {{ a < b }}"
    ));
    // `<` on a `Page<F>` needs `F: PartialOrd` only, and v0 orders by `cmp`.
    assert_rejects(
        &format!("{ORD}\npub struct Page<T> {{ pub items: Vec<T> }}\n#[derive(PartialEq, PartialOrd)]\npub struct F(pub f64);\npub fn k(a: Page<F>, b: Page<F>) -> bool {{ a < b }}"),
        "`F` derives `PartialOrd` without `Ord`",
    );
}

#[test]
fn a_hand_written_order_has_one_shape() {
    assert_parse_rejects(
        "use std::cmp::Ordering;\n#[derive(PartialEq, Eq, PartialOrd)]\npub struct W { pub n: u8 }\nimpl Ord for W { fn cmp(&self, other: &Self) -> Ordering { other.n.cmp(&self.n) } }",
        "`W` derives `PartialOrd` and writes `impl Ord`: `<` would compare field by field and `cmp` by hand",
    );
    assert_parse_rejects(
        "use std::cmp::Ordering;\n#[derive(PartialEq, Eq, Ord)]\npub struct W { pub n: u8 }\nimpl PartialOrd for W { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { self.n.partial_cmp(&other.n) } }",
        "`impl PartialOrd` is taken in v0 only as `fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }`",
    );
    assert_parse_rejects(
        "use std::cmp::Ordering;\n#[derive(PartialEq, Eq, Ord)]\npub struct W { pub n: u8 }\nimpl PartialOrd for W { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { None } }",
        "`impl PartialOrd` is taken in v0 only as",
    );
    assert_parse_rejects(
        "use std::cmp::Ordering;\n#[derive(PartialEq, Eq, PartialOrd)]\npub struct W { pub n: u8 }\nimpl Ord for W { fn cmp(&self, o: &Self) -> Ordering { self.n.cmp(&o.n) } fn max(self, o: Self) -> Self { o } }",
        "derives `PartialOrd` and writes `impl Ord`",
    );
    assert_parse_rejects(
        "use std::cmp::Ordering;\n#[derive(PartialEq, Eq)]\npub struct W { pub n: u8 }\nimpl Ord for W { fn cmp(&self, o: &Self) -> Ordering { self.n.cmp(&o.n) } fn max(self, o: Self) -> Self { o } }",
        "`impl Ord` has only `fn cmp` in v0",
    );
}

#[test]
fn the_companion_holds_one_cmp() {
    let found = diagnostics(&format!(
        "use std::cmp::Ordering;\n{ORD}\npub struct V {{ pub n: u8 }}\nimpl V {{ pub fn cmp(&self, n: u8) -> Ordering {{ self.n.cmp(&n) }} }}"
    ));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].message.contains("`V` derives `Ord` and has its own method `cmp`"), "{}", found[0].message);
    assert_eq!(found[0].item, 0, "points at the struct");
    // The crate's own `Ordering` hides std's, which the derived `cmp` gives.
    assert_rejects(
        &format!("pub enum Ordering {{ Before, After }}\n{ORD}\npub struct V {{ pub n: u8 }}\npub fn f(a: V, b: V) -> bool {{ a < b }}"),
        "`V`'s derived `cmp` gives `std::cmp::Ordering`, which the crate's own `Ordering` hides",
    );
}
