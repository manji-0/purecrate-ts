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
fn receiver_calls_elsewhere_are_rejected_with_the_method_name() {
    assert_rejects("pub fn f(x: i32) -> i32 { x.abs() }", "`.abs()` on `i32` is not in v0");
    assert_rejects(&format!("{HAND} pub fn f(c: Card) -> bool {{ c.missing() }}"), "`.missing()` on `Card`");
    assert_rejects(&format!("{HAND} pub fn f(a: Card) -> bool {{ a.beats() }}"), "takes 1 argument(s) after the receiver, got 0");
    assert_rejects(&format!("{HAND} pub fn f(c: Card) -> i32 {{ c.is_face() }}"), "expected `i32`, found `bool`");
}
