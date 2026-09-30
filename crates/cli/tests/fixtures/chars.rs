pub enum Token {
    Digit(u32),
    Letter(char),
    Space,
    Other(char),
}

/// Literal and range patterns, `|`, and ASCII methods, in return position.
pub fn classify(c: char) -> Token {
    match c {
        '0'..='9' => Token::Digit(u32::from(c) - 48u32),
        'a'..='z' | 'A'..='Z' => Token::Letter(c.to_ascii_lowercase()),
        ' ' | '\t' | '\n' => Token::Space,
        _ => Token::Other(c),
    }
}

/// Every ASCII predicate, one bit each.
pub fn ascii_bits(c: char) -> u32 {
    let mut n = 0u32;
    if c.is_ascii() {
        n += 1u32;
    }
    if c.is_ascii_alphabetic() {
        n += 2u32;
    }
    if c.is_ascii_alphanumeric() {
        n += 4u32;
    }
    if c.is_ascii_control() {
        n += 8u32;
    }
    if c.is_ascii_digit() {
        n += 16u32;
    }
    if c.is_ascii_graphic() {
        n += 32u32;
    }
    if c.is_ascii_hexdigit() {
        n += 64u32;
    }
    if c.is_ascii_lowercase() {
        n += 128u32;
    }
    if c.is_ascii_punctuation() {
        n += 256u32;
    }
    if c.is_ascii_uppercase() {
        n += 512u32;
    }
    if c.is_ascii_whitespace() {
        n += 1024u32;
    }
    n
}

pub fn upper(c: char) -> char {
    c.to_ascii_uppercase()
}

pub fn same_letter(a: char, b: char) -> bool {
    a.eq_ignore_ascii_case(&b)
}

pub fn utf8_len(c: char) -> usize {
    c.len_utf8()
}

pub fn code(c: char) -> u32 {
    u32::from(c)
}

pub fn wide_code(c: char) -> u64 {
    u64::from(c)
}

pub fn from_byte(b: u8) -> char {
    char::from(b)
}

pub fn from_code(n: u32) -> Option<char> {
    char::from_u32(n)
}

/// Panics on a radix outside 2..=36, with Rust's message.
pub fn digit(c: char, radix: u32) -> Option<u32> {
    c.to_digit(radix)
}

pub fn is_digit_in(c: char, radix: u32) -> bool {
    c.is_digit(radix)
}

/// Ordering by code point: U+FFFF is below U+10000, unlike UTF-16 order.
pub fn order(a: char, b: char) -> i32 {
    if a < b {
        -1i32
    } else if a > b {
        1i32
    } else if a == b {
        0i32
    } else {
        2i32
    }
}

pub fn at_most(a: char, b: char) -> bool {
    a <= b && b >= a
}

/// A range over the planes, and `matches!`. The exclusive `..'\u{10ffff}'`
/// is on purpose: U+10FFFF falls through to `_`.
#[allow(non_contiguous_range_endpoints)]
pub fn plane(c: char) -> u8 {
    let p: u8 = match c {
        '\u{0}'..='\u{7f}' => 0u8,
        '\u{80}'..='\u{d7ff}' => 1u8,
        '\u{e000}'..='\u{ffff}' => 2u8,
        '\u{10000}'..'\u{10ffff}' => 3u8,
        _ => 4u8,
    };
    if matches!(c, '😀' | 'é') {
        p + 10u8
    } else {
        p
    }
}

/// Literals with escapes, compared and returned.
pub fn escapes(c: char) -> Option<char> {
    if c == '\'' {
        Some('"')
    } else if c == '\\' {
        Some('\u{10ffff}')
    } else if c == '\0' {
        Some('\u{7f}')
    } else {
        None
    }
}
