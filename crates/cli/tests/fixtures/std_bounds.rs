// Std bounds that carry a dictionary: `T: Ord` and `T: PartialOrd` order a
// `T` through its `cmp`, `T: Display` prints it through its `toString`,
// and `T: Default` makes one through its `default`. A call passes a crate
// type's companion, a primitive's dictionary the crate emits (`ordI32`), or
// a record built from a comparator; a generic type derives `Ord` and
// `Default` with its parameters' dictionaries.

use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Level {
    #[default]
    Low,
    Mid,
    High,
}

/// Ordered by `key` alone, so two of one key are equal and show which of
/// them `max`, `min`, and a stable `sort` keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tagged {
    pub key: u8,
    pub tag: String,
}

impl Ord for Tagged {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

impl PartialOrd for Tagged {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Its `cmp` adds one first: it overflows at `u8::MAX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bumped(pub u8);

impl Ord for Bumped {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.0 + 1).cmp(&(other.0 + 1))
    }
}

impl PartialOrd for Bumped {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Money {
    pub cents: i64,
    pub unit: String,
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{:02} {}", self.cents / 100, self.cents % 100, self.unit)
    }
}

/// Generic, deriving `Ord` and `Default`: its `cmp` and `default` take
/// `T`'s dictionaries.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Ranked<T> {
    pub rank: u8,
    pub value: T,
    pub rest: Vec<T>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tree<T> {
    Leaf,
    Node(Box<Tree<T>>, T, Box<Tree<T>>),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Holder {
    pub ranked: Ranked<u32>,
    pub level: Level,
}

pub fn largest<T: Ord + Clone>(xs: &[T]) -> Option<T> {
    xs.iter().max().cloned()
}

pub fn smallest<T: Ord + Clone>(xs: &[T]) -> Option<T> {
    xs.iter().min().cloned()
}

/// The greatest by a loop and `>`: the first of equals.
pub fn first_largest<T>(xs: &[T]) -> Option<T>
where
    T: PartialOrd + Clone,
{
    let mut best: Option<T> = None;
    for x in xs {
        best = match best {
            None => Some(x.clone()),
            Some(b) => Some(if *x > b { x.clone() } else { b }),
        };
    }
    best
}

pub fn compare<T: Ord>(a: &T, b: &T) -> (Ordering, bool, bool, bool, bool) {
    (a.cmp(b), a < b, a <= b, a > b, a >= b)
}

pub fn picks<T: Ord + Clone>(a: &T, b: &T) -> (T, T) {
    (a.clone().max(b.clone()), a.clone().min(b.clone()))
}

/// An `Option`'s `clamp` panics with std's one message, whatever `T` is.
pub fn clamped<T: Ord + Clone>(x: &T, lo: &T, hi: &T) -> Option<T> {
    Some(x.clone()).clamp(Some(lo.clone()), Some(hi.clone()))
}

pub fn sorted<T: Ord + Clone>(xs: &[T]) -> Vec<T> {
    let mut v: Vec<T> = xs.iter().cloned().collect();
    v.sort();
    v
}

/// `Option`, tuple, and `Vec` of a `T` order through its dictionary too.
pub fn wrapped<T: Ord + Clone>(a: &T, b: &T) -> (bool, Ordering, bool) {
    let some = Some(a.clone()) < Some(b.clone());
    let (one, zero): (u8, u8) = (1, 0);
    let tuple = (a.clone(), one).cmp(&(b.clone(), zero));
    let list = vec![a.clone(), b.clone()] > vec![b.clone()];
    (some, tuple, list)
}

pub fn best_by_key<T: Ord + Clone>(xs: &[(T, u8)]) -> Option<(T, u8)> {
    xs.iter().max_by_key(|p| p.0.clone()).cloned()
}

/// Forwards its dictionary only to itself.
pub fn count_down<T: Ord>(xs: &[T], n: u32) -> usize {
    if n == 0 {
        xs.len()
    } else {
        count_down(xs, n - 1)
    }
}

/// Forwards its dictionary to itself and uses it.
pub fn rank_of<T: Ord>(xs: &[T], x: &T, at: usize) -> usize {
    if at >= xs.len() {
        at
    } else if xs[at] >= *x {
        at
    } else {
        rank_of(xs, x, at + 1)
    }
}

pub struct Shelf<T> {
    pub items: Vec<T>,
}

impl<T: Ord + Clone> Shelf<T> {
    pub fn top(&self) -> Option<T> {
        largest(&self.items)
    }
}

pub fn show<T: fmt::Display>(x: &T) -> String {
    format!("<{}>", x)
}

pub fn show_twice<T: fmt::Display>(x: &T, n: u32) -> String {
    let next = n + 1;
    format!("{x}:{n}/{0}|{next}", x)
}

pub fn named(x: impl fmt::Display) -> String {
    x.to_string()
}

pub fn zero<T: Default>() -> T {
    T::default()
}

pub fn zeros<T: Default>() -> (T, T) {
    (Default::default(), T::default())
}

pub fn ints(xs: Vec<i32>, a: i32, b: i32) -> (Option<i32>, Option<i32>, Option<i32>, (Ordering, bool, bool, bool, bool)) {
    (largest(&xs), smallest(&xs), first_largest(&xs), compare(&a, &b))
}

pub fn wide(xs: Vec<u64>, x: u64, lo: u64, hi: u64) -> (Vec<u64>, Option<u64>, (u64, u64)) {
    let lowest = smallest(&xs).unwrap_or(0);
    (sorted(&xs), clamped(&x, &lo, &hi), picks(&lowest, &x))
}

pub fn texts(xs: Vec<String>, a: String, b: String) -> (Option<String>, (Ordering, bool, bool, bool, bool), Vec<String>) {
    (largest(&xs), compare(&a, &b), sorted(&xs))
}

pub fn chars(a: char, b: char) -> ((char, char), (bool, Ordering, bool)) {
    (picks(&a, &b), wrapped(&a, &b))
}

pub fn bools(xs: Vec<bool>) -> (Option<bool>, Vec<bool>) {
    (largest(&xs), sorted(&xs))
}

pub fn levels(xs: Vec<Level>, a: Level, b: Level) -> (Option<Level>, Vec<Level>, (bool, Ordering, bool), Option<Level>) {
    (largest(&xs), sorted(&xs), wrapped(&a, &b), clamped(&a, &Level::Mid, &b))
}

pub fn tagged_ones(xs: Vec<Tagged>) -> (Option<Tagged>, Option<Tagged>, Option<Tagged>, Vec<Tagged>) {
    (largest(&xs), smallest(&xs), first_largest(&xs), sorted(&xs))
}

pub fn bumped_ones(xs: Vec<Bumped>) -> Option<Bumped> {
    largest(&xs)
}

pub fn pairs(xs: Vec<(u8, char)>) -> (Option<(u8, char)>, Vec<(u8, char)>) {
    (largest(&xs), sorted(&xs))
}

pub fn lists(xs: Vec<Vec<i32>>) -> (Option<Vec<i32>>, Vec<Vec<i32>>) {
    (largest(&xs), sorted(&xs))
}

pub fn ranked_ones(xs: Vec<Ranked<u32>>, a: Ranked<u32>, b: Ranked<u32>) -> (Option<Ranked<u32>>, bool, Ranked<u32>) {
    let less = a < b;
    (largest(&xs), less, a.max(b))
}

pub fn ranked_texts(xs: Vec<Ranked<String>>) -> Vec<Ranked<String>> {
    sorted(&xs)
}

pub fn trees(a: Tree<i64>, b: Tree<i64>) -> (Ordering, Option<Tree<i64>>) {
    let both = vec![a.clone(), b.clone()];
    (a.cmp(&b), largest(&both))
}

pub fn holders(a: Holder, b: Holder) -> (Ordering, Holder) {
    (a.cmp(&b), Holder::default())
}

pub fn keyed(xs: Vec<(String, u8)>) -> Option<(String, u8)> {
    best_by_key(&xs)
}

pub fn counted(xs: Vec<Level>, n: u32) -> (usize, usize) {
    (count_down(&xs, n), rank_of(&xs, &Level::Mid, 0))
}

pub fn shelf_top(items: Vec<i64>) -> Option<i64> {
    Shelf { items }.top()
}

pub fn shown(n: i32, b: bool, c: char, s: String, m: Money) -> (String, String, String, String, String) {
    (show(&n), show(&b), show(&c), show(&s), show(&m))
}

pub fn shown_more(n: u8, k: u32, s: String, m: Money) -> (String, String, String, String) {
    (show_twice(&n, k), show_twice(&m, 0), named(s.as_str()), named(m))
}

pub fn defaults() -> (i64, String, bool, char, u8) {
    (zero(), zero(), zero(), zero(), zero())
}

pub fn crate_defaults() -> (Level, Money, Ranked<u32>, (u16, u16), Holder) {
    let h: Holder = zero();
    (zero(), zero(), zero(), zeros(), h)
}
