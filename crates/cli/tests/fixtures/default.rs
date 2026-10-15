// `#[derive(Default)]` and `impl Default` become the companion member
// `default`, called by `X::default()`, `Default::default()` where the type
// is known, and `..Default::default()`. Each field's default has its
// type's TS form: `bigint` for the 64- and 128-bit integers.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Widths {
    pub a: i8,
    pub b: i16,
    pub c: i32,
    pub d: i64,
    pub e: i128,
    pub f: u8,
    pub g: u16,
    pub h: u32,
    pub i: u64,
    pub j: u128,
    pub k: usize,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Mode {
    Off,
    #[default]
    Auto,
    Manual(u8),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Settings {
    pub name: String,
    pub on: bool,
    pub ratio: f64,
    pub scale: f32,
    pub mark: char,
    pub tags: Vec<String>,
    pub limits: BTreeMap<String, i64>,
    pub seen: BTreeSet<String>,
    pub parent: Option<u64>,
    pub pair: (i64, bool),
    pub widths: Widths,
    pub mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meters(u64);

/// A hand-written `default`.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub width: u32,
    pub height: u32,
    pub title: String,
}

impl Default for Window {
    fn default() -> Self {
        Window { width: 640, height: 4_294_967_294, title: String::from("é") }
    }
}

/// A `default` that panics, to show the fields run before it.
#[derive(Debug, Clone, PartialEq)]
pub struct Strict {
    pub n: i32,
    pub m: i32,
}

impl Default for Strict {
    fn default() -> Self {
        panic!("no default")
    }
}

pub fn all_widths() -> Widths {
    Widths::default()
}

pub fn all_settings() -> Settings {
    let s: Settings = Default::default();
    s
}

pub fn no_meters() -> Meters {
    Default::default()
}

pub fn default_mode() -> Mode {
    Mode::default()
}

pub fn window_of(width: u32) -> Window {
    Window { width, ..Default::default() }
}

pub fn taller(w: u32) -> Window {
    let base = Window::default();
    Window { height: base.height + w, ..base }
}

pub fn strict_of(d: i32) -> Strict {
    Strict { n: 10 / d, ..Default::default() }
}

pub fn named(name: String) -> Settings {
    Settings { name, on: true, ..Default::default() }
}

pub fn is_default(w: Widths) -> bool {
    w == Widths::default()
}

pub fn count(xs: Vec<u8>) -> usize {
    let mut n: usize = Default::default();
    for x in xs {
        if x > 0 {
            n += 1;
        }
    }
    n
}

pub fn first_or_default(xs: Vec<i64>) -> i64 {
    match xs.first() {
        Some(x) => *x,
        None => Default::default(),
    }
}
