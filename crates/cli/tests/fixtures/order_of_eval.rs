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

/// A `?` inside what `ok_or(e)?` takes leaves the function; two `ok_or(e)?`
/// in one body keep their values apart.
pub fn try_in_ok_or(a: i32, v: i32, fail: bool) -> Result<i32, i32> {
    let s = a.checked_add(given(v, fail)?).ok_or(-1i32)?;
    let t = s.checked_mul(2i32).ok_or(-2i32)?;
    Ok(t)
}

/// Two `map_err(f)?` in one body keep their results apart.
pub fn two_mapped_tries(v: i32, fail_first: bool, fail_second: bool) -> Result<i32, i32> {
    let a = given(v, fail_first).map_err(|e| e + 1)?;
    let b = given(v + 1, fail_second).map_err(|e| e + 2)?;
    Ok(a * 10 + b)
}

/// A `?` in what `ok_or` takes runs before the test, as Rust evaluates the
/// argument whether or not the option is `Some`: an `Err` leaves even when
/// `checked_add` succeeds, and the division may panic first.
pub fn try_in_ok_or_arg(a: i32, v: i32, fail: bool) -> Result<i32, i32> {
    let x = a.checked_add(1).ok_or(a / given(v, fail)?)?;
    Ok(x)
}

/// The same with `ok_or(..)?` as an operand.
pub fn try_in_ok_or_arg_operand(a: i32, v: i32, fail: bool) -> Result<i32, i32> {
    Ok(a.checked_add(1).ok_or(a / given(v, fail)?)? + 1)
}

/// An `unwrap_or` inside what `ok_or(e)?` takes keeps its option apart from
/// the guarded one (both were `opt`).
pub fn unwrap_or_in_ok_or(a: i32) -> Result<i32, i32> {
    Ok(7i32.checked_add(a.checked_sub(1).unwrap_or(5)).ok_or(a)? % 3)
}

/// `ok_or` as a scrutinee: TS learns the `Result`'s error type from its
/// annotation, as it does from a user's annotated block.
pub fn ok_or_scrutinee(o: Option<i32>, b: i32) -> i32 {
    match o.ok_or(b) {
        Ok(x) => x * 2,
        Err(e) => e,
    }
}

pub fn annotated_scrutinee(a: i32) -> i32 {
    match {
        let t: Result<i32, i32> = Err(a);
        t
    } {
        Ok(x) => x,
        Err(e) => e + 1,
    }
}

/// A parameter read only where a constant test folds away is unread in TS.
#[allow(unused_parens)]
pub fn folded_param(a: i32, b: i32) -> i32 {
    (if false { a } else { 1 }) + b
}

/// The same with a `map_err(f)?`, which typing has made a `match` that
/// returns; `-a` runs before it, and may panic first.
pub fn mapped_try_in_ok_or_arg(a: i32, v: i32, fail: bool) -> Result<i32, i32> {
    let x = a.checked_add(1).ok_or(-a * given(v, fail).map_err(|e| e * 2)?)?;
    Ok(x)
}
