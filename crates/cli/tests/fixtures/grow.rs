// A local `let mut v: Vec<T>` that `push` grows, and `clone`: the arrays the
// caller holds are never written.

pub fn evens(n: u32) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    for i in 0..n {
        if i % 2 == 0 {
            v.push(i);
        }
    }
    v
}

/// Grows what the caller passed: a copy, so the parameter comes back as it was.
pub fn appended(xs: Vec<u32>, x: u32) -> (Vec<u32>, Vec<u32>) {
    let mut v = xs.clone();
    v.push(x);
    (xs, v)
}

fn same(xs: Vec<u32>) -> Vec<u32> {
    xs
}

/// What a function returns may be the caller's array: copied before it grows.
pub fn through(xs: Vec<u32>) -> (Vec<u32>, Vec<u32>) {
    let mut v = same(xs.clone());
    v.push(7);
    (xs, v)
}

/// Assigned again, then grown.
pub fn restarted(xs: Vec<u32>, again: bool) -> (Vec<u32>, Vec<u32>) {
    let mut v: Vec<u32> = vec![1];
    if again {
        v = xs.clone();
    }
    v.push(2);
    (xs, v)
}

/// Overflow inside the loop panics where Rust does.
pub fn running(xs: Vec<u8>) -> Vec<u8> {
    let mut total = 0u8;
    let mut v: Vec<u8> = vec![];
    for x in &xs {
        total += *x;
        v.push(total);
    }
    v
}

pub fn copied_name(s: &Option<String>) -> Option<String> {
    s.clone()
}

pub fn name_len(s: &Option<String>) -> usize {
    s.as_deref().map(|t| t.len()).unwrap_or(0)
}

/// A choice of arrays, one of them the caller's through a function: the
/// whole value is copied before it grows.
pub fn chosen(xs: Vec<u32>, fresh: bool) -> (Vec<u32>, Vec<u32>) {
    let mut v = if fresh { vec![] } else { same(xs.clone()) };
    v.push(5);
    (xs, v)
}
