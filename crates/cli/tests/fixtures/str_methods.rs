pub fn len_of(s: &str) -> usize {
    s.len()
}

pub fn owned_len(s: String) -> usize {
    s.len()
}

pub fn empty(s: &str) -> bool {
    s.is_empty()
}

pub fn starts(s: &str, t: &str) -> bool {
    s.starts_with(t)
}

pub fn ends(s: &str, t: &str) -> bool {
    s.ends_with(t)
}

pub fn has(s: &str, t: &str) -> bool {
    s.contains(t)
}

/// An owned needle, borrowed as Rust requires.
pub fn owned_needle(s: String, t: String) -> bool {
    s.starts_with(&t) && s.ends_with(&t)
}

/// Literal needles, and the results used in arithmetic and control flow.
pub fn classify(s: &str) -> u32 {
    let mut n = 0u32;
    if s.starts_with("pm_") {
        n += 1u32;
    }
    if s.ends_with("é") {
        n += 10u32;
    }
    if s.contains("😀") {
        n += 100u32;
    }
    if s.is_empty() {
        n += 1000u32;
    }
    if s.len() > 3usize {
        n += 10000u32;
    }
    n
}

/// `len` against a byte index: past the end panics with Rust's message.
pub fn last_byte(s: &str) -> u8 {
    s.as_bytes()[s.len() - 1usize]
}

/// Equal once ASCII letters are folded; nothing else folds (`é` / `É`).
pub fn same_ignoring_case(s: &str, t: &str) -> bool {
    s.eq_ignore_ascii_case(t)
}

/// An owned needle, and a literal one.
pub fn owned_same_ignoring_case(s: String, t: String) -> bool {
    s.eq_ignore_ascii_case(&t) || s.eq_ignore_ascii_case("PM_CARD")
}
