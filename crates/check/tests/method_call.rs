mod common;

use common::{assert_clean, assert_rejects};

const HAND: &str = "pub enum Suit { Hearts, Spades }
     pub struct Card { pub rank: i32, pub suit: Suit }
     impl Card {
         pub fn is_face(&self) -> bool { self.rank > 10 }
         pub fn beats(&self, other: &Card) -> bool { self.rank > other.rank }
     }
     impl Suit {
         pub fn is_red(&self) -> bool { match self { Suit::Hearts => true, Suit::Spades => false } }
     }";

#[test]
fn receiver_calls_resolve_to_the_crates_own_methods() {
    assert_clean(&format!("{HAND} pub fn f(c: Card) -> bool {{ c.is_face() }}"));
    assert_clean(&format!("{HAND} pub fn f(a: Card, b: Card) -> bool {{ a.beats(&b) && b.suit.is_red() }}"));
    assert_clean(&format!(
        "{HAND} pub type Hand = Card; pub fn f(h: Hand) -> bool {{ let c = Card {{ rank: 1, suit: Suit::Spades }}; h.beats(&c) }}"
    ));
}

#[test]
fn vec_len_and_index_are_accepted() {
    assert_clean("pub fn f(xs: Vec<i32>) -> usize { xs.len() }");
    assert_clean("pub fn f(xs: Vec<i32>, i: usize) -> i32 { xs[i] }");
    assert_clean("pub fn f(xs: &[i32]) -> i32 { if xs.len() == 0 { 0 } else { xs[0] } }");
    assert_rejects("pub fn f(xs: Vec<i32>) -> i32 { xs[0i32] }", "expected `usize`, found `i32`");
    assert_rejects("pub fn f(s: String) -> usize { s.len() }", "`.len()` on `String` is not in v0");
    assert_rejects("pub fn f(s: String) -> i32 { s[0] }", "cannot index `String`");
}

#[test]
fn receiver_calls_elsewhere_are_rejected_with_the_method_name() {
    assert_rejects("pub fn f(x: i32) -> i32 { x.abs() }", "`.abs()` on `i32` is not in v0");
    assert_rejects(&format!("{HAND} pub fn f(c: Card) -> bool {{ c.missing() }}"), "`.missing()` on `Card`");
    assert_rejects(&format!("{HAND} pub fn f(a: Card) -> bool {{ a.beats() }}"), "takes 1 argument(s) after the receiver, got 0");
    assert_rejects(&format!("{HAND} pub fn f(c: Card) -> i32 {{ c.is_face() }}"), "expected `i32`, found `bool`");
}

/// The receiver is typed once per call, so a chain costs time linear in its
/// length. Typing it twice made each link double the work (2^40 here).
#[test]
fn long_method_chains_type_in_linear_time() {
    let chain = ".bump()".repeat(40);
    let src = format!(
        "pub struct C {{ pub n: i32 }}
         impl C {{ pub fn bump(self) -> C {{ C {{ n: self.n }} }} }}
         pub fn f(c: C) -> C {{ c{chain} }}"
    );
    let start = std::time::Instant::now();
    assert_clean(&src);
    assert!(start.elapsed() < std::time::Duration::from_secs(20), "{:?}", start.elapsed());
}
