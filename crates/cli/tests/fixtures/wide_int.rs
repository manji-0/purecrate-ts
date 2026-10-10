// `u128` and `i128` as `bigint`s, as `u64` and `i64` are: operators that
// panic on overflow, the bitwise operators and shifts, the integer methods,
// `from`, `as`, and `parse`; and byte strings, as a `&[u8]` const and in a
// function.

const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const RAW: &[u8] = b"\xff\x00a";

pub fn add(a: u128, b: u128) -> u128 {
    a + b
}

pub fn sub(a: i128, b: i128) -> i128 {
    a - b
}

pub fn mul(a: u128, b: u128) -> u128 {
    a * b
}

pub fn div_rem(a: i128, b: i128) -> (i128, i128) {
    (a / b, a % b)
}

pub fn bitwise(a: u128, b: u128, n: u32) -> (u128, u128, u128, u128) {
    (a & b, a | b ^ !b, a << n, a >> n)
}

pub fn signed_shift(a: i128, n: u32) -> (i128, i128) {
    (a << n, a >> n)
}

pub fn methods(a: u128, b: u128) -> (Option<u128>, u128, u128, u128) {
    (a.checked_add(b), a.saturating_sub(b), a.wrapping_mul(b), a.min(b))
}

pub fn signed_methods(a: i128) -> (Option<i128>, i128, i128) {
    (a.checked_neg(), a.wrapping_neg(), a.saturating_mul(a))
}

pub fn joined(hi: u64, lo: u64) -> u128 {
    (u128::from(hi) << 64) | u128::from(lo)
}

pub fn halves(v: u128) -> (u64, u64) {
    ((v >> 64) as u64, v as u64)
}

pub fn signed(v: u128) -> (i128, i64, i8) {
    (v as i128, v as i64, v as i8)
}

pub fn widened(a: i64, b: u8) -> (i128, u128, i128) {
    (i128::from(a), u128::from(b), i128::from(b))
}

pub fn parsed(s: &str) -> (Option<u128>, Option<i128>) {
    (s.parse::<u128>().ok(), s.parse::<i128>().ok())
}

/// The 26 base32 digits of a 128-bit value, five bits at a time from the
/// top (the first digit takes three), as ULID writes one.
pub fn encoded(v: u128) -> String {
    let mut out = String::new();
    for k in 0..26u32 {
        let shift = 125 - 5 * k;
        out.push(char::from(ALPHABET[((v >> shift) & 31) as usize]));
    }
    out
}

pub fn raw(i: usize) -> u8 {
    RAW[i]
}

pub fn bytes_len() -> usize {
    b"h\xc3\xa9llo".len()
}
