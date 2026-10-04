pub fn or_zero(x: Option<i32>) -> i32 {
    match x {
        Some(v) => v,
        None => 0,
    }
}

pub fn none_first(x: Option<i32>) -> i32 {
    match x {
        None => -1,
        Some(_) => 1,
    }
}

pub fn doubled(x: Option<i32>) -> Option<i32> {
    if let Some(v) = x {
        Some(v * 2)
    } else {
        None
    }
}

pub fn is_none(x: Option<i32>) -> bool {
    if let None = x {
        true
    } else {
        false
    }
}

pub fn safe_div(a: i32, b: i32) -> Result<i32, i32> {
    if b == 0 {
        Err(a)
    } else {
        Ok(a / b)
    }
}

pub fn div_or(a: i32, b: i32, d: i32) -> i32 {
    match safe_div(a, b) {
        Ok(q) => q,
        Err(_) => d,
    }
}

pub fn failed_numerator(a: i32, b: i32) -> i32 {
    if let Err(e) = safe_div(a, b) {
        e
    } else {
        0
    }
}

pub fn sum_both(x: Option<i32>, y: Option<i32>) -> i32 {
    match x {
        Some(a) => match y {
            Some(b) => a + b,
            None => a,
        },
        None => 0,
    }
}

pub fn bump(x: Option<i32>) -> i32 {
    let base = or_zero(x);
    base + 1
}

pub fn first_some(x: Option<i32>, y: Option<i32>) -> Option<i32> {
    match x {
        Some(a) => Some(a),
        None => y,
    }
}

pub fn half_even(n: i32) -> Result<i32, i32> {
    if n % 2 == 0 {
        Ok(n / 2)
    } else {
        Err(n)
    }
}

pub fn quarter(n: i32) -> Result<i32, i32> {
    let h = half_even(n)?;
    half_even(h)
}

pub fn eighth_plus(n: i32) -> Result<i32, i32> {
    Ok(half_even(half_even(half_even(n)?)?)? + 1)
}

pub fn both_halves(a: i32, b: i32) -> Result<i32, i32> {
    Ok(half_even(a)? + half_even(b)?)
}

pub fn opt_add(a: Option<i32>, b: Option<i32>) -> Option<i32> {
    Some(a? + b?)
}

pub fn first_or_bail(x: Option<i32>) -> i32 {
    let v = match x {
        Some(v) => v,
        None => return -1,
    };
    v * 2
}

pub fn clamp_negative(n: i32) -> i32 {
    if n < 0 {
        return 0
    } else {
        n
    }
}

pub fn chosen(flag: bool, x: Option<i32>) -> Result<i32, i32> {
    let base = if flag {
        match x {
            Some(v) => v,
            None => return Err(0),
        }
    } else {
        10
    };
    Ok(base + 1)
}

pub fn guarded(n: i32) -> Result<i32, i32> {
    let q = if n > 100 { half_even(n)? } else { n };
    Ok(q)
}

pub fn blocky(n: i32) -> i32 {
    let x = {
        let y = n + 1;
        y * 2
    };
    x + 1
}

pub fn shadowed(n: i32) -> i32 {
    let n = n + 1;
    let n = match Some(n) {
        Some(n) => n * 2,
        None => 0,
    };
    n + 1
}

pub fn add_twice(start: u8, n: u8) -> u8 {
    let mut x = start;
    x += n;
    x += n;
    x
}

pub fn count_positive(a: i32, b: i32, c: i32) -> i32 {
    let mut count = 0i32;
    if a > 0 {
        count += 1;
    }
    if b > 0 {
        count += 1;
    }
    if c > 0 {
        count += 1;
    }
    count
}

pub fn latest(a: Option<i32>, b: Option<i32>) -> Option<i32> {
    let mut seen: Option<i32> = None;
    match a {
        Some(v) => seen = Some(v),
        None => {}
    }
    match b {
        Some(v) => {
            seen = Some(v);
        }
        None => {}
    }
    seen
}

pub fn reassigned(flag: bool, x: Option<i32>) -> i32 {
    let mut out = 1i32;
    out = match x {
        Some(v) => v,
        None => out * 10,
    };
    out = if flag { out + 1 } else { out - 1 };
    out
}

pub fn twice_matched(a: i32, b: i32) -> i32 {
    let mut total = 0i32;
    match safe_div(a, b) {
        Ok(q) => total += q,
        Err(_) => total -= 1,
    }
    match safe_div(b, a) {
        Ok(q) => total += q,
        Err(_) => total -= 1,
    }
    total
}

#[allow(unused_must_use)]
pub fn checked_first(a: i32, b: i32) -> i32 {
    a / b;
    a
}

pub fn accumulate(a: i32, b: i32) -> Result<i32, i32> {
    let mut sum = 0i32;
    sum += half_even(a)?;
    sum = sum + half_even(b)?;
    Ok(sum)
}

pub fn unit_ok(a: i32) -> Result<(), i32> {
    if a < 0i32 {
        return Err(a);
    }
    Ok(())
}

pub fn unit_some(a: i32) -> Option<()> {
    if a == 0i32 { None } else { Some(()) }
}

/// `if let` whose payload may fail to match: `Some(5)` takes the `else`, as
/// `None` does (it panicked as an unexpected variant).
pub fn refutable_if_let(x: Option<i32>, r: Result<i32, i32>) -> i32 {
    let a: i32 = if let Some(0..=9) = x { 1 } else { 2 };
    let b: i32 = if let Err(-1) = r { 10 } else { 20 };
    a + b
}
