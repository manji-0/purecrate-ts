// Building a `String`: `String::new`, `push` and `push_str` on a local `let
// mut`, and `collect::<String>()` over `char`s.

pub fn shouted(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        out.push(c.to_ascii_uppercase());
    }
    out.push_str("!");
    out
}

/// The pieces joined with a separator, as `join` would.
pub fn joined(pieces: Vec<String>, sep: char) -> String {
    let mut out = String::new();
    for (i, p) in pieces.iter().enumerate() {
        if i > 0 {
            out.push(sep);
        }
        out.push_str(p);
    }
    out
}

pub fn from_chars(cs: Vec<char>) -> String {
    cs.iter().collect::<String>()
}

/// Every other char, through a stage.
pub fn every_other(s: &str) -> String {
    s.chars().filter(|c| u32::from(*c) % 2 == 0).collect()
}

/// A prefix, then a piece chosen by a test.
pub fn prefixed(p: &str, keep: bool) -> String {
    let mut s = String::from("xn--");
    s.push_str(if keep { p } else { "" });
    s
}

/// What was built is read like any `String`.
pub fn built_len(s: &str, n: u32) -> usize {
    let mut out = String::new();
    for _i in 0..n {
        out.push_str(s);
        out.push('é');
    }
    out.len()
}
