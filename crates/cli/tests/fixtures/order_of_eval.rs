pub struct P {
    pub a: i32,
    pub b: i32,
}

pub fn mk(a: i32, z: i32) -> P {
    P { a: a / z, b: 0 }
}

pub fn mk_or(b: i32, fail: bool) -> Result<P, i32> {
    if fail {
        Err(b)
    } else {
        Ok(P { a: 0, b })
    }
}

pub fn given(v: i32, fail: bool) -> Result<i32, i32> {
    if fail {
        Err(v)
    } else {
        Ok(v)
    }
}

pub fn maybe(v: i32, none: bool) -> Option<i32> {
    if none {
        None
    } else {
        Some(v)
    }
}

pub fn diff(x: i32, y: i32) -> i32 {
    x - y
}

/// `a / z` runs before `?`: a zero divisor panics even when `r` is `Err`.
pub fn try_after_div(a: i32, z: i32, v: i32, fail: bool) -> Result<i32, i32> {
    let r: Result<i32, i32> = given(v, fail);
    Ok(a / z + r?)
}

/// The index runs before `?`.
pub fn try_after_index(xs: Vec<i32>, i: usize, v: i32, none: bool) -> Option<i32> {
    let r: Option<i32> = maybe(v, none);
    Some(xs[i] + r?)
}

/// Arguments run left to right, and the first one is not a `?`.
pub fn try_in_second_arg(a: i32, z: i32, v: i32, fail: bool) -> Result<i32, i32> {
    Ok(diff(a / z, given(v, fail)?))
}

/// Fields run before `..base`.
pub fn update_field_first(a: i32, z: i32) -> i32 {
    let p = P { a: a + 2147483647, ..mk(a, z) };
    p.a + p.b
}

/// A field that panics runs before a base whose `?` would leave.
pub fn update_before_try(a: i32, z: i32, b: i32, fail: bool) -> Result<i32, i32> {
    let p = P { a: a / z, ..mk_or(b, fail)? };
    Ok(p.a + p.b)
}

/// A `for` range's start runs before a `?` in its end.
pub fn try_in_range_end(a: i32, v: i32, fail: bool) -> Result<i32, i32> {
    let mut t = 0i32;
    for i in (a + 1)..given(v, fail)? {
        t = i;
    }
    Ok(t)
}

/// The payload is read before the arm assigns the place it came from.
pub fn reassigned_scrutinee(x: Option<u32>) -> u32 {
    let mut o = x;
    let v = match o {
        Some(v) => {
            if v > 5 {
                o = None;
            }
            v
        }
        None => return 0,
    };
    match o {
        Some(_) => 1,
        None => v,
    }
}

/// A `?` that typing turns into a `match` leaves the function from an
/// operand and from an argument, after the operands before it.
pub fn mapped_try_operand(a: i32, v: i32, fail: bool) -> Result<i32, i32> {
    Ok(a + 1 + given(v, fail).map_err(|e| e * 2)?)
}

pub fn try_in_default(x: Option<i32>, v: i32, fail: bool) -> Result<i32, i32> {
    Ok(x.unwrap_or(given(v, fail)?) + 0)
}
