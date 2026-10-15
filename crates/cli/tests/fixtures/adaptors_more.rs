// `take`, `skip`, `take_while`, and `skip_while` as stages of a sequence,
// and `partition` (design/01 §7.28): each runs as lazily as Rust's, so a
// panic in a stage before it comes at the same item or not at all.

// `take` asks for no item past the `n`th: `6 / 0` on the third never runs,
// and `[1, 2, 0]` sums to 9.
pub fn lazy_take(xs: &[i32], n: usize) -> i32 {
    xs.iter().map(|x| 6 / x).take(n).sum()
}

pub fn first_two(xs: &[i32]) -> i32 {
    xs.iter().map(|x| 6 / x).take(2).sum()
}

// `skip` pulls the items it drops: the stages before it run on them.
pub fn skipped(xs: &[i32], n: usize) -> usize {
    xs.iter().map(|x| 6 / x).skip(n).count()
}

// `take_while` pulls the first item it refuses, so `map` runs on it.
pub fn while_big(xs: &[i32]) -> i32 {
    xs.iter().map(|x| 12 / x).take_while(|q| *q > 2).sum()
}

// `skip_while` does not call its predicate after its first refusal.
pub fn after_big(xs: &[i32]) -> i32 {
    xs.iter().copied().skip_while(|x| 12 / x > 2).sum()
}

// A predicate that panics on the first item it sees.
pub fn first_checked(xs: &[u8]) -> usize {
    xs.iter().take_while(|x| **x + 200 > 0).count()
}

// The receiver is evaluated before `n`: the slice's panic comes first.
pub fn sliced_first(xs: &[u8], from: usize, n: usize) -> usize {
    xs[from..].iter().take(n - 1).count()
}

// Stages after a cut, and cuts after cuts.
pub fn middle(xs: &[u32], a: usize, b: usize) -> Vec<u32> {
    xs.iter().skip(a).take(b).map(|x| x * 2).collect()
}

pub fn odd_after(xs: &[u32], n: usize) -> Option<u32> {
    xs.iter().copied().skip(n).filter(|x| x % 2 == 1).take(1).max()
}

// Over a string's chars: non-ASCII ones count as one each.
pub fn chars_cut(s: &str, a: usize, b: usize) -> String {
    s.chars().skip(a).take(b).collect()
}

pub fn no_digits(s: &str) -> String {
    s.chars().skip_while(|c| c.is_ascii_digit()).take_while(|c| *c != ';').collect()
}

pub fn leading_bytes(s: &str) -> u32 {
    s.bytes().take_while(|b| matches!(b, b'a'..=b'z')).map(|b| u32::from(b) + 1).sum()
}

// In a `for`, its stages run as the loop asks.
pub fn walked(xs: &[u8], n: usize) -> u32 {
    let mut s = 0u32;
    for x in xs.iter().skip(1).take(n) {
        s += u32::from(*x);
    }
    s
}

pub fn walked_while(xs: Vec<String>) -> usize {
    let mut n = 0;
    for s in xs.iter().take_while(|s| !s.is_empty()) {
        n += s.len();
    }
    n
}

// A loop whose body does nothing still runs `map` on each item `take` gives.
#[allow(unused_variables)]
pub fn drained(xs: &[i32], n: usize) -> i32 {
    for q in xs.iter().map(|x| 6 / x).take(n) {}
    0
}

// `partition`: the result's type is the function's.
pub fn signs(xs: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
    xs.into_iter().partition(|x| *x > 0)
}

// Or a typed `let`, after stages that may panic.
pub fn split_doubled(xs: &[u32], k: u32) -> (usize, u32) {
    let (big, small): (Vec<u32>, Vec<u32>) = xs.iter().map(|x| x * 2).partition(|x| *x > k);
    (big.len(), small.iter().sum())
}

pub fn long_words(xs: Vec<String>) -> (Vec<String>, Vec<String>) {
    xs.into_iter().take(4).partition(|s| s.chars().count() > 2)
}

pub fn digits_apart(s: &str) -> (Vec<char>, Vec<char>) {
    s.chars().partition(|c| c.is_ascii_digit())
}

// On a range, `take` and `skip` keep it a range: computed, so a long one
// costs nothing, and `rev` and `step_by` compose with them in order.
pub fn range_window(n: u64, a: usize, b: usize) -> u64 {
    (3..n).skip(a).take(b).sum()
}

pub fn range_back(n: i32, k: usize) -> Vec<i32> {
    (0..n).take(k).rev().step_by(2).collect()
}

pub fn range_cut_stepped(a: u8, b: u8, k: usize, n: usize) -> Vec<u8> {
    (a..=b).step_by(k).skip(n).collect()
}

pub fn range_down(a: i64, b: i64, n: usize) -> Vec<i64> {
    (a..b).rev().skip(n).take(3).collect()
}

pub fn range_far(n: usize) -> u32 {
    (0..4_294_967_295u32).skip(n).take(2).fold(0u32, |a, x| a.wrapping_add(x))
}

pub fn range_skip_rev(a: u16, b: u16, n: usize) -> Option<u16> {
    (a..b).skip(n).rev().max()
}

pub fn range_for(n: u32, k: usize) -> u32 {
    let mut s = 0u32;
    for i in (0..n).skip(k).take(3) {
        s += i;
    }
    s
}
