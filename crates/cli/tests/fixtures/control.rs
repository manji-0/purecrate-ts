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
