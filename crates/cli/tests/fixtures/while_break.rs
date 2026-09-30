// `while`, `break`, and `continue`: a `match` (a `switch` in TS) around a
// jump, nested loops where `break` leaves only the inner one, `?` in a
// `while` condition, early `return`, and overflow inside the loop.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token {
    Digit(u8),
    Skip,
    Stop,
}

/// Collatz steps; `3n + 1` overflows `u32` for large inputs.
pub fn steps(n: u32) -> u32 {
    let mut n = n;
    let mut count = 0u32;
    while n > 1 {
        if n % 2 == 0 {
            n = n / 2;
        } else {
            n = 3 * n + 1;
        }
        count += 1;
    }
    count
}

/// `break` and `continue` inside `match` arms leave or skip the loop, not
/// the `switch`.
pub fn read(tokens: Vec<Token>) -> u32 {
    let mut value = 0u32;
    for t in &tokens {
        match t {
            Token::Stop => break,
            Token::Skip => continue,
            Token::Digit(d) => value = value * 10 + u32::from(*d),
        }
        value += 1000;
    }
    value
}

/// The inner `break` leaves the inner loop only.
pub fn rows(n: u32, width: u32) -> u32 {
    let mut total = 0u32;
    for i in 0..n {
        let mut j = 0u32;
        while j < 100 {
            if j == width {
                break;
            }
            if (i + j) % 3 == 0 {
                j += 1;
                continue;
            }
            total += 1;
            j += 1;
        }
    }
    total
}

fn next(budget: u32, i: u32) -> Result<bool, u32> {
    if i > budget {
        return Err(i);
    }
    Ok(i < 5)
}

/// `?` in the condition runs before every pass.
pub fn guarded(budget: u32) -> Result<u32, u32> {
    let mut i = 0u32;
    while next(budget, i)? {
        i += 1;
    }
    Ok(i)
}

/// Early `return` from inside `while`, with `continue` in a `match`.
pub fn first_even_square(xs: Vec<i32>) -> Option<i32> {
    let mut k = 0usize;
    while k < xs.len() {
        let x = xs[k];
        k += 1;
        match x % 2 {
            0 => {}
            _ => continue,
        }
        return Some(x * x);
    }
    None
}

/// `break` in a `for` over chars, inside an `if` inside a `match`.
pub fn prefix_len(s: &str) -> u32 {
    let mut n = 0u32;
    for c in s.chars() {
        match c {
            'a'..='z' => {
                if n == 3 {
                    break;
                }
                n += 1;
            }
            _ => break,
        }
    }
    n
}
