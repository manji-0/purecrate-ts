// Slicing strings at UTF-8 byte positions and `Vec`s and slices, with
// Rust's panics, and `strip_prefix` / `strip_suffix`.

pub fn mid(s: &str, a: usize, b: usize) -> &str {
    &s[a..b]
}

pub fn from(s: String, a: usize) -> String {
    String::from(&s[a..])
}

pub fn to(s: &str, b: usize) -> &str {
    &s[..b]
}

pub fn whole(s: &str) -> &str {
    &s[..]
}

// Chained: a slice of a slice, and a method on a slice.
pub fn inner(s: &str, a: usize, b: usize) -> usize {
    s[a..][..b].len()
}

pub fn digits_after(s: &str, a: usize) -> u32 {
    let mut n: u32 = 0;
    for c in s[a..].chars() {
        if c.is_ascii_digit() {
            n += 1;
        }
    }
    n
}

pub fn items(xs: &[u8], a: usize, b: usize) -> &[u8] {
    &xs[a..b]
}

pub fn items_from(xs: Vec<u32>, a: usize) -> u32 {
    let mut total: u32 = 0;
    for x in &xs[a..] {
        total += x;
    }
    total
}

pub fn items_to(xs: &[u8], b: usize) -> usize {
    xs[..b].len()
}

pub fn bytes_of(s: &str, a: usize, b: usize) -> usize {
    s.as_bytes()[a..b].len()
}

pub fn strip(s: &str) -> Option<&str> {
    s.strip_prefix("pm_")
}

pub fn strip_both(s: &str, p: &str, q: &str) -> Option<usize> {
    let rest = s.strip_prefix(p)?;
    let core = rest.strip_suffix(q)?;
    Some(core.len())
}
