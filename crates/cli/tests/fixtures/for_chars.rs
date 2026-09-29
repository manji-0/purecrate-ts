pub fn digits(s: &str) -> u32 {
    let mut n = 0u32;
    for c in s.chars() {
        if c.is_ascii_digit() {
            n += 1;
        }
    }
    n
}

pub fn first_non_ascii(s: String) -> Option<char> {
    for c in s.chars() {
        if !c.is_ascii() {
            return Some(c);
        }
    }
    None
}

pub fn code_sum(s: &String) -> u64 {
    let mut sum = 0u64;
    for c in s.chars() {
        sum += u64::from(c);
    }
    sum
}

/// Counts UTF-8 bytes through the chars: equals `s.len()`.
pub fn utf8_len(s: &str) -> usize {
    let mut n = 0usize;
    for c in s.chars() {
        n += c.len_utf8();
    }
    n
}

fn lower(c: char) -> Result<u32, char> {
    if c.is_ascii_lowercase() {
        Ok(u32::from(c))
    } else {
        Err(c)
    }
}

pub fn all_lower(s: &str) -> Result<u32, char> {
    let mut sum = 0u32;
    for c in s.chars() {
        sum += lower(c)?;
    }
    Ok(sum)
}

/// Pairs in order: `<` on `char` is code-point order, which JS strings do
/// not follow past U+E000.
pub fn ascending_pairs(s: &str) -> u32 {
    let mut n = 0u32;
    for c in s.chars() {
        for d in s.chars() {
            if c < d {
                n += 1;
            }
        }
    }
    n
}

pub fn last(s: &str) -> Option<char> {
    let mut seen: Option<char> = None;
    for c in s.chars() {
        seen = Some(c);
    }
    seen
}

pub fn shadow(s: &str) -> char {
    let c = '#';
    let mut n = 0u32;
    for c in s.chars() {
        if c == '#' {
            n += 1;
        }
    }
    if n > 0 {
        c
    } else {
        '-'
    }
}

pub fn captured(s: &str) -> u32 {
    let mut n = 0u32;
    for c in s.chars() {
        let is = |d: char| d == c;
        if is('a') {
            n += 1;
        }
    }
    n
}

fn doubled(s: &str) -> String {
    let mut t = String::from(s);
    t = String::from(&t);
    t
}

/// The string is evaluated once, before the first char.
pub fn from_call(s: &str) -> u32 {
    let mut n = 0u32;
    for c in doubled(s).chars() {
        if c.is_ascii_alphabetic() {
            n += 1;
        }
    }
    n
}

/// Overflows in the body, after `limit` chars.
pub fn overflow(s: &str, limit: u8) -> u8 {
    let mut n = 0u8;
    for c in s.chars() {
        if c.is_ascii() {
            n += limit;
        }
    }
    n
}
