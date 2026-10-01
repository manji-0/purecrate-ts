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
