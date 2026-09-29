pub enum Class {
    Digit(u8),
    Upper,
    Lower,
    At,
    Space,
    Other,
}

/// Byte-literal ranges and `|`, in return position.
pub fn class_of(b: u8) -> Class {
    match b {
        b'0'..=b'9' => Class::Digit(b - b'0'),
        b'A'..=b'Z' => Class::Upper,
        b'a'..=b'z' => Class::Lower,
        b'@' => Class::At,
        b' ' | b'\t' | b'\n' | b'\r' => Class::Space,
        _ => Class::Other,
    }
}

/// Escapes in byte literals, compared as values.
pub fn escapes(b: u8) -> u32 {
    let mut n = 0u32;
    if b == b'\'' {
        n += 1u32;
    }
    if b == b'\\' {
        n += 2u32;
    }
    if b == b'\x7f' {
        n += 4u32;
    }
    if b == b'\0' {
        n += 8u32;
    }
    n
}

/// Negative literals, half-open ranges, first match wins over an overlap.
pub fn signed(x: i32) -> i32 {
    let r: i32 = match x {
        -5..0 => 1,
        0 => 2,
        1..=9 | 100 => 3,
        5..=200 => 4,
        -2147483648..=-6 => 5,
        _ => 6,
    };
    r * 10
}

/// `bigint` scrutinees.
pub fn wide(x: i64, y: u64) -> u8 {
    let a: u8 = match x {
        0 => 1u8,
        1..=1000 => 2u8,
        -9223372036854775808 => 3u8,
        _ => 4u8,
    };
    let b: u8 = match y {
        18446744073709551615 => 10u8,
        0..10 => 20u8,
        _ => 30u8,
    };
    a + b
}

/// A match inside an arm of an enum match, and `?` in an integer arm.
pub fn digits(s: &str) -> Result<u32, usize> {
    let b = s.as_bytes();
    let mut total = 0u32;
    for i in 0..b.len() {
        let d: Result<u32, usize> = match class_of(b[i]) {
            Class::Digit(v) => Ok(u32::from(v)),
            _ => match b[i] {
                b'_' => Ok(0u32),
                _ => Err(i),
            },
        };
        total = total * 10u32 + d?;
    }
    Ok(total)
}

/// `matches!` with byte ranges, as idiomatic Rust writes a character class.
pub fn hex_value(b: u8) -> Option<u8> {
    if matches!(b, b'0'..=b'9') {
        Some(b - b'0')
    } else if matches!(b, b'a'..=b'f') {
        Some(b - b'a' + 10u8)
    } else if matches!(b, b'A'..=b'F' | b'_') {
        Some(b - b'A' + 10u8)
    } else {
        None
    }
}
