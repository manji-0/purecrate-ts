// A test TS has already decided by narrowing is folded, never printed:
// TS refuses `r.kind === "Ok"` where `r` is known `Err` ("no overlap").
// Each fold holds only while nothing writes the place.

/// `r` read again inside its own arm.
pub fn rematch(r: Result<i32, i32>, b: i32) -> i32 {
    match r {
        Ok(v) => r.ok().unwrap_or(b) + v,
        Err(e) => match r {
            Ok(x) => x,
            Err(y) => y + e,
        },
    }
}

/// `r` read in the closure `map_err` runs on its `Err`.
pub fn read_in_map_err(r: Result<i32, i32>) -> Result<i32, i32> {
    Ok(r.map_err(|e| if r.ok().is_some() { 1 } else { e })?)
}

/// Past `o?`, `o.unwrap_or(a)` is `o`, and `a` is unread.
#[allow(unused_variables)]
pub fn past_try(o: Option<i32>, a: i32) -> Option<i32> {
    let v = o?;
    Some(v + o.unwrap_or(a))
}

/// Past `map_err(f)?`, a second one never returns.
pub fn past_mapped_try(r: Result<i32, i32>, a: i32, b: i32) -> Result<i32, i32> {
    let x = r.map_err(|_| a)?;
    let y = r.map_err(|_| b)?;
    Ok(x + y)
}

/// Written after the `?`: nothing is folded.
pub fn written_past_try(x: Option<u32>, y: Option<u32>) -> Option<u32> {
    let mut o = x;
    let v = o?;
    o = y;
    Some(v + o.unwrap_or(7))
}

/// Written after a `map_err(f)?` on it: nothing is folded.
pub fn written_past_mapped_try(r: Result<u32, u32>, s: Result<u32, u32>) -> Result<u32, u32> {
    let mut q = r;
    let v = q.map_err(|e| e + 1)?;
    q = s;
    Ok(v + match q {
        Ok(w) => w,
        Err(e) => e,
    })
}

/// A `let` of a constructor decides the `match` on it.
pub fn bound_constructor(a: i32, c: bool) -> i32 {
    let o = if c { Some(a) } else { None };
    let p = Some(a);
    o.unwrap_or(0) + p.unwrap_or(1)
}

/// A guard narrowing decided, and two literals compared.
#[allow(clippy::eq_op)]
pub fn decided_guard(o: Option<i32>, c: bool) -> i32 {
    if c && 1i32 == 65535i32 {
        return 9;
    }
    if matches!(o, Some(v) if o.map(|_| v).is_none()) {
        1
    } else {
        2
    }
}
