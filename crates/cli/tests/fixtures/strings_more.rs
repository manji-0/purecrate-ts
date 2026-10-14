// `str` methods that build or cut text: `to_lowercase` / `to_uppercase` by
// full Unicode, `split` with a `&str`, `split_whitespace`, `lines`, `join`,
// `repeat`, and `to_string` on what `format!` prints.

use uuid::Uuid;

pub fn cased(s: &str) -> (String, String) {
    (s.to_lowercase(), s.to_uppercase())
}

/// A `String` receiver, chained.
pub fn cased_owned(s: String) -> String {
    s.to_uppercase().to_lowercase()
}

/// A literal separator prints as JS `split`.
pub fn by_comma(s: &str) -> Vec<&str> {
    s.split(", ").collect()
}

/// A separator that is not a literal, empty included, through the runtime.
pub fn by_str(s: &str, p: &str) -> (Vec<String>, Vec<String>) {
    let all: Vec<&str> = s.split(p).collect();
    let owned: Vec<String> = s.split(p).map(|t| String::from(t)).collect();
    (all.iter().map(|t| String::from(*t)).collect(), owned)
}

/// A `String` separator, mapped.
pub fn by_owned(s: &str, p: String) -> Vec<usize> {
    s.split(&p).map(|t| t.len()).collect()
}

/// In a `for`, and into consumers.
pub fn walked(s: &str, p: &str) -> (String, usize, bool, Option<usize>) {
    let mut out = String::new();
    for t in s.split(p) {
        out.push_str(t);
        out.push('|');
    }
    (out, s.split(p).count(), s.split("--").all(|t| !t.is_empty()), s.split(p).position(|t| t == "b"))
}

pub fn words(s: &str) -> Vec<&str> {
    s.split_whitespace().collect()
}

pub fn word_stats(s: &str) -> (usize, Vec<String>, usize) {
    let mut longest: usize = 0;
    for w in s.split_whitespace() {
        if w.len() > longest {
            longest = w.len();
        }
    }
    (s.split_whitespace().count(), s.split_whitespace().map(|w| w.to_uppercase()).collect(), longest)
}

pub fn lines(s: &str) -> Vec<&str> {
    s.lines().collect()
}

pub fn line_stats(s: &str) -> (usize, Vec<usize>, String) {
    let mut out = String::new();
    for l in s.lines() {
        out.push_str(l.trim());
        out.push(';');
    }
    (s.lines().count(), s.lines().map(|l| l.len()).collect(), out)
}

pub fn joined(xs: Vec<String>, sep: &str) -> (String, String, String) {
    (xs.join(sep), xs.join(""), xs.join(","))
}

pub fn joined_strs(xs: &[&str]) -> String {
    xs.join(", ")
}

pub fn joined_slice(xs: &[String], sep: String) -> String {
    xs.join(&sep)
}

/// Split, mapped, collected, joined again.
pub fn rejoined(s: &str) -> String {
    let parts: Vec<String> = s.split(',').map(|t| t.trim().to_lowercase()).collect();
    parts.join(";")
}

pub fn repeated(s: &str, n: usize) -> (String, usize) {
    let r = s.repeat(n);
    let len = r.len();
    (r, len)
}

pub fn shown(n: u32, m: i64, b: bool, c: char) -> (String, String, String, String) {
    (n.to_string(), m.to_string(), b.to_string(), c.to_string())
}

/// A `String` or `&str` is the text itself.
pub fn shown_text(s: &str, t: String) -> (String, String) {
    (s.to_string(), t.to_string())
}

pub fn shown_wide(a: i8, b: u128, c: i128, d: usize) -> String {
    let mut out = a.to_string();
    out.push_str(&b.to_string());
    out.push_str(&c.to_string());
    out.push_str(&d.to_string());
    out
}

pub fn shown_id(u: Uuid) -> String {
    u.to_string()
}
