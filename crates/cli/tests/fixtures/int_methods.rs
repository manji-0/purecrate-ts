// The integer methods: `min`, `max`, `abs`, `pow`, and the `checked_*`,
// `saturating_*`, and `wrapping_*` forms of the operators.

pub fn clamp_add(a: u8, b: u8, cap: u8) -> u8 {
    a.saturating_add(b).min(cap)
}

pub fn spread(a: i32, b: i32) -> i32 {
    a.max(b) - a.min(b)
}

pub fn magnitude(a: i16) -> i16 {
    a.abs()
}

pub fn power(a: i32, e: u32) -> i32 {
    a.pow(e)
}

pub fn power_u8(a: u8, e: u32) -> (Option<u8>, u8, u8) {
    (a.checked_pow(e), a.saturating_pow(e), a.wrapping_pow(e))
}

pub fn power_i8(a: i8, e: u32) -> (Option<i8>, i8, i8) {
    (a.checked_pow(e), a.saturating_pow(e), a.wrapping_pow(e))
}

pub fn checked(a: i32, b: i32) -> (Option<i32>, Option<i32>, Option<i32>) {
    (a.checked_add(b), a.checked_sub(b), a.checked_mul(b))
}

pub fn checked_div(a: i32, b: i32) -> (Option<i32>, Option<i32>, Option<i32>) {
    (a.checked_div(b), a.checked_rem(b), a.checked_neg())
}

pub fn saturating(a: i8, b: i8) -> (i8, i8, i8) {
    (a.saturating_add(b), a.saturating_sub(b), a.saturating_mul(b))
}

pub fn wrapping(a: i8, b: i8) -> (i8, i8, i8, i8) {
    (a.wrapping_add(b), a.wrapping_sub(b), a.wrapping_mul(b), a.wrapping_neg())
}

pub fn wrapping_div(a: i32, b: i32) -> (i32, i32) {
    (a.wrapping_div(b), a.wrapping_rem(b))
}

pub fn wide(a: i64, b: i64) -> (Option<i64>, i64, i64, i64) {
    (a.checked_mul(b), a.saturating_mul(b), a.wrapping_mul(b), a.max(b))
}

pub fn unsigned_wide(a: u64, b: u64) -> (Option<u64>, u64, u64) {
    (a.checked_sub(b), a.saturating_sub(b), a.wrapping_sub(b))
}

pub fn remaining(len: usize, used: usize) -> (Option<usize>, usize, Option<usize>) {
    (len.checked_sub(used), len.saturating_sub(used), len.checked_div(used))
}

pub fn unsigned_neg(a: u32) -> (Option<u32>, u32) {
    (a.checked_neg(), a.wrapping_neg())
}
