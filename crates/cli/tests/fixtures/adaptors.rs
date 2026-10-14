// Iterator adaptors and ranges (design/01 §7.22): `filter_map`, `flat_map`,
// `flatten`, `windows`, `chunks`, `dedup`, ranges before a consumer, and
// `step_by` with `rev` in either order; and an accumulator's unsuffixed
// literal typed by its first use.

// `filter_map`: each `Some`'s value, lazily.
pub fn positives(xs: &[i32]) -> Vec<i32> {
    xs.iter().filter_map(|x| if *x > 0 { Some(*x) } else { None }).collect()
}

pub fn digits(s: &str) -> Vec<u32> {
    s.chars().filter_map(|c| c.to_digit(10)).collect()
}

pub fn parsed_total(s: &str) -> u32 {
    s.split(',').filter_map(|t| t.parse::<u32>().ok()).sum()
}

// `f` runs on an item only when `find` asks for it: an overflow after the
// item found does not happen.
pub fn first_big(xs: &[u8]) -> Option<u8> {
    xs.iter().filter_map(|x| if *x > 100 { Some(x + 100) } else { None }).find(|y| *y > 250)
}

pub fn some_total(xs: Vec<Option<u32>>) -> u32 {
    let mut n = 0u32;
    for x in xs.iter().filter_map(|o| *o) {
        n += x;
    }
    n
}

// A loop with nothing in its body still runs each stage on each item.
pub fn walked(xs: &[u8]) -> u8 {
    for _ in xs.iter().map(|x| x + 200) {}
    for _ in xs.iter().filter_map(|x| x.checked_mul(2)).map(|y| y + 100) {}
    for _ in (0..xs.len()).step_by(xs.len()) {}
    0
}

// `flatten`: a list's lists, or its `Some`s.
pub fn joined(xs: Vec<Vec<u32>>) -> Vec<u32> {
    xs.into_iter().flatten().collect()
}

pub fn some_sum(xs: &[Option<i64>]) -> i64 {
    xs.iter().flatten().sum()
}

pub fn words(lines: Vec<Vec<String>>) -> usize {
    lines.iter().flatten().filter(|w| !w.is_empty()).count()
}

pub fn longest(rows: &[Vec<String>]) -> usize {
    let mut m = 0usize;
    for w in rows.iter().flatten() {
        if w.len() > m {
            m = w.len();
        }
    }
    m
}

// `flat_map`: `f` returning a `Vec` or an `Option`.
pub fn doubled(xs: &[u32]) -> Vec<u32> {
    xs.iter().flat_map(|x| vec![*x, x * 2]).collect()
}

pub fn halves(xs: &[u32]) -> Vec<u32> {
    xs.iter().flat_map(|x| if x % 2 == 0 { Some(x / 2) } else { None }).collect()
}

pub fn spread(words: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in words.iter().flat_map(|w| vec![w.clone(), w.to_ascii_uppercase()]) {
        out.push(w);
    }
    out
}

// `windows`: each run of `n` in a row; `n` of 0 panics.
pub fn sorted(xs: &[u32]) -> bool {
    xs.windows(2).all(|w| w[0] <= w[1])
}

pub fn rises(xs: &[i32], n: usize) -> usize {
    xs.windows(n).filter(|w| w[0] < w[w.len() - 1]).count()
}

pub fn no_window(xs: &[u8]) -> usize {
    xs.windows(0).count()
}

pub fn best_pair(xs: &[u16]) -> u32 {
    let mut best = 0u32;
    for w in xs.windows(2) {
        let s = u32::from(w[0]) + u32::from(w[1]);
        if s > best {
            best = s;
        }
    }
    best
}

pub fn heads_back(xs: &[u8]) -> Vec<u8> {
    xs.windows(2).rev().map(|w| w[0]).collect()
}

pub fn pairs(words: Vec<String>) -> Vec<String> {
    words.windows(2).map(|w| format!("{}-{}", w[0], w[1])).collect()
}

// `chunks`: `n` at a time, the last maybe shorter; `n` of 0 panics.
pub fn chunk_sums(xs: &[u8], n: usize) -> Vec<u32> {
    xs.chunks(n).map(|c| c.iter().map(|b| u32::from(*b)).sum()).collect()
}

// `dedup`: each run of equal elements, as `==` compares, kept as its first.
pub fn deduped(a: Vec<u32>) -> Vec<u32> {
    let mut a = a;
    a.dedup();
    a
}

