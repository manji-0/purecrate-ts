// Tuple patterns where Rust takes an irrefutable one: `let`, closure
// parameters, and `for` variables.

fn split(x: u32) -> (u32, i64) {
    (x / 2, i64::from(x) - 1)
}

pub fn swap(x: u32) -> (i64, u32) {
    let (half, less) = split(x);
    (less, half)
}

// A `mut` element, an annotation, and `_`.
pub fn bump(x: u32) -> u32 {
    let (mut half, _): (u32, i64) = split(x);
    half += 1;
    half
}

// Elements are evaluated left to right; the first overflow panics.
pub fn both(a: u8, b: u8) -> u8 {
    let (x, y) = (a + 1, b + 1);
    x * y
}

// Only the first element used.
pub fn first(x: u32) -> u32 {
    let (half, _less) = split(x);
    half
}

pub fn sum_pairs(xs: &[(u32, u32)]) -> u32 {
    let mut total: u32 = 0;
    for (k, v) in xs {
        if *k == 0 {
            break;
        }
        if *v == 0 {
            continue;
        }
        total += k * v;
    }
    total
}

pub fn sum_refs(xs: Vec<(u8, u8)>) -> u32 {
    let mut total: u32 = 0;
    for &(a, b) in xs.iter() {
        total += u32::from(a) * u32::from(b);
    }
    total
}

pub fn add(o: Option<(u32, u32)>) -> Option<u32> {
    o.map(|(a, b)| a + b)
}

pub fn scaled(x: u32, k: u32) -> u32 {
    let apply = |(a, b): (u32, u32), c: u32| a * c + b;
    apply(split_u(x), k)
}

fn split_u(x: u32) -> (u32, u32) {
    (x / 3, x % 3)
}
