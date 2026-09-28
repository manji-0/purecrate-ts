pub struct Id(u64);

pub struct Label(String);

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

pub struct Floats {
    pub x: f32,
    pub y: f64,
}

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

pub enum Shape {
    Dot,
    Circle(f64),
    Rect(i32, i32),
    Named { label: Label, tag: Option<u8> },
    Tagged(Id),
}

pub enum Tree {
    Leaf,
    Node(Box<Tree>, i32, Box<Tree>),
}

pub struct Chain {
    pub value: i32,
    pub next: Option<Box<Chain>>,
}

pub struct Holder {
    pub tree: Tree,
    pub shapes: Vec<Shape>,
    pub first: Option<Shape>,
    pub chain: Chain,
    pub floats: Floats,
    pub misc: Misc,
}
