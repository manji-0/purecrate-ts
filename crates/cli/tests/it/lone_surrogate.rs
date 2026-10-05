//! A JS string may hold a lone surrogate, which no Rust `str` does
//! (design/01 §6): a `pub` function checks each `String` / `&str`
//! parameter (and each in an `Option` or a `Vec`) on entry and panics, so
//! the encoding never meets one; a private helper is not checked again.

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
"#;

const DRIVER: &str = r#"
import { byteLen, firstLen } from "./src/index.ts";
const out = [];
const panics = (f) => {
  try {
    f();
    return "returned";
  } catch (e) {
    return e instanceof Error && e.name === "Panic" ? e.message : `threw ${e}`;
  }
};
const message = "a string holds a lone surrogate, which no Rust `str` does";
for (const [what, f] of [
  ["lead", () => byteLen("a\ud800")],
  ["trail", () => byteLen("\udc00a")],
  ["reversed pair", () => byteLen("\udc00\ud800")],
  ["in an Option", () => firstLen("\ud800", [])],
  ["in a Vec", () => firstLen(null, ["ok", "\udfff"])],
]) {
  const got = panics(f);
  if (got !== message) out.push(`${what}: ${got}`);
}
for (const [what, f, want] of [
  ["a pair", () => byteLen("😀"), 4],
  ["the last scalar", () => byteLen("\u{10ffff}"), 4],
  ["empty", () => byteLen(""), 0],
  ["none and a Vec", () => firstLen(null, ["a", "😀"]), 2],
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
