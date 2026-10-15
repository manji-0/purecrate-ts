// `PartialOrd` and `Ord` on the crate's types (design/01 §7.25): a derived
// `cmp` on structs (fields in order, an equal prefix deciding nothing),
// newtypes, enums (variants by discriminant, written or by place, then
// their fields), and types that hold each other through `Option`, `Vec`,
// tuples, and `Box`; a hand-written `impl Ord` with `PartialOrd` as
// `Some(self.cmp(other))`, whose ties tell which of two equals std keeps;
// and every use: `<` .. `>=`, `cmp`, `max`, `min`, `clamp` (and its
// panic), `sort`, `sort_by`, `sort_by_key`, iterator `max` / `min` /
// `max_by_key`, and `cmp` on `Option`, `Vec`, and tuples of them.

use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: i64,
    pub pre: Option<String>,
    pub build: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id(pub u16);

/// No field: every two are equal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unit {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shape {
    Dot,
    Circle(i32),
    Rect { w: u8, h: u8 },
    Named(String, char, bool),
}

/// Written discriminants order the variants, not their place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    High = 30,
    Low = -10,
    Mid = 20,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Small {
    B = 2,
    A = 1,
}

/// One variant: both sides are it, and its fields decide.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Solo {
    Only(u8, String),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tree {
    Leaf(Id),
    Node(Box<Tree>, Vec<Tree>),
}

/// Holds the others: a crate type, an `Option` and a `Vec` of one, a tuple.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Entry {
    pub level: Level,
    pub shape: Option<Shape>,
    pub ids: Vec<Id>,
    pub pair: (Small, String),
}

/// Ordered by hand by its text's length only, so two of one length are
/// equal but tell apart by `tag`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByLen {
    pub text: String,
    pub tag: u8,
}

impl Ord for ByLen {
    fn cmp(&self, other: &Self) -> Ordering {
        self.text.len().cmp(&other.text.len())
    }
}

impl PartialOrd for ByLen {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Ordered by hand through arithmetic that overflows at 255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bumped {
    pub n: u8,
}

impl PartialOrd for Bumped {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for Bumped {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.n + 1).cmp(&(other.n + 1)).reverse()
    }
}

pub fn ops_version(a: Version, b: Version) -> (Ordering, bool, bool, bool, bool) {
    (a.cmp(&b), a < b, a <= b, a > b, a >= b)
}

pub fn ops_shape(a: Shape, b: Shape) -> (Ordering, bool, bool, bool, bool) {
    (a.cmp(&b), a < b, a <= b, a > b, a >= b)
}

pub fn ops_level(a: Level, b: Level) -> (Ordering, bool, Ordering) {
    (a.cmp(&b), a < b, Small::A.cmp(&Small::B))
}

pub fn ops_entry(a: Entry, b: Entry) -> (Ordering, bool, bool) {
    (a.cmp(&b), a < b, a >= b)
}

pub fn ops_tree(a: Tree, b: Tree) -> (Ordering, bool) {
    (a.cmp(&b), a > b)
}

pub fn ops_solo(a: Solo, b: Solo) -> (Ordering, bool) {
    (a.cmp(&b), a < b)
}

pub fn ops_unit(a: Unit, b: Unit) -> (Ordering, bool, bool) {
    (a.cmp(&b), a < b, a <= b)
}

pub fn ops_id(a: Id, b: Id) -> (Ordering, bool, Id, Id) {
    (a.cmp(&b), a > b, a.max(b), a.min(b))
}

pub fn ops_by_len(a: ByLen, b: ByLen) -> (Ordering, bool, bool, ByLen, ByLen) {
    (a.cmp(&b), a < b, a == b, a.clone().max(b.clone()), a.min(b))
}

pub fn ops_bumped(a: Bumped, b: Bumped) -> (bool, Ordering) {
    (a < b, a.cmp(&b))
}

pub fn sort_bumped(v: Vec<Bumped>) -> Vec<Bumped> {
    let mut v = v;
    v.sort();
    v
}

pub fn clamp_shape(x: Shape, lo: Shape, hi: Shape) -> Shape {
    x.clamp(lo, hi)
}

pub fn clamp_by_len(x: ByLen, lo: ByLen, hi: ByLen) -> ByLen {
    x.clamp(lo, hi)
}

pub fn sorted(v: Vec<Version>) -> (Vec<Version>, Vec<Version>, Vec<Version>) {
    let mut up = v.clone();
    up.sort();
    let mut down = v.clone();
    down.sort_by(|a, b| b.cmp(a));
    let mut same = v;
    same.sort_by(|a, b| a.cmp(b));
    (up, down, same)
}

/// Stable: equal lengths keep their order.
pub fn sorted_by_len(v: Vec<ByLen>) -> Vec<ByLen> {
    let mut v = v;
    v.sort();
    v
}

pub fn sorted_by_key(v: Vec<Entry>) -> Vec<Entry> {
    let mut v = v;
    v.sort_by_key(|e| e.shape.clone());
    v
}

pub fn extremes(v: Vec<ByLen>) -> (Option<ByLen>, Option<ByLen>, Option<ByLen>) {
    let max = v.iter().cloned().max();
    let min = v.iter().cloned().min();
    let by_key = v.iter().cloned().max_by_key(|x| Id(u16::from(x.tag)));
    (max, min, by_key)
}

pub fn extreme_shapes(v: Vec<Shape>) -> (Option<Shape>, Option<Shape>) {
    (v.iter().cloned().max(), v.iter().cloned().min())
}

pub fn compound(a: Option<Shape>, b: Option<Shape>, xs: Vec<Id>, ys: Vec<Id>) -> (Ordering, bool, Ordering, bool) {
    (a.cmp(&b), a < b, xs.cmp(&ys), xs >= ys)
}

pub fn tuples(a: (Level, Option<Id>), b: (Level, Option<Id>)) -> (Ordering, bool, (Level, Option<Id>)) {
    (a.cmp(&b), a <= b, a.max(b))
}

pub fn options(a: Option<u8>, b: Option<u8>, s: Vec<Option<String>>, t: Vec<Option<String>>) -> (Ordering, bool, Ordering) {
    (a.cmp(&b), a > b, s.cmp(&t))
}
