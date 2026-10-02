// `str::parse` into every integer type, read back through `ok()`, `match`,
// `map_err(..)`, and `map_err(..)?`: a leading `+`, a `-` only where the
// type is signed, ASCII digits only, the type's range, and leading zeros.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bad {
    NotANumber,
    TooBig(u8),
}

pub fn as_i8(s: &str) -> Option<i8> {
    s.parse::<i8>().ok()
}

pub fn as_i16(s: &str) -> Option<i16> {
    s.parse::<i16>().ok()
}

pub fn as_i32(s: &str) -> Option<i32> {
    s.parse::<i32>().ok()
}

pub fn as_i64(s: &str) -> Option<i64> {
    s.parse::<i64>().ok()
}

pub fn as_u8(s: &str) -> Option<u8> {
    s.parse::<u8>().ok()
}

pub fn as_u16(s: &str) -> Option<u16> {
    s.parse::<u16>().ok()
}

pub fn as_u32(s: &str) -> Option<u32> {
    s.parse::<u32>().ok()
}

pub fn as_u64(s: &str) -> Option<u64> {
    s.parse::<u64>().ok()
}

pub fn as_usize(s: &str) -> Option<usize> {
    s.parse::<usize>().ok()
}

/// A `match` on the `Result`, with the error named and unused.
pub fn or_zero(s: &str) -> u32 {
    match s.parse::<u32>() {
        Ok(n) => n,
        Err(_) => 0,
    }
}

/// `map_err` with a variant, then `?`; the parsed value is used after.
pub fn doubled(s: &str) -> Result<u16, Bad> {
    let n = s.parse::<u16>().map_err(|_| Bad::NotANumber)?;
    n.checked_mul(2).ok_or(Bad::TooBig(2))
}

/// `map_err` kept as a value, into a typed `let`.
pub fn kept(s: &str) -> Result<i64, Bad> {
    let r: Result<i64, Bad> = s.parse::<i64>().map_err(|_| Bad::TooBig(0));
    r
}

fn not_a_number(_e: std::num::ParseIntError) -> Bad {
    Bad::NotANumber
}

/// `map_err` with a function name, returned as it is.
pub fn named(s: &str) -> Result<u8, Bad> {
    s.parse::<u8>().map_err(not_a_number)
}
