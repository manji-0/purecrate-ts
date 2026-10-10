// `x as T` between integer types where std has no `From`: the value
// modulo 2^bits of `T`, as Rust's `as` wraps; to `usize`, a value above
// 2^53−1 panics, as every `usize` that large does (design/01 §3). One
// function per pair; `u8`, `u16`, and `u32` to `usize` are the widenings
// `as` already took.

pub fn i8_as_u8(x: i8) -> u8 {
    x as u8
}

pub fn i8_as_u16(x: i8) -> u16 {
    x as u16
}

pub fn i8_as_u32(x: i8) -> u32 {
    x as u32
}

pub fn i8_as_u64(x: i8) -> u64 {
    x as u64
}

pub fn i8_as_usize(x: i8) -> usize {
    x as usize
}

pub fn i16_as_i8(x: i16) -> i8 {
    x as i8
}

pub fn i16_as_u8(x: i16) -> u8 {
    x as u8
}

pub fn i16_as_u16(x: i16) -> u16 {
    x as u16
}

pub fn i16_as_u32(x: i16) -> u32 {
    x as u32
}

pub fn i16_as_u64(x: i16) -> u64 {
    x as u64
}

pub fn i16_as_usize(x: i16) -> usize {
    x as usize
}

pub fn i32_as_i8(x: i32) -> i8 {
    x as i8
}

pub fn i32_as_i16(x: i32) -> i16 {
    x as i16
}

pub fn i32_as_u8(x: i32) -> u8 {
    x as u8
}

pub fn i32_as_u16(x: i32) -> u16 {
    x as u16
}

pub fn i32_as_u32(x: i32) -> u32 {
    x as u32
}

pub fn i32_as_u64(x: i32) -> u64 {
    x as u64
}

pub fn i32_as_usize(x: i32) -> usize {
    x as usize
}

pub fn i64_as_i8(x: i64) -> i8 {
    x as i8
}

pub fn i64_as_i16(x: i64) -> i16 {
    x as i16
}

pub fn i64_as_i32(x: i64) -> i32 {
    x as i32
}

pub fn i64_as_u8(x: i64) -> u8 {
    x as u8
}

pub fn i64_as_u16(x: i64) -> u16 {
    x as u16
}

pub fn i64_as_u32(x: i64) -> u32 {
    x as u32
}

pub fn i64_as_u64(x: i64) -> u64 {
    x as u64
}

pub fn i64_as_usize(x: i64) -> usize {
    x as usize
}

pub fn u8_as_i8(x: u8) -> i8 {
    x as i8
}

pub fn u8_as_usize(x: u8) -> usize {
    x as usize
}

pub fn u16_as_i8(x: u16) -> i8 {
    x as i8
}

pub fn u16_as_i16(x: u16) -> i16 {
    x as i16
}

pub fn u16_as_u8(x: u16) -> u8 {
    x as u8
}

pub fn u16_as_usize(x: u16) -> usize {
    x as usize
}

pub fn u32_as_i8(x: u32) -> i8 {
    x as i8
}

pub fn u32_as_i16(x: u32) -> i16 {
    x as i16
}

pub fn u32_as_i32(x: u32) -> i32 {
    x as i32
}

pub fn u32_as_u8(x: u32) -> u8 {
    x as u8
}

pub fn u32_as_u16(x: u32) -> u16 {
    x as u16
}

pub fn u32_as_usize(x: u32) -> usize {
    x as usize
}

pub fn u64_as_i8(x: u64) -> i8 {
    x as i8
}

pub fn u64_as_i16(x: u64) -> i16 {
    x as i16
}

pub fn u64_as_i32(x: u64) -> i32 {
    x as i32
}

pub fn u64_as_i64(x: u64) -> i64 {
    x as i64
}

pub fn u64_as_u8(x: u64) -> u8 {
    x as u8
}

pub fn u64_as_u16(x: u64) -> u16 {
    x as u16
}

pub fn u64_as_u32(x: u64) -> u32 {
    x as u32
}

pub fn u64_as_usize(x: u64) -> usize {
    x as usize
}

pub fn usize_as_i8(x: usize) -> i8 {
    x as i8
}

pub fn usize_as_i16(x: usize) -> i16 {
    x as i16
}

pub fn usize_as_i32(x: usize) -> i32 {
    x as i32
}

pub fn usize_as_i64(x: usize) -> i64 {
    x as i64
}

pub fn usize_as_u8(x: usize) -> u8 {
    x as u8
}

pub fn usize_as_u16(x: usize) -> u16 {
    x as u16
}

pub fn usize_as_u32(x: usize) -> u32 {
    x as u32
}

pub fn usize_as_u64(x: usize) -> u64 {
    x as u64
}

/// A 5-bit value indexing a table, as base32 does.
pub fn digit(v: u64, shift: u32) -> char {
    let alphabet: Vec<char> = "0123456789ABCDEFGHJKMNPQRSTVWXYZ".chars().collect();
    alphabet[((v >> shift) & 31) as usize]
}

/// The bytes of `v`, most significant first.
pub fn bytes(v: u64) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for k in 0..8u32 {
        out.push((v >> (56 - 8 * k)) as u8);
    }
    out
}
