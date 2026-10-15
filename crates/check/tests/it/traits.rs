//! Traits (design/01 §7.25): what a trait, an `impl` of one, and a bound
//! may hold, and the forms refused.

use crate::common::{assert_clean, assert_parse_rejects, assert_rejects};

const PRICED: &str = "pub trait Priced { const RATE: i64 = 2; fn price(&self) -> i64; fn doubled(&self) -> i64 { self.price() * Self::RATE } fn zero() -> Self; }\n\
     pub struct Item { pub price: i64 }\n\
     impl Priced for Item { fn price(&self) -> i64 { self.price } fn zero() -> Self { Item { price: 0 } } }\n";

fn with(rest: &str) -> String {
    format!("{PRICED}{rest}")
}

#[test]
fn traits_impls_and_bounds_are_accepted() {
    assert_clean(&with(
        "pub fn f(i: &Item) -> i64 { i.price() + Item::price(i) + Priced::price(i) + i.doubled() + Item::RATE }",
    ));
    assert_clean(&with("pub fn total<T: Priced>(xs: &[T]) -> i64 { let mut t = 0; for x in xs { t += x.doubled(); } t + T::RATE + T::zero().price() }\npub fn f(xs: Vec<Item>) -> i64 { total(&xs) }"));
    assert_clean(&with("pub fn g(x: impl Priced) -> i64 { x.price() }\npub fn f(i: Item) -> i64 { g(i) }"));
    assert_clean(&with("pub trait Named: Priced { fn name(&self) -> String; }\nimpl Named for Item { fn name(&self) -> String { String::new() } }\nfn total<T: Priced>(x: &T) -> i64 { x.price() }\npub fn f<T: Named>(x: &T) -> i64 { total(x) + x.price() }"));
    assert_clean(&with("pub struct Shelf<T> { pub items: Vec<T> }\nimpl<T: Priced> Shelf<T> { pub fn first(&self) -> i64 { self.items.first().map(|x| x.price()).unwrap_or(0) } }"));
    assert_clean(&with("pub struct Bag<T> { pub items: Vec<T> }\nimpl<T: Clone> Priced for Bag<T> { fn price(&self) -> i64 { 1 } fn zero() -> Self { Bag { items: Vec::new() } } }"));
    assert_clean(&with("pub fn h<T>(x: &T) -> i64 where T: Priced { x.price() }"));
}

#[test]
fn a_trait_is_over_self_alone() {
    assert_parse_rejects("pub trait T { type A; }", "the associated type `A` of trait `T` is not in v0");
    assert_parse_rejects("pub trait Conv<T> { fn conv(&self) -> T; }", "the trait `Conv` has type parameters");
    assert_parse_rejects("pub trait T { fn f<U>(&self, u: U) -> U; }", "the trait method `f` has type parameters");
    assert_parse_rejects("pub trait T where Self: Sized { fn f(&self) -> u8; }", "on a trait is not in v0");
    assert_parse_rejects("pub trait T { fn f(&self) -> u8 where Self: Sized; }", "on a trait method is not in v0");
    assert_parse_rejects("pub trait T { fn f(&mut self); }", "`&mut self` is not in v0");
    assert_parse_rejects("pub trait T: Clone { fn f(&self) -> u8; }", "the supertrait `Clone` is not in v0");
    assert_parse_rejects("pub trait Marked {}", "the trait `Marked` has no methods or consts");
    assert_parse_rejects("pub trait T { unsafe fn f(&self) -> u8; }", "unsafe is not allowed in v0");
}

#[test]
fn an_impl_is_on_a_type_of_the_crate() {
    assert_parse_rejects(
        &with("impl Priced for u32 { fn price(&self) -> i64 { 0 } fn zero() -> Self { 0 } }"),
        "`impl Priced for u32` is not in v0",
    );
    assert_parse_rejects(
        &with("impl Priced for Vec<Item> { fn price(&self) -> i64 { 0 } fn zero() -> Self { Vec::new() } }"),
        "`impl Priced for Vec<Item>` is not in v0",
    );
    assert_parse_rejects(
        "pub trait Base { fn b(&self) -> u8; }\npub trait Tr { fn t(&self) -> u8; }\nimpl<T: Base> Tr for T { fn t(&self) -> u8 { self.b() } }",
        "the blanket impl",
    );
    assert_parse_rejects(
        &with("pub struct Page<T> { pub items: Vec<T> }\nimpl<T: Priced> Priced for Page<T> { fn price(&self) -> i64 { 0 } fn zero() -> Self { Page { items: Vec::new() } } }"),
        "its dictionary would be a function of `T`'s",
    );
}

