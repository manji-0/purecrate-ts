//! A JS string may hold a lone surrogate, which no Rust `str` does
//! (design/01 §6): a `pub` function checks each parameter that may hold a
//! string (one, or one anywhere in an open struct, an enum, an `Option`, a
//! `Vec`, or a tuple) on entry and panics, so the encoding never meets one;
//! a private helper, a closed type, and a type with no string are not
//! checked.

use crate::support;

use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_pack::assemble;
use purecrate_syntax::parse_source;

const SOURCE: &str = r#"
pub fn byte_len(s: &str) -> usize {
    helper(s)
}

fn helper(s: &str) -> usize {
    s.len()
}

pub fn first_len(o: Option<String>, xs: Vec<String>) -> usize {
    match o {
        Some(s) => s.len() + xs.len(),
        None => xs.len(),
    }
}

pub struct Label {
    pub text: String,
}

pub struct Wrapped {
    pub label: Label,
    pub count: u8,
}

pub enum Shape {
    Named(String),
    Tagged { tag: Option<String> },
    Empty,
}

pub struct Count {
    pub n: u8,
}

pub struct Sealed {
    text: String,
}

impl Sealed {
    pub fn new(text: String) -> Sealed {
        Sealed { text }
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }
}

pub fn deep(w: Wrapped, s: Shape, xs: Vec<Option<String>>, o: Option<Vec<String>>, pair: (u8, String)) -> usize {
    let shape = match s {
        Shape::Named(t) => t.len(),
        Shape::Tagged { tag: Some(t) } => t.len(),
        Shape::Tagged { tag: None } | Shape::Empty => 0,
    };
    let inner = match o {
        Some(ys) => ys.len(),
        None => 0,
    };
    let (_, last) = pair;
    w.label.text.len() + shape + xs.len() + inner + last.len()
}

pub fn unchecked(c: Count, s: Sealed) -> usize {
    s.len() + usize::from(c.n)
}
"#;

const DRIVER: &str = r#"
import { byteLen, deep, firstLen, Sealed, unchecked } from "./src/index.ts";
const out = [];
const panics = (f) => {
  try {
    f();
    return "returned";
  } catch (e) {
    return e instanceof Error && e.name === "Panic" ? e.message : `threw ${e}`;
  }
};
const w = (text) => ({ label: { text }, count: 1 });
const named = (value) => ({ kind: "Named", value });
const message = "a string holds a lone surrogate, which no Rust `str` does";
for (const [what, f] of [
  ["lead", () => byteLen("a\ud800")],
  ["trail", () => byteLen("\udc00a")],
  ["reversed pair", () => byteLen("\udc00\ud800")],
  ["in an Option", () => firstLen("\ud800", [])],
  ["in a Vec", () => firstLen(null, ["ok", "\udfff"])],
  ["in a nested struct", () => deep(w("\ud800"), named("a"), [], null, [0, ""])],
  ["in a variant", () => deep(w("a"), named("\ud800"), [], null, [0, ""])],
  ["in a variant's field", () => deep(w("a"), { kind: "Tagged", tag: "\udc00" }, [], null, [0, ""])],
  ["in an Option in a Vec", () => deep(w("a"), named("a"), [null, "\ud800"], null, [0, ""])],
  ["in a Vec in an Option", () => deep(w("a"), named("a"), [], ["\ud800"], [0, ""])],
  ["in a tuple", () => deep(w("a"), named("a"), [], null, [0, "\ud800"])],
  ["through a constructor", () => Sealed.new("\ud800")],
]) {
  const got = panics(f);
  if (got !== message) out.push(`${what}: ${got}`);
}
for (const [what, f, want] of [
  ["a pair", () => byteLen("😀"), 4],
  ["the last scalar", () => byteLen("\u{10ffff}"), 4],
  ["empty", () => byteLen(""), 0],
  ["none and a Vec", () => firstLen(null, ["a", "😀"]), 2],
  ["deep", () => deep(w("ab"), { kind: "Tagged", tag: "😀" }, ["x", null], ["y"], [1, "z"]), 10],
  ["no string", () => unchecked({ n: 3 }, Sealed.new("é")), 5],
]) {
  const got = f();
  if (got !== want) out.push(`${what}: ${got}, want ${want}`);
}
console.log(out.length === 0 ? "ok" : out.join("\n"));
"#;

#[test]
fn a_lone_surrogate_panics_on_entry() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let krate = parse_source("lone", SOURCE).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("rejected: {d:#?}"));
    let dir = support::scratch("lone-surrogate");
    support::write_package(&dir, &assemble(&typed));
    support::typecheck(&dir);
    let helper = fs::read_to_string(dir.join("src/byte-len.ts")).expect("byte-len.ts");
    assert_eq!(helper.matches("Str.wellFormed").count(), 1, "only the pub function checks:\n{helper}");
    let deep = fs::read_to_string(dir.join("src/deep.ts")).expect("deep.ts");
    assert_eq!(deep.matches("Str.wellFormed(").count(), 5, "every parameter holds a string:\n{deep}");
    let unchecked = fs::read_to_string(dir.join("src/unchecked.ts")).expect("unchecked.ts");
    assert!(!unchecked.contains("Str.wellFormed"), "neither holds a string a caller wrote:\n{unchecked}");
    fs::write(dir.join("driver.ts"), DRIVER).expect("write driver");
    let output = Command::new("node")
        .arg(format!("--conditions={}", support::SOURCE_CONDITION))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.trim() == "ok",
        "in {}:\n{stdout}\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&dir);
}
