pub fn sum(a: i32, b: i32) -> i32 {
    let mut s = 0i32;
    for i in a..b {
        s += i;
    }
    s
}

pub fn sum_big(a: i64, b: i64) -> i64 {
    let mut s = 0i64;
    for i in a..b {
        s += i * i;
    }
    s
}

pub fn first_zero(xs: Vec<u8>) -> Option<usize> {
    for i in 0..xs.len() {
        if xs[i] == 0u8 {
            return Some(i);
        }
    }
    None
}

fn checked(x: i32) -> Result<i32, i32> {
    if x < 0i32 {
        Err(x)
    } else {
        Ok(x)
    }
}

pub fn all_checked(a: i32, b: i32) -> Result<i32, i32> {
    let mut s = 0i32;
    for i in a..b {
        s += checked(i)?;
    }
    Ok(s)
}

pub fn table(n: u32) -> u32 {
    let mut s = 0u32;
    for i in 0..n {
        for j in i..n {
            s += i * j;
        }
    }
    s
}

pub fn captured(n: i32) -> i32 {
    let mut s = 0i32;
    for i in 0..n {
        let twice = |k: i32| k + i;
        s += twice(i);
    }
    s
}

/// The end is read once: growing `n` in the body does not extend the loop.
pub fn bound_once(n: u32) -> u32 {
    let mut m = n;
    let mut count = 0u32;
    for i in 0..m {
        m += 1u32;
        count += i;
    }
    count + m
}

pub fn shadow(n: u32) -> u32 {
    let i = 100u32;
    let mut s = 0u32;
    for i in 0..n {
        s += i;
    }
    s + i
}

/// The bounds panic before the loop, the body's overflow inside it.
pub fn bounds_panic(a: i32, b: i32) -> i32 {
    let mut s = 0i32;
    for i in 0i32..(a / b) {
        s += i * 1000000i32;
    }
    s
}

/// A flag the body sets, read by the test at the loop's head: not decided
/// by its value before the loop.
pub fn steps_until_past(limit: u32) -> u32 {
    let mut done = false;
    let mut steps: u32 = 0;
    while !done {
        steps += 1;
        done = steps > limit;
    }
    steps
}

/// The same with the flag set to go on.
pub fn steps_while_going(limit: u32) -> u32 {
    let mut going = true;
    let mut steps: u32 = 0;
    while going {
        steps += 1;
        going = steps <= limit;
    }
    steps
}