#[test]
fn a_name_the_companion_holds_is_refused() {
    let tr = "pub trait Tr { fn go(&self) -> u8; }\n";
    assert_parse_rejects(
        &format!("{tr}pub struct S {{}}\nimpl S {{ pub fn go(&self) -> u8 {{ 1 }} }}\nimpl Tr for S {{ fn go(&self) -> u8 {{ 2 }} }}"),
        "a method or const of an inherent `impl S`",
    );
    assert_parse_rejects(
        &format!("{tr}pub enum E {{ go }}\nimpl Tr for E {{ fn go(&self) -> u8 {{ 2 }} }}"),
        "the variant `E::go`",
    );
    assert_parse_rejects(
        "pub trait Tr { fn of(&self) -> u8; }\npub struct S {}\nimpl Tr for S { fn of(&self) -> u8 { 2 } }",
        "the constructor `of`",
    );
    assert_parse_rejects(
        &format!("{tr}pub trait Other {{ fn go(&self) -> u8 {{ 3 }} }}\npub struct S {{}}\nimpl Other for S {{}}\nimpl Tr for S {{ fn go(&self) -> u8 {{ 2 }} }}"),
        "already holds a member of `impl Tr for S`",
    );
    assert_parse_rejects(
        "pub trait Tr { const N: u8; }\npub struct S {}\nimpl S { pub const N: u8 = 1; }\nimpl Tr for S { const N: u8 = 2; }",
        "a method or const of an inherent `impl S`",
    );
}

#[test]
fn a_name_a_std_trait_puts_on_the_companion_is_refused() {
    let derived = "use std::cmp::Ordering;\npub trait Tr { fn cmp(&self, o: &Self) -> Ordering { Ordering::Equal } }\n\
         #[derive(PartialEq, Eq, PartialOrd, Ord)]\npub struct S { pub n: u8 }\nimpl Tr for S {}";
    assert_parse_rejects(derived, "the `cmp` that `#[derive(PartialOrd)]` gives");
    assert_parse_rejects(
        "pub trait Tr { fn default() -> Self; }\n#[derive(Default)]\npub struct S { pub n: u8 }\nimpl Tr for S { fn default() -> Self { S { n: 1 } } }",
        "the `default` that `#[derive(Default)]` gives",
    );
    assert_parse_rejects(
        "pub trait Tr { fn from_u8(n: u8) -> Self; }\npub struct S { pub n: u8 }\nimpl From<u8> for S { fn from(n: u8) -> Self { S { n } } }\nimpl Tr for S { fn from_u8(n: u8) -> Self { S { n } } }",
        "the `fromU8` that `impl From` gives",
    );
    assert_parse_rejects(
        "use std::fmt;\npub trait Tr { fn to_string(&self) -> String; }\npub struct S { pub n: u8 }\nimpl fmt::Display for S { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, \"s\") } }\nimpl Tr for S { fn to_string(&self) -> String { String::new() } }",
        "the `toString` that `impl fmt::Display` gives",
    );
}

#[test]
fn no_value_is_dispatched_on() {
    assert_parse_rejects(&with("pub fn f(x: &dyn Priced) -> i64 { x.price() }"), "a trait object `dyn Priced`");
    assert_parse_rejects(&with("pub fn f(x: Box<dyn Priced>) -> i64 { x.price() }"), "a trait object `dyn Priced`");
    assert_parse_rejects(&with("pub fn f() -> impl Priced { Item { price: 1 } }"), "`impl Tr` in return position");
    assert_parse_rejects(
        &with("pub fn f(i: &Item) -> i64 { <Item as Priced>::price(i) }"),
        "the qualified path `<Item as Priced>::price` is not in v0",
    );
}

#[test]
fn a_type_parameter_of_a_type_has_no_crate_trait_bound() {
    assert_parse_rejects(
        &with("pub struct Shelf<T: Priced> { pub items: Vec<T> }"),
        "the bound `Priced` on a struct's or enum's type parameter is not in v0",
    );
    assert_parse_rejects(
        &with("pub enum E<T> where T: Priced { A(T) }"),
        "the bound `Priced` on a struct's or enum's type parameter is not in v0",
    );
}

#[test]
fn a_bound_names_what_the_body_calls() {
    assert_rejects(
        &with("pub fn f<T: Priced>(x: &T) -> i64 { T::nothing() }"),
        "no bound of `T` here has a method `nothing`",
    );
}
