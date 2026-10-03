// `map` and `filter` over a sequence, lazy as in Rust: each item runs every
// stage, then the consumer, before the next item, so the first panic is the
// same one on both sides.

/// The doubling panics on a large item; the sum may overflow before it.
pub fn doubled_sum(xs: Vec<u32>) -> u32 {
    xs.iter().map(|x| x * 2).sum()
}

pub fn doubled(xs: Vec<u32>) -> Vec<u32> {
    xs.iter().map(|x| x * 2).collect()
}

/// A division by an item that is zero, only for the items the filter keeps.
pub fn quotients(xs: Vec<u32>, n: u32) -> Vec<u32> {
    xs.iter().filter(|x| **x != 1).map(|x| n / x).collect()
}

pub fn kept(xs: Vec<u32>) -> Vec<u32> {
    xs.iter().copied().filter(|x| *x % 2 == 0).collect()
}

pub fn copy(xs: Vec<u32>) -> Vec<u32> {
    xs.iter().copied().collect()
}

pub fn digits(s: &str) -> usize {
    s.chars().filter(|c| c.is_ascii_digit()).count()
}

pub fn letters(s: &str) -> Vec<char> {
    s.chars().filter(|c| c.is_ascii_alphabetic()).collect()
}

pub fn byte_sum(s: &str) -> u8 {
    s.bytes().map(|b| b - b'0').sum()
}

pub fn first_big(xs: Vec<u32>) -> Option<usize> {
    xs.iter().map(|x| x * 3).position(|y| y > 10)
}

fn checked(x: u32) -> Result<u32, u32> {
    if x > 5 {
        Err(x)
    } else {
        Ok(x * 10)
    }
}

/// Into a `Result`: stops at the first `Err` among the items kept.
pub fn all_small(xs: Vec<u32>) -> Result<Vec<u32>, u32> {
    xs.iter().copied().filter(|x| *x != 0).map(checked).collect()
}

pub fn words(s: &str) -> Vec<usize> {
    s.split(' ').filter(|w| !w.is_empty()).map(|w| w.len()).collect()
}
