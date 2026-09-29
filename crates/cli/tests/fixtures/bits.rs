pub fn and_u8(a: u8, b: u8) -> u8 {
    a & b
}
pub fn or_i8(a: i8, b: i8) -> i8 {
    a | b
}
pub fn xor_i16(a: i16, b: i16) -> i16 {
    a ^ b
}
pub fn ops_u16(a: u16, b: u16) -> (u16, u16, u16) {
    (a & b, a | b, a ^ b)
}
pub fn ops_i32(a: i32, b: i32) -> (i32, i32, i32) {
    (a & b, a | b, a ^ b)
}
pub fn ops_u32(a: u32, b: u32) -> (u32, u32, u32) {
    (a & b, a | b, a ^ b)
}
pub fn ops_i64(a: i64, b: i64) -> (i64, i64, i64) {
    (a & b, a | b, a ^ b)
}
pub fn ops_u64(a: u64, b: u64) -> (u64, u64, u64) {
    (a & b, a | b, a ^ b)
}

pub fn not_i8(a: i8) -> i8 {
    !a
}
pub fn not_u8(a: u8) -> u8 {
    !a
}
pub fn not_i32(a: i32) -> i32 {
    !a
}
pub fn not_u32(a: u32) -> u32 {
    !a
}
pub fn not_i64(a: i64) -> i64 {
    !a
}
pub fn not_u64(a: u64) -> u64 {
    !a
}
/// `!` stays logical on `bool`.
pub fn not_bool(a: bool, x: u16) -> bool {
    !a && !x == 65535u16
}

/// The amount has its own type; bits shifted out are dropped.
pub fn shl_u8(a: u8, n: u32) -> u8 {
    a << n
}
pub fn shl_i8(a: i8, n: u8) -> i8 {
    a << n
}
pub fn shr_i8(a: i8, n: i64) -> i8 {
    a >> n
}
pub fn shl_u16(a: u16, n: u64) -> u16 {
    a << n
}
pub fn shl_i32(a: i32, n: i32) -> i32 {
    a << n
}
pub fn shr_i32(a: i32, n: i32) -> i32 {
    a >> n
}
pub fn shl_u32(a: u32, n: u8) -> u32 {
    a << n
}
pub fn shr_u32(a: u32, n: u8) -> u32 {
    a >> n
}
pub fn shl_i64(a: i64, n: u32) -> i64 {
    a << n
}
pub fn shr_i64(a: i64, n: u64) -> i64 {
    a >> n
}
pub fn shl_u64(a: u64, n: i8) -> u64 {
    a << n
}
pub fn shr_u64(a: u64, n: usize) -> u64 {
    a >> n
}

/// Unsuffixed amounts are `i32`; an unsuffixed left side takes the result's type.
pub fn literals(x: u64) -> u64 {
    let hi: u64 = 1 << 63;
    let mask = (1u64 << 8) - 1u64;
    (x & mask) | (hi >> 3) ^ (x >> 60)
}

/// Compound assignment in a loop: a bit count, a byte reverse, and toggles.
pub fn popcount(x: u32) -> u32 {
    let mut n = 0u32;
    let mut v = x;
    for _i in 0..32u32 {
        n += v & 1u32;
        v >>= 1;
    }
    n
}
pub fn reverse_bytes(x: u32) -> u32 {
    let mut out = 0u32;
    let mut v = x;
    for _i in 0..4u32 {
        out <<= 8;
        out |= v & 0xffu32;
        v >>= 8u8;
    }
    out
}
pub fn toggles(x: i16) -> i16 {
    let mut v = x;
    v ^= 0x5555i16;
    v &= !0xf0i16;
    v <<= 1;
    v
}

/// RFC 4226 §5.3 dynamic truncation, as the RFC writes it.
pub fn truncate(hmac: &[u8]) -> u32 {
    let offset = usize::from(hmac[19] & 0xf);
    (u32::from(hmac[offset]) & 0x7f) << 24
        | (u32::from(hmac[offset + 1]) & 0xff) << 16
        | (u32::from(hmac[offset + 2]) & 0xff) << 8
        | (u32::from(hmac[offset + 3]) & 0xff)
}

/// Permission flags.
pub fn allows(perms: u64, flag: u64) -> bool {
    perms & flag == flag
}
