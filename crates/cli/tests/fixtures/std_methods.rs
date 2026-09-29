pub struct Cart {
    pub items: Vec<u32>,
    pub coupon: Option<String>,
}

pub fn empty(xs: Vec<i32>) -> bool {
    xs.is_empty()
}

/// On a slice, and on the bytes of a string.
pub fn blank(xs: &[i32], s: &str) -> bool {
    xs.is_empty() || s.as_bytes().is_empty()
}

pub fn some(x: Option<i32>) -> bool {
    x.is_some()
}

pub fn none(x: Option<String>) -> bool {
    x.is_none()
}

/// Through a borrow and a field, negated, as a condition.
pub fn ready(cart: &Cart, note: &Option<char>) -> u8 {
    let mut score = 0u8;
    if !cart.items.is_empty() {
        score += 1u8;
    }
    if cart.coupon.is_some() {
        score += 2u8;
    }
    if !note.is_none() {
        score += 4u8;
    }
    score
}

/// `Option<u64>` is `bigint | null`; `0` is still `Some`.
pub fn zero_is_some(x: Option<u64>, y: Option<bool>) -> bool {
    x.is_some() && y.is_some()
}
