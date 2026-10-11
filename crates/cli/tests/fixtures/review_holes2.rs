// Shapes an independent review of 0.13.1 found wrong, each now as Rust
// computes it: a written-out `v[i] = v[i] + x` (the read comes first), a
// lazy `split` with a closure, a block's binding that ends with the block,
// narrowing by `==` then a `match` on a copy, a variant bound to a name a
// `match` tests, a local that shadows a function used as a pattern, and a
// discriminant at the top of the fold's range.

use std::cmp::Ordering;

pub fn written_out(i: usize, x: i32) -> Vec<i32> {
    let mut v: Vec<i32> = vec![1, 2];
    v[i] = v[i] + (x + 1);
    v
}

pub fn first_piece(s: &str) -> usize {
    for t in s.split(|c: char| 100 / (u32::from(c) - 1) == 0) {
        return t.len();
    }
    0
}

pub fn any_empty(s: &str) -> bool {
    s.split(|c: char| 100 / (u32::from(c) - 1) == 0).any(|t| t.is_empty())
}

pub fn block_scoped(n: u32) -> u32 {
    let m = n;
    {
        let m = 1u32;
        if m == 0 {
            return 0;
        }
    }
    m
}

pub fn block_scoped_vec(v: Vec<i32>) -> (Vec<i32>, u32) {
    let mut w: Vec<i32> = Vec::new();
    let mut m: u32 = 5;
    {
        let mut w = v.clone();
        w.push(1);
        let mut m = m + 10;
        m += 1;
        if m == 0 {
            return (w, m);
        }
    }
    w.push(2);
    m += 2;
    (w, m)
}

pub fn narrowed_copy(o: Ordering) -> u8 {
    if o == Ordering::Less {
        let p = o;
        return match p {
            Ordering::Greater => 1,
            _ => 2,
        };
    }
    0
}

#[derive(Clone, Copy, PartialEq)]
pub enum K {
    A,
    B(u8),
}

pub fn matched_variant(x: u8) -> u8 {
    let e = K::B(x);
    match e {
        K::A => 1,
        K::B(n) => n,
    }
}

pub fn reassigned_variant(e0: K) -> u8 {
    let mut e = e0;
    if matches!(e, K::A) {
        e = K::B(6);
    }
    match e {
        K::A => 1,
        K::B(n) => n,
    }
}

fn is_x(c: char) -> bool {
    c == 'x'
}

pub fn shadowed_pattern(s: &str, is_x: char) -> Option<usize> {
    s.find(is_x)
}

pub fn calls_is_x(c: char) -> bool {
    is_x(c)
}

#[repr(i128)]
#[derive(Clone, Copy)]
pub enum Top {
    A = 170141183460469231731687303715884105727,
}

pub fn top_value() -> i128 {
    Top::A as i128
}
