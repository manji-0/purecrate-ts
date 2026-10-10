use crate::common::{assert_clean, assert_rejects};

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
    // `String::len` is the allow-listed UTF-8 byte count (`str_methods_come_from_the_allow_list`).
    assert_clean("pub fn f(s: String) -> usize { s.len() }");
    assert_rejects("pub fn f(s: String) -> i32 { s[0] }", "cannot index `String`");
}

#[test]
fn receiver_calls_elsewhere_are_rejected_with_the_method_name() {
    assert_rejects(
        "pub fn f(x: i32) -> i32 { x.signum() }",
        "`.signum()` on `i32` is not on the std allow-list; allowed: `min`, `max`, `abs`, `pow`, `checked_add`",
    );
    assert_rejects("pub fn f(x: u32) -> u32 { x.abs() }", "`u32` has no `abs`; it is never negative");
    assert_rejects("pub fn f(x: u32) -> u32 { x.pow(2u8) }", "expected `u32`, found `u8`");
    assert_rejects(
        &format!("{HAND} pub fn f(c: Card) -> bool {{ c.missing() }}"),
        "`Card` has no method `missing` in the crate's own `impl` blocks",
    );
    assert_rejects(
        &format!("{HAND} pub fn f(a: Card) -> bool {{ a.beats() }}"),
        "takes 1 argument(s) after the receiver, got 0",
    );
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

/// The `str` allow-list (design/01 §6): the methods, receivers and needles
/// it names, and nothing else.
#[test]
fn a_string_grows_as_a_local() {
    assert_clean("pub fn f(s: &str) -> String { let mut o = String::new(); o.push('x'); o.push_str(s); o }");
    assert_clean("pub fn f(s: &str) -> String { s.chars().map(|c| c.to_ascii_uppercase()).collect() }");
    assert_rejects(
        "pub struct W { pub s: String } pub fn f(w: W) -> String { let mut v = w; v.s.push('x'); v.s }",
        "`push` grows a local `let mut s: String` in v0",
    );
    assert_rejects(
        "pub fn f() -> String { let mut o = String::new(); o.push(\"x\"); o }",
        "`String::push` takes a `char`",
    );
    assert_rejects(
        "pub fn f(xs: Vec<u8>) -> String { xs.iter().collect::<String>() }",
        "`collect` builds a `String` from `char`s in v0, not `u8`",
    );
}

#[test]
fn str_methods_come_from_the_allow_list() {
    assert_clean("pub fn f(s: String, t: &str) -> bool { s.starts_with(t) && t.ends_with(\"x\") && s.contains(&s) && !t.is_empty() }");
    assert_clean("pub fn f(s: &str) -> usize { s.len() }");
    assert_clean(
        "pub fn f(s: &str, t: String) -> bool { s.eq_ignore_ascii_case(&t) || s.eq_ignore_ascii_case(\"FREQ\") }",
    );
    assert_rejects(
        "pub fn f(s: &str) -> bool { s.eq_ignore_ascii_case('a') }",
        "`str::eq_ignore_ascii_case` takes a `&str` pattern in v0, found `char`",
    );
    assert_rejects(
        "pub fn f(s: &str) -> bool { s.starts_with(1u8) }",
        "`str::starts_with` takes a `&str` pattern in v0, found `u8`",
    );
    assert_rejects(
        "pub fn f(s: &str) -> bool { s.starts_with() }",
        "`str::starts_with` takes 1 argument(s) after the receiver, got 0",
    );
    assert_rejects(
        "pub fn f(s: &str) -> bool { s.is_empty(s) }",
        "`str::is_empty` takes 0 argument(s) after the receiver, got 1",
    );
    assert_rejects("pub fn f(s: &str) -> i32 { s.len() }", "expected `i32`, found `usize`");
    assert_rejects(
        "pub fn f(s: &str) -> bool { s.trim() == \"\" }",
        "`.trim()` on `&str` is not on the std allow-list; allowed: `len`, `is_empty`, `starts_with`, `ends_with`, `contains`, `strip_prefix`, `strip_suffix`, `split_once`, `eq_ignore_ascii_case`, `as_bytes`, `cmp`, `parse`, `clone`, `chars`, `bytes`, `split`, slicing `s[a..b]`",
    );
}

/// A std method outside the allow-list names what the receiver does allow,
/// not the crate's `impl` blocks.
#[test]
fn std_rejections_list_what_the_receiver_allows() {
    assert_rejects(
        "pub fn f(x: Option<u32>) -> u32 { x.unwrap_or_default() }",
        "allowed: `is_some`, `is_none`, `unwrap_or`, `ok_or`, `map`, `clone`, `as_ref`, `as_deref`",
    );
    assert_rejects("pub fn f(xs: Vec<u8>) -> bool { xs.contains(&0u8) }", "allowed: `len`, `is_empty`, `cmp`, `clone`, `iter`, `into_iter`, push and insert (on a `let mut` local), indexing `xs[i]`");
    assert_rejects("pub fn f(c: char) -> bool { c.is_alphabetic() }", "allowed: `is_ascii`, `is_ascii_alphabetic`");
    assert_rejects("pub fn f(b: u8) -> bool { b.is_ascii_digit() }", "use `matches!(b, b'0'..=b'9')`");
}

/// With positions, a rejected call is located at the method name, not at
/// the statement it sits in.
#[test]
fn rejected_calls_point_at_the_call() {
    let src = "pub fn f(x: Option<u32>) -> u32 {\n    let y = 1u32;\n    y + x.unwrap_or_default()\n}\n";
    let (krate, _) = purecrate_syntax::parse_source_spanned("c", src).expect("parse");
    let found = purecrate_check::check(&krate);
    assert_eq!(found.len(), 1, "{found:#?}");
    let at = found[0].at.expect("a position");
    assert_eq!((at.line, at.col), (3, 11), "{found:#?}"); // 1-based: the `u` of `unwrap_or_default`
}

/// `Option::unwrap_or`, `ok_or`, and `map` (`option_methods_equivalence.rs`
/// for their values); what `map` refuses.
#[test]
fn option_combinators_are_checked() {
    assert_clean("pub fn f(x: Option<u8>) -> u8 { x.map(|v| v / 2).unwrap_or(0) }");
    assert_clean("pub fn f(x: Option<u8>) -> Result<u8, u32> { x.ok_or(7u32) }");
    assert_rejects(
        "pub fn g(v: u8) -> Result<u8, u8> { Ok(v) }\npub fn f(x: Option<u8>) -> Result<Option<u8>, u8> { Ok(x.map(|v| g(v)?)) }",
        "a closure passed to `Option::map` may not use `?` or `return`",
    );
    assert_rejects(
        "pub fn g(a: u8, b: u8) -> u8 { a + b }\npub fn f(x: Option<u8>) -> Option<u8> { x.map(g) }",
        "`map` calls its function with 1 argument, which takes 2",
    );
    // A one-field tuple variant is a function (`.map(PreId::Numeric)`).
    assert_clean("pub enum E { A(u8), B } pub fn f(x: Option<u8>) -> Option<E> { x.map(E::A) }");
    assert_rejects(
        "pub enum E { A(u8), B } pub fn f(x: Option<u8>) -> Option<E> { x.map(E::B) }",
        "`Option::map` takes a closure",
    );
    assert_rejects(
        "pub enum E { A(u8, u8) } pub fn f(x: Option<u8>) -> Option<E> { x.map(E::A) }",
        "constructed with the wrong shape",
    );
}
