// `for` over a `Vec` or slice (`xs`, `&xs`, `xs.iter()`, `xs.into_iter()`)
// and over a string's `bytes()`: sums that overflow, early `return` and `?`
// in the body, nesting, fields and `Option` elements, a `match` in the
// body.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub qty: u32,
    pub price: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Digit,
    Letter,
    Other,
}

pub fn sum(xs: Vec<i32>) -> i32 {
    let mut total = 0i32;
    for x in &xs {
        total += x;
    }
    total
}

pub fn sum_iter(xs: Vec<i32>) -> i32 {
    let mut total = 0i32;
    for x in xs.iter() {
        total = total + *x;
    }
    total
}

pub fn sum_owned(xs: Vec<i32>) -> i32 {
    let mut total = 0i32;
    for x in xs {
        total += x;
    }
    total
}

pub fn sum_slice(xs: &[u8]) -> u32 {
    let mut total = 0u32;
    for b in xs {
        total += u32::from(*b);
    }
    total
}

pub fn digits(s: &str) -> u32 {
    let mut n = 0u32;
    for b in s.bytes() {
        if matches!(b, b'0'..=b'9') {
            n += 1;
        }
    }
    n
}

pub fn high_bytes(s: String) -> u32 {
    let mut n = 0u32;
    for b in s.as_bytes() {
        if *b >= 0x80 {
            n += 1;
        }
    }
    n
}

/// The first line whose total overflows `i64`, or the grand total.
pub fn total(lines: Vec<Line>) -> Result<i64, u32> {
    let mut sum = 0i64;
    let mut index = 0u32;
    for line in &lines {
        let amount = checked(line)?;
        sum += amount;
        index += 1;
    }
    if index == 0 {
        return Err(0);
    }
    Ok(sum)
}

fn checked(line: &Line) -> Result<i64, u32> {
    if line.qty > 1000 {
        return Err(line.qty);
    }
    Ok(i64::from(line.qty) * line.price)
}

/// Early `return` from inside a nested loop.
pub fn first_pair(xs: Vec<i32>, target: i32) -> Option<u32> {
    let mut i = 0u32;
    for a in &xs {
        let mut j = 0u32;
        for b in &xs {
            if i != j && *a + *b == target {
                return Some(i * 100 + j);
            }
            j += 1;
        }
        i += 1;
    }
    None
}

pub fn count_some(xs: Vec<Option<u8>>) -> u8 {
    let mut n = 0u8;
    for x in &xs {
        match x {
            Some(_) => n += 1,
            None => {}
        }
    }
    n
}

pub fn classify(s: &str) -> Vec<u32> {
    let mut digits = 0u32;
    let mut letters = 0u32;
    let mut others = 0u32;
    for c in s.chars() {
        let kind = if c.is_ascii_digit() {
            Kind::Digit
        } else if c.is_ascii_alphabetic() {
            Kind::Letter
        } else {
            Kind::Other
        };
        match kind {
            Kind::Digit => digits += 1,
            Kind::Letter => letters += 1,
            Kind::Other => others += 1,
        }
    }
    vec![digits, letters, others]
}
