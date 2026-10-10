// `str` methods for parsing headers and tokens: ASCII case folding, `trim`
// by Rust's whitespace, `trim_matches` and its one-sided forms with a
// `char`, a `&str`, or a closure, `find` as a UTF-8 byte offset, and
// `split` with a closure.

pub fn folded(s: &str) -> (String, String) {
    (s.to_ascii_lowercase(), s.to_ascii_uppercase())
}

pub fn trimmed(s: &str) -> (String, String, String) {
    (String::from(s.trim()), String::from(s.trim_start()), String::from(s.trim_end()))
}

pub fn trimmed_by(s: &str, c: char) -> (String, String, String) {
    (String::from(s.trim_matches(c)), String::from(s.trim_start_matches(c)), String::from(s.trim_end_matches(c)))
}

pub fn trimmed_ows(s: &str) -> String {
    String::from(s.trim_matches(|c: char| c == ' ' || c == '\t'))
}

pub fn trimmed_str(s: &str, p: &str) -> (String, String) {
    (String::from(s.trim_start_matches(p)), String::from(s.trim_end_matches(p)))
}

pub fn found(s: &str, c: char, p: &str) -> (Option<usize>, Option<usize>, Option<usize>) {
    (s.find(c), s.find(p), s.find(|x: char| !x.is_ascii()))
}

/// The pieces between `,` and `;`, each trimmed, empty ones kept.
pub fn pieces(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in s.split(|c: char| c == ',' || c == ';') {
        out.push(String::from(t.trim()));
    }
    out
}

pub fn collected(s: &str) -> Vec<&str> {
    s.split(|c: char| c.is_ascii_digit()).collect()
}