pub fn deduped_floats(a: Vec<f64>) -> Vec<f64> {
    let mut a = a;
    a.dedup();
    a
}

pub fn deduped_pairs(a: Vec<(String, Option<u8>)>) -> Vec<(String, Option<u8>)> {
    let mut v = a;
    v.dedup();
    v
}

pub fn deduped_by_len(a: Vec<String>) -> Vec<String> {
    let mut v = a;
    v.dedup_by_key(|s| s.len());
    v
}

pub fn deduped_by_overflow(a: Vec<u8>) -> Vec<u8> {
    let mut v = a;
    v.dedup_by_key(|x| *x + 100);
    v
}

// `step_by` on a range, with `rev` before it, after it, or both; `k` of 0
// panics, an empty range too. No step past the end wraps.
pub fn evens(n: u32) -> u32 {
    let mut s = 0u32;
    for i in (0..n).step_by(2) {
        s += i;
    }
    s
}

pub fn stepped(a: u8, b: u8, k: usize) -> Vec<u8> {
    (a..b).step_by(k).collect()
}

pub fn stepped_through(a: u8, b: u8, k: usize) -> Vec<u8> {
    (a..=b).step_by(k).collect()
}

pub fn down_by(a: u8, b: u8, k: usize) -> Vec<u8> {
    (a..=b).rev().step_by(k).collect()
}

pub fn by_then_down(a: u8, b: u8, k: usize) -> Vec<u8> {
    (a..=b).step_by(k).rev().collect()
}

pub fn signed_by_then_down(a: i32, b: i32, k: usize) -> Vec<i32> {
    (a..b).step_by(k).rev().collect()
}

pub fn down_by_back(a: u16, b: u16, k: usize) -> Vec<u16> {
    (a..b).rev().step_by(k).rev().collect()
}

pub fn wide_steps(a: u64, b: u64, k: usize) -> Vec<u64> {
    (a..=b).step_by(k).collect()
}

pub fn wide_down(a: i64, b: i64, k: usize) -> Vec<i64> {
    (a..b).rev().step_by(k).collect()
}

pub fn stride_sum(n: u32, k: usize) -> u64 {
    let mut s = 0u64;
    for i in (0..=n).rev().step_by(k) {
        s += u64::from(i);
    }
    s
}

pub fn odd_slots(xs: &[u8]) -> Vec<u8> {
    (1..xs.len()).step_by(2).map(|i| xs[i]).collect()
}

// A range before a stage or a consumer.
pub fn first_square_over(n: u32, limit: u32) -> Option<u32> {
    (1..=n).map(|x| x * x).find(|y| *y > limit)
}

pub fn triangle(n: u16) -> u16 {
    (1..=n).sum()
}

pub fn between(a: i8, b: i8) -> (usize, Option<i8>, Option<i8>) {
    ((a..b).count(), (a..=b).max(), (a..b).rev().min())
}

pub fn multiples(n: u32, d: u32) -> Vec<u32> {
    (0..n).filter(|x| x % d == 0).collect()
}

pub fn last_below(n: i64, limit: i64) -> Option<usize> {
    (0..=n).rev().position(|x| x < limit)
}

// An unsuffixed start whose type its first use gives.
pub fn upto(n: u32) -> u32 {
    let mut s = 0;
    for i in 0..=n {
        s += i;
    }
    s
}

pub fn grid_total(xs: Vec<Vec<u32>>) -> u32 {
    let mut s = 0;
    for r in &xs {
        for x in r {
            s += x;
        }
    }
    s
}

pub fn zeros(xs: &[u8]) -> usize {
    let mut n = 0;
    for x in xs {
        if *x == 0 {
            n += 1;
        }
    }
    n
}

pub fn lowest(xs: &[i16]) -> i16 {
    let mut lo = 0;
    for x in xs {
        if *x < lo {
            lo = *x;
        }
    }
    lo
}

fn half(x: u64) -> u64 {
    x / 2
}

pub fn halved(k: bool) -> u64 {
    let n = 18_446_744_073_709_551_615;
    if k {
        half(n)
    } else {
        3
    }
}

pub fn widest(xs: &[i32]) -> i64 {
    let mut m = -1;
    for x in xs {
        if i64::from(*x) > m {
            m = i64::from(*x);
        }
    }
    m
}
