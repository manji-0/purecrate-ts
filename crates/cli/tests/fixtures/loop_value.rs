// `loop` with `break` and a value, `loop` left only by `return`, `while let`
// (its scrutinee evaluated before every pass), and `for` over `a..=b` and
// `(a..b).rev()` at the ends of their types.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Add(u8),
    Skip,
    Stop,
}

/// The first square at or past `n`; `i * i` overflows `u32` past 65535.
pub fn first_square(n: u32) -> u32 {
    let mut i = 0u32;
    let found = loop {
        if i * i >= n {
            break i;
        }
        i += 1;
    };
    found + 1
}

/// Left only by `return`: the loop has type `!`.
pub fn collatz(n: u64) -> u32 {
    let mut n = n;
    let mut steps = 0u32;
    loop {
        if n <= 1 {
            return steps;
        }
        n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
        steps += 1;
    }
}

/// `break` with a value from the arms of a `match` (a `switch` in TS), a
/// `continue` among them, and a `loop` that is the function's value.
pub fn run_steps(steps: Vec<Step>) -> u32 {
    let mut i = 0usize;
    let mut total = 0u32;
    loop {
        if i >= steps.len() {
            break total * 2;
        }
        let s = steps[i];
        i += 1;
        match s {
            Step::Add(n) => total += u32::from(n),
            Step::Skip => continue,
            Step::Stop => break total,
        }
    }
}

/// A `loop` whose value is a tuple, with an inner `loop` whose bare
/// `break` leaves only it.
pub fn grid(w: u8, h: u8) -> (u8, u32) {
    let mut y = 0u8;
    let mut cells = 0u32;
    let out = loop {
        if y == h {
            break (y, cells);
        }
        let mut x = 0u8;
        loop {
            if x == w {
                break;
            }
            x += 1;
            cells += 1;
        }
        if cells > 1000 {
            break (0, cells);
        }
        y += 1;
    };
    out
}

/// A `loop` whose value is dropped: a statement.
pub fn count_down(n: u8) -> u8 {
    let mut n = n;
    let mut seen = 0u8;
    loop {
        if n == 0 {
            break;
        }
        n -= 1;
        seen += 1;
    }
    seen
}

/// `while let` on a method whose value changes each pass.
pub fn strip_all(s: &str, p: &str) -> usize {
    let mut t: &str = s;
    let mut n = 0usize;
    if p.is_empty() {
        return 0;
    }
    while let Some(rest) = t.strip_prefix(p) {
        t = rest;
        n += 1;
    }
    n * 100 + t.len()
}

fn at(steps: &[Step], i: usize) -> Option<Step> {
    if i < steps.len() {
        Some(steps[i])
    } else {
        None
    }
}

/// `while let` on a variant inside `Some`, with a `break` inside.
pub fn take_adds(steps: Vec<Step>) -> u32 {
    let mut i = 0usize;
    let mut total = 0u32;
    while let Some(Step::Add(n)) = at(&steps, i) {
        if n == 0 {
            break;
        }
        total += u32::from(n);
        i += 1;
    }
    total * 1000 + i as u32
}

fn head(s: &str) -> &str {
    match s.split_once(',') {
        Some((a, _)) => a,
        None => s,
    }
}

/// `while let` on a `Result`, ending at the first error.
pub fn sum_numbers(s: &str) -> u64 {
    let mut rest: &str = s;
    let mut sum = 0u64;
    while let Ok(n) = head(rest).parse::<u32>() {
        sum += u64::from(n);
        match rest.split_once(',') {
            Some((_, b)) => rest = b,
            None => break,
        }
    }
    sum
}

/// `a..=b` up to the type's maximum, and an empty one (`a > b`).
pub fn sum_to(a: u8, b: u8) -> u32 {
    let mut s = 0u32;
    for i in a..=b {
        s += u32::from(i);
    }
    s
}

pub fn count_to_i8(a: i8, b: i8) -> i32 {
    let mut n = 0i32;
    for i in a..=b {
        n = n * 3 % 1_000_003 + i32::from(i);
    }
    n
}

pub fn last_of_u64(a: u64, b: u64) -> u64 {
    let mut last = 0u64;
    let mut n = 0u64;
    for i in a..=b {
        last = i;
        n += 1;
        if n == 5 {
            break;
        }
    }
    last ^ n
}

/// `(a..b).rev()` down to an unsigned start of 0, with `continue`.
pub fn digits_down(n: u8) -> u64 {
    let mut acc = 0u64;
    for i in (0..n).rev() {
        if i % 4 == 1 {
            continue;
        }
        acc = acc % 1_000_000_007 * 10 + u64::from(i % 10);
    }
    acc
}

/// `(a..=b).rev()` from the type's maximum and down to its minimum.
pub fn down_incl(a: i8, b: i8) -> i64 {
    let mut acc = 0i64;
    for i in (a..=b).rev() {
        acc = acc * 7 % 1_000_000_007 + i64::from(i) + 200;
    }
    acc
}

pub fn down_u64(a: u64, b: u64) -> u64 {
    let mut acc = 0u64;
    let mut n = 0u32;
    for i in (a..b).rev() {
        acc ^= i;
        n += 1;
        if n == 4 {
            break;
        }
    }
    acc
}

/// The bounds run once, start first, in both directions: a start that
/// panics wins over an end that does.
pub fn bounds_order(a: u8, b: u8) -> u32 {
    let mut s = 0u32;
    for i in (a + 1..b * 2).rev() {
        s += u32::from(i);
    }
    for i in a * 2..=b + 1 {
        s += u32::from(i);
    }
    s
}
