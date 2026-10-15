use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct Id(u64);

#[derive(Serialize, Deserialize)]
pub struct Label(String);

#[derive(Serialize, Deserialize)]
pub struct Ints {
    pub a: i8,
    pub b: i16,
    pub c: i32,
    pub d: i64,
    pub e: u8,
    pub f: u16,
    pub g: u32,
    pub h: u64,
    pub i: usize,
}

/// `toJson` writes these without reading the item or the value.
#[derive(Serialize, Deserialize)]
pub struct Units {
    pub r: Vec<()>,
}

#[derive(Serialize, Deserialize)]
pub struct Nothing {}

/// Generic types: each schema a function of its parameters' schemas, each
/// writer of its parameters' writers.
#[derive(Serialize, Deserialize)]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub total: u32,
}

#[derive(Serialize, Deserialize)]
pub enum Choice<A, B> {
    One(A),
    Two { b: B, n: u8 },
    Neither,
}

#[derive(Serialize, Deserialize)]
pub enum Links<T> {
    End,
    Link(T, Box<Links<T>>),
}

/// A generic newtype: its content, as for any newtype.
#[derive(Serialize, Deserialize)]
pub struct Wrapped<T>(pub T);

#[derive(Serialize, Deserialize)]
pub struct Holding {
    pub labels: Paged<Label>,
    pub wrapped: Wrapped<u64>,
    pub wrapped_labels: Wrapped<Vec<Label>>,
    pub nums: Paged<u64>,
    pub pick: Choice<String, Id>,
    pub other: Choice<String, Id>,
    pub none: Choice<String, Id>,
    pub links: Links<i8>,
}

/// A JSON object, keys in Rust's order, and an array of keys.
#[derive(Serialize, Deserialize)]
pub struct Catalog {
    pub counts: BTreeMap<String, u32>,
    pub tags: BTreeSet<String>,
    pub nested: BTreeMap<String, Vec<Label>>,
}

/// Arrays: a JSON array of exactly `N` elements, as serde reads one, up to
/// the 32 serde implements (design/01 §7.27).
#[derive(Serialize, Deserialize)]
pub struct Blocks {
    pub key: [u8; 4],
    pub grid: [[i16; 2]; 2],
    pub names: Option<[String; 2]>,
    pub rows: Vec<[u64; 1]>,
    pub full: [u8; 32],
    pub none: [bool; 0],
}

#[derive(Serialize, Deserialize)]
pub struct Wide {
    pub a: i128,
    pub b: u128,
}

#[derive(Serialize, Deserialize)]
pub struct Floats {
    pub x: f32,
    pub y: f64,
}

#[derive(Serialize, Deserialize)]
pub struct Misc {
    pub flag: bool,
    pub text: String,
    pub unit: (),
    pub maybe: Option<i32>,
    pub list: Vec<i32>,
    pub pair: (i32, String),
    pub id: Id,
    pub labels: Option<Vec<Label>>,
    pub ints: Box<Ints>,
}

#[derive(Serialize, Deserialize)]
pub enum Shape {
    Dot,
    Circle(f64),
    Rect(i32, i32),
    Named { label: Label, tag: Option<u8> },
    Tagged(Id),
}

#[derive(Serialize, Deserialize)]
pub enum Tree {
    Leaf,
    Node(Box<Tree>, i32, Box<Tree>),
}

#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub value: i32,
    pub next: Option<Box<Chain>>,
}

#[derive(Serialize, Deserialize)]
pub struct Holder {
    pub tree: Tree,
    pub shapes: Vec<Shape>,
    pub first: Option<Shape>,
    pub chain: Chain,
    pub floats: Floats,
    pub misc: Misc,
}

#[derive(Serialize, Deserialize)]
pub struct Sealed {
    code: i32,
    pub hint: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Letters {
    pub one: char,
    pub maybe: Option<char>,
    pub many: Vec<char>,
}

#[derive(Serialize, Deserialize)]
pub struct Ids {
    pub one: Uuid,
    pub maybe: Option<Uuid>,
    pub many: Vec<Uuid>,
}

/// Two types that refer to each other: each schema waits for the other.
#[derive(Serialize, Deserialize)]
pub enum Node {
    Leaf(i32),
    Group(Group),
}

#[derive(Serialize, Deserialize)]
pub struct Group {
    pub label: String,
    pub children: Vec<Node>,
}

/// Declared before the type it reads: the schemas are ordered by use.
#[derive(Serialize, Deserialize)]
pub struct Early {
    pub late: Late,
}

#[derive(Serialize, Deserialize)]
pub struct Late {
    pub n: i32,
}
