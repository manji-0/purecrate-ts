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

/// A `let mut` read only by a `let` nothing reads: `unwrap_or`'s eager
/// default runs alone.
#[allow(unused_variables, unused_mut)]
pub fn unread_default(r: Result<i32, i32>, b: i32) -> i32 {
    let mut a: i32 = 3i32.checked_mul(r.ok().unwrap_or(2147483647)).unwrap_or(b.pow(2));
    let mut c: bool = a >= a;
    b
}

/// A loop test folding decided `false`: its left side still runs.
pub fn decided_loop(o: Option<i32>, a: i32, n: i32) -> i32 {
    let mut k: i32 = 0;
    match o {
        Some(_) => {
            while k < 100 / n && o.map(|_| a).is_none() {
                k += 1;
            }
        }
        None => {}
    }
    k
}

/// A decided `if` whose taken side declares names, with statements after
/// it: past `v3?`, `v3.map(f).is_none()` is `false` (reduced from a
/// generated body).
#[allow(unused_variables, unused_mut, unused_parens)]
pub fn decided_declaring(a: i32, b: i32, o: Option<i32>, r: Result<i32, i32>, c: bool) -> Option<i32> {
    if ((o).map(|v2| b)).is_some() {
        let mut v3: Option<i32> = o;
        let mut b: i32 = ((v3).ok_or(b)).ok().unwrap_or((65535i32 * a));
        let mut k4: i32 = 0;
        while k4 < ((v3?).checked_mul((a - a))?) % 5 && ((v3).ok_or((-7i32))).ok().is_some() {
            k4 += 1;
            if ((v3).map(|v12| (-65535i32))).is_none() {
                let v14: i32 = (match (r).map_err(|a| a) {
                    Ok(v13) => (v13 % 3i32),
                    Err(b) => (65535i32 / 100i32),
                });
            } else {
                let v21: i32 = (match {
                    let t17: Result<i32, i32> = (if c { Err(2i32) } else { r });
                    t17
                } {
                    Ok(v18) => {
                        (match r {
                            Ok(v19) => v19,
                            Err(a) => 2147483647i32,
                        })
                    }
                    Err(v20) => b,
                });
                b |= (a).checked_add(v21)?;
            }
            match (r).map_err(|v22| {
                (match r {
                    Ok(v23) => v23,
                    Err(v24) => v24,
                })
            }) {
                Ok(v25) => {}
                Err(_) => continue,
            }
        }
    }
    Some({
        let v79 = (r).ok().unwrap_or(a);
        (b).checked_mul(3i32).unwrap_or(0)
    })
}

/// `r?` where `r` is known `Err` always leaves: what follows never runs.
pub fn known_err_try(r: Result<i32, i32>, b: i32) -> Result<i32, i32> {
    match r {
        Ok(v) => Ok(v),
        Err(_) => {
            let y: i32 = r?;
            Ok(y + b)
        }
    }
}
