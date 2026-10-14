// 0.18.0's std methods. On `Option`: `and_then`, `filter`, `map_or` (its
// default evaluated on `Some` too), `map_or_else`, `unwrap_or_else` /
// `ok_or_else` / `or_else` (their function only on `None`), `or`,
// `is_some_and`, `is_none_or`. On `Result`: `err`, `is_ok`, `is_err`,
// `is_ok_and`, `is_err_and`, `unwrap_or`, `unwrap_or_else`, `map_or`,
// `and_then`, `or_else`. On a `Vec`: `get`, `first`, `last`, `contains`. On
// a sequence: `fold`, `rev`, `zip`.

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub x: i32,
    pub label: String,
}

fn halve(n: i32) -> Option<i32> {
    if n % 2 == 0 {
        Some(n / 2)
    } else {
        None
    }
}

fn fallback() -> i32 {
    -1
}

pub fn chain(o: Option<i32>) -> Option<i32> {
    o.and_then(halve).and_then(|n| if n > 10 { Some(n - 10) } else { None })
}

/// `d * 1000` runs, and may overflow, only on `None`.
pub fn lazy(o: Option<i32>, d: i32) -> i32 {
    o.unwrap_or_else(|| d * 1000)
}

pub fn lazy_fn(o: Option<i32>) -> i32 {
    o.unwrap_or_else(fallback)
}

pub fn keep_big(o: Option<i32>) -> Option<i32> {
    o.filter(|n| *n > 5)
}

/// `d + 1` runs on `Some` too, so it overflows there as well.
pub fn either(o: Option<i32>, d: i32) -> i32 {
    o.map_or(d + 1, |n| n * 3)
}

pub fn either_lazy(o: Option<i32>, d: i32) -> i32 {
    o.map_or_else(|| d * 2, |n| n - 1)
}

pub fn big(o: Option<i32>) -> bool {
    o.is_some_and(|n| n > 100)
}

pub fn small_or_none(o: Option<i32>) -> bool {
    o.is_none_or(|n| n < 0)
}

pub fn need(o: Option<u8>) -> Result<u8, String> {
    let v = o.ok_or_else(|| String::from("missing"))?;
    Ok(v + 1)
}

/// `or`'s argument is evaluated either way; `or_else`'s only on `None`.
pub fn other(o: Option<u8>, p: Option<u8>, n: u8) -> Option<u8> {
    o.or(p.map(|x| x + n)).or_else(|| Some(n * 2))
}

pub fn parsed(s: &str) -> Result<u32, String> {
    s.parse::<u32>()
        .map_err(|_| String::from("bad"))
        .and_then(|n| if n > 9 { Err(format!("{n} too big")) } else { Ok(n) })
}

pub fn parsed_or(s: &str) -> u32 {
    s.parse::<u32>().unwrap_or(77)
}

pub fn parsed_else(s: &str) -> u32 {
    s.parse::<u32>().unwrap_or_else(|_| s.len() as u32)
}

pub fn which(r: Result<u8, String>) -> (bool, bool, bool, bool) {
    (r.is_ok(), r.is_err(), r.clone().is_ok_and(|n| n > 3), r.is_err_and(|e| e.is_empty()))
}

pub fn err_of(r: Result<u8, String>) -> Option<String> {
    r.err()
}

/// `500 + n` runs on `Ok` too.
pub fn mapped_or(r: Result<u8, String>, n: u16) -> u16 {
    r.map_or(500 + n, |v| u16::from(v) * 2)
}

pub fn recover(r: Result<u8, String>) -> Result<u8, u8> {
    r.or_else(|e| if e.len() > 3 { Ok(0) } else { Err(e.len() as u8) })
}

pub fn at(xs: Vec<u16>, i: usize) -> Option<u16> {
    xs.get(i).copied()
}

pub fn ends(xs: Vec<String>) -> (Option<String>, Option<String>) {
    (xs.first().cloned(), xs.last().cloned())
}

pub fn second_or(xs: Vec<u8>) -> u8 {
    match xs.get(1) {
        Some(b) => *b,
        None => 0,
    }
}

pub fn last_x(ps: Vec<Point>) -> i32 {
    ps.last().map(|p| p.x).unwrap_or(-1)
}

pub fn has(xs: Vec<i64>, n: i64) -> bool {
    xs.contains(&n)
}

/// NaN is not equal to itself; `-0.0` equals `0.0`.
pub fn has_float(xs: Vec<f64>, n: f64) -> bool {
    xs.contains(&n)
}

pub fn has_point(xs: Vec<Point>, p: Point) -> bool {
    xs.contains(&p)
}

pub fn has_text(xs: Vec<String>, s: String) -> bool {
    xs.contains(&s)
}

/// The start is evaluated before any item; each step may overflow.
pub fn total(xs: Vec<u32>, start: u32) -> u32 {
    xs.iter().fold(start + 1, |acc, x| acc * 2 + x)
}

pub fn digits(s: &str) -> u64 {
    s.chars().filter(|c| c.is_ascii_digit()).fold(0u64, |n, c| n * 10 + u64::from(c.to_digit(10).unwrap_or(0)))
}

/// `map` runs from the back: the first overflow is the last item's.
pub fn backwards(xs: Vec<i32>) -> Vec<i32> {
    xs.iter().rev().map(|x| x * 3).collect()
}

pub fn back_text(s: &str) -> String {
    s.chars().rev().collect()
}

pub fn back_bytes(s: &str) -> Vec<u8> {
    s.bytes().rev().collect()
}

pub fn last_big(xs: Vec<i32>) -> Option<usize> {
    xs.iter().rev().position(|x| *x > 50)
}

pub fn dot(a: Vec<i32>, b: Vec<i32>) -> i32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

pub fn same(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).filter(|(x, y)| x == y).count()
}

pub fn walk(xs: Vec<u8>) -> u32 {
    let mut acc = 0u32;
    for x in xs.iter().rev() {
        if *x == 0 {
            break;
        }
        acc = acc * 3 + u32::from(*x);
    }
    acc
}

pub fn pairs(a: Vec<u8>, b: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (n, s) in a.iter().zip(b.iter()) {
        out.push(format!("{n}:{s}"));
    }
    out
}

pub fn indexed(xs: Vec<i32>) -> i64 {
    let mut t = 0i64;
    for (i, x) in xs.iter().rev().enumerate() {
        t += i as i64 * i64::from(*x);
    }
    t
}

pub fn zip_owned(a: Vec<u8>, b: Vec<u8>) -> Vec<u16> {
    a.into_iter().zip(b).map(|(x, y)| u16::from(x) + u16::from(y)).collect()
}
