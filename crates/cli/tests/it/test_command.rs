//! `purecrate-ts test` (design/01 §8.4): a user's crate called in Rust and
//! in the generated TypeScript on drawn arguments. It passes on an example,
//! reaches private fields and nested modules, builds a closed type through
//! its constructor, and fails on a stated non-equivalence: `usize` past
//! 2^53 - 1, where Rust goes on and the TS `number` throws (design/01 §3).

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use crate::support;

fn test(krate: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_purecrate-ts"))
        .arg("test")
        .arg(krate)
        .args(flags)
        .output()
        .expect("run purecrate-ts")
}

fn skip() -> bool {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        eprintln!("PURECRATE_SKIP_NODE set: skipping purecrate-ts test");
        return true;
    }
    false
}

#[test]
fn an_example_agrees() {
    if skip() {
        return;
    }
    let out = test(&support::repo().join("examples/ulid"), &["--cases", "30"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    // `Ulid` is closed: every method on it needs one made by `parse`, `new`,
    // or `from_bytes` first.
    for name in ["Ulid.parse", "Ulid.encode", "Ulid.toBytes", "next", "compare"] {
        assert!(stdout.contains(&format!("ok        {name} (30 cases)")), "{name}:\n{stdout}");
    }
}

#[test]
fn a_disagreement_fails_and_names_the_call() {
    if skip() {
        return;
    }
    let dir = support::scratch("test-command");
    let src = dir.join("src");
    fs::create_dir_all(src.join("shapes")).expect("mkdir");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"shapes-test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[profile.release]\noverflow-checks = true\n",
    )
    .expect("write");
    fs::write(
        src.join("lib.rs"),
        "mod shapes;\n\npub use shapes::{area, Shape};\n\npub fn double(n: usize) -> usize {\n    n * 2\n}\n",
    )
    .expect("write");
    // A closed type with a private field, in a private module two levels
    // down, inside an enum the function under test takes.
    fs::write(
        src.join("shapes.rs"),
        "mod inner;\n\npub use inner::Side;\n\npub enum Shape {\n    Square(Side),\n    Rect { w: i64, h: i64 },\n    Dot,\n}\n\n\
         pub fn area(s: Shape) -> i64 {\n    match s {\n        Shape::Square(side) => side.len() * side.len(),\n        Shape::Rect { w, h } => w * h,\n        Shape::Dot => 0,\n    }\n}\n",
    )
    .expect("write");
    fs::write(
        src.join("shapes/inner.rs"),
        "pub struct Side {\n    len: i64,\n}\n\nimpl Side {\n    pub fn new(len: i64) -> Option<Side> {\n        if len >= 0 {\n            Some(Side { len })\n        } else {\n            None\n        }\n    }\n\n\
         pub fn len(&self) -> i64 {\n        self.len\n    }\n}\n",
    )
    .expect("write");
    let out = test(&dir, &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("DIFFERS   double"), "{stderr}");
    assert!(stderr.contains("ts:   panic(attempt to multiply with overflow)"), "{stderr}");
    for name in ["area", "Side.new", "Side.len"] {
        assert!(stderr.contains(&format!("ok        {name} (100 cases)")), "{name}:\n{stderr}");
    }
    // The same seed draws the same calls.
    let again = test(&dir, &[]);
    assert_eq!(String::from_utf8_lossy(&again.stderr), stderr);
    fs::remove_dir_all(&dir).ok();
}

/// A function with a crate trait's bound is called at each public type
/// that implements it (design/01 §7.25), and one whose bound no type
/// implements is skipped, saying so.
#[test]
fn a_bounded_function_is_called_at_each_implementor() {
    if skip() {
        return;
    }
    let dir = support::scratch("test-command-traits");
    fs::create_dir_all(&dir).expect("mkdir");
    let src = dir.join("lib.rs");
    fs::write(
        &src,
        "pub trait Priced {\n    fn price(&self) -> i64;\n\n    fn doubled(&self) -> i64 {\n        self.price() * 2\n    }\n}\n\n\
         pub trait Unused {\n    fn never(&self) -> u8;\n}\n\n\
         pub struct Item {\n    pub price: i64,\n}\n\n\
         impl Priced for Item {\n    fn price(&self) -> i64 {\n        self.price\n    }\n}\n\n\
         pub enum Coin {\n    Small,\n    Big(i32),\n}\n\n\
         impl Priced for Coin {\n    fn price(&self) -> i64 {\n        match self {\n            Coin::Small => 1,\n            Coin::Big(n) => i64::from(*n),\n        }\n    }\n}\n\n\
         pub fn total<T: Priced>(xs: &[T]) -> i64 {\n    let mut t = 0i64;\n    for x in xs {\n        t += x.doubled();\n    }\n    t\n}\n\n\
         pub fn lonely<T: Unused>(x: &T) -> u8 {\n    x.never()\n}\n",
    )
    .expect("write");
    let out = test(&src, &["--cases", "20", "--allow-overflow-wrap"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    for name in ["total<Item>", "total<Coin>", "Item.price", "Coin.price"] {
        assert!(stdout.contains(&format!("ok        {name} (20 cases)")), "{name}:\n{stdout}");
    }
    assert!(
        stdout.contains(
            "skipped   lonely: no public type of the crate that can be drawn implements `Unused`, the bound of `T`"
        ),
        "{stdout}"
    );
    fs::remove_dir_all(&dir).ok();
}

/// A function with std bounds alone (`Ord`, `Display`, `Default`) is
/// called at `i32`, its dictionaries written by the driver; one with a
/// crate trait's bound too at each type that has both.
#[test]
fn a_std_bound_is_called_at_i32() {
    if skip() {
        return;
    }
    let dir = support::scratch("test-command-std-bounds");
    fs::create_dir_all(&dir).expect("mkdir");
    let src = dir.join("lib.rs");
    fs::write(
        &src,
        "use std::fmt;\n\n\
         pub trait Named {\n    fn name(&self) -> String;\n}\n\n\
         #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]\npub struct Tag {\n    pub n: u8,\n}\n\n\
         impl Named for Tag {\n    fn name(&self) -> String {\n        format!(\"tag{}\", self.n)\n    }\n}\n\n\
         pub fn largest<T: Ord + Clone>(xs: &[T]) -> Option<T> {\n    xs.iter().max().cloned()\n}\n\n\
         pub fn show<T: fmt::Display>(x: T, n: u8) -> String {\n    format!(\"{x}/{n}\")\n}\n\n\
         pub fn zero<T: Default>() -> T {\n    T::default()\n}\n\n\
         pub fn best<T: Named + Ord>(a: T, b: T) -> String {\n    if a < b {\n        b.name()\n    } else {\n        a.name()\n    }\n}\n",
    )
    .expect("write");
    let out = test(&src, &["--cases", "20"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}{}", String::from_utf8_lossy(&out.stderr));
    for name in ["largest<i32>", "show<i32>", "zero<i32>", "best<Tag>"] {
        assert!(stdout.contains(&format!("ok        {name} (20 cases)")), "{name}:\n{stdout}");
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_crate_that_uses_uuid_is_refused() {
    let out = test(&support::repo().join("examples/raft"), &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("uses `uuid`"), "{stderr}");
}
