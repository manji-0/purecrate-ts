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
