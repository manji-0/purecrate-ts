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

/// Past `match r { .., Err(_) => break }`, `r` is `Ok`, as TS knows.
pub fn past_break(r: Result<i32, i32>, n: i32) -> Result<i32, i32> {
    let mut k: i32 = 0;
    while k < n {
        k += 1;
        match r {
            Ok(v) => {
                k += v;
            }
            Err(_) => break,
        }
        let x: i32 = r.map_err(|e| e + 1)?;
        k += x;
    }
    Ok(k)
}

/// `if c { r } else { r }` and `{ let t = r; t }` are `r`, narrowed.
#[allow(unused_parens)]
pub fn same_sides(r: Result<i32, i32>, c: bool, a: i32) -> i32 {
    match r {
        Ok(v) => v,
        Err(_) => {
            let m: Result<i32, i32> = ({
                let t: Result<i32, i32> = (if c { r } else { r });
                t
            })
            .map_err(|e| e + a);
            match m {
                Ok(x) => x,
                Err(e) => e,
            }
        }
    }
}

/// Inside `if c`, TS has `c` as `true`: comparing it with what it knows
/// is `false` is folded, and a `match` on it takes its `true` arm.
pub fn decided_bools(c: bool, d: bool, a: i32) -> i32 {
    let mut x: i32 = a;
    if c {
        if !c == c {
            x = 1;
        }
        if !d {
            if c == d {
                x += 2;
            }
            if c != d {
                x += 3;
            }
        }
        x += match c {
            true => 10,
            _ => 20,
        };
    } else if !(d || c) {
        if d == c {
            x += 4;
        }
    }
    x
}

/// Past `if c { .. return .. }`, `c` is `false`: TS refuses `c == !c`.
pub fn past_returning_if(c: bool, k: i32) -> bool {
    if c {
        if c && c {
            return k > 0;
        }
    }
    let mut n: i32 = 0;
    while n < k % 5 && c == !c {
        n += 1;
    }
    n > 0
}

/// A `match` on what a decided `if` gives (`r`, where `c` is `false`) is
/// decided as `r` is.
#[allow(unused_variables, unused_mut, unused_parens)]
pub fn decided_scrutinee(r: Result<i32, i32>, c: bool, b: i32) -> i32 {
    match r {
        Ok(a) => {
            if c {
                a
            } else {
                match ({
                    let t: Result<i32, i32> = (if c { Ok(7i32) } else { r });
                    t
                }) {
                    Ok(_) => b.pow(1),
                    Err(e) => e % a,
                }
            }
        }
        Err(e) => e,
    }
}

/// `.ok()` of what a decided `if` gives, in a loop under `if c`.
#[allow(unused_variables, unused_assignments, unused_parens)]
pub fn decided_receiver(r: Result<i32, i32>, c: bool, b: i32) -> Option<i32> {
    let mut a: Option<i32> = None;
    if c {
        match r {
            Ok(_) => {}
            Err(_) => {
                let mut k: i32 = 0;
                while k < (b >> 3) % 5 && a.is_some() && !c {
                    a = ({
                        let t: Result<i32, i32> = (if c { r } else { Err(65535i32) });
                        t
                    })
                    .ok();
                    k += 1;
                }
            }
        }
    }
    Some(b * if c { 100 } else { 7 })
}

/// A decided test whose left side still runs, and whose taken side
/// declares a name a `let` after it declares again: the side keeps a block.
pub fn decided_redeclared(c: bool, n: i32) -> i32 {
    if c {
        let mut s: i32 = 0;
        if 100 / n > 0 && !c {
            let x: i32 = 1;
            s += x;
        } else {
            let x: i32 = 2;
            s += x;
        }
        let x: i32 = s + 3;
        x * x
    } else {
        0
    }
}

/// Equal sides that are not a place stay a choice: `None` read as a
/// `match` scrutinee is decided where it is built, not as `null ?? a`.
#[allow(unused_parens)]
pub fn equal_constructors(c: bool, a: i32) -> i32 {
    match ({
        let t: Option<i32> = (if c { None } else { None });
        t
    }) {
        Some(v) => v,
        None => a,
    }
}

/// `r?` on a known `Err` returns `r` where the function's `Ok` type differs.
pub fn known_err_other_ok(r: Result<i32, i32>) -> Result<bool, i32> {
    match r {
        Ok(v) => Ok(v > 0),
        Err(_) => {
            let y: i32 = r?;
            Ok(y > 1)
        }
    }
}

/// Narrowed, `o.unwrap_or(3)` is `o` read as its payload: a `match` on the
/// payload (`0..=9`) is not decided by `o` being `Some` (it took `_`).
pub fn payload_test(o: Option<i32>) -> bool {
    match o {
        Some(_) => matches!(o.unwrap_or(3), 0..=9),
        None => false,
    }
}

/// A `let` of `t.as_str()` nothing reads goes, and `t` with it: the
/// parameter is `_t`, as TS refuses an unread one.
#[allow(unused_variables)]
pub fn unread_as_str(s: &str, t: String, xs: Vec<i32>) -> usize {
    let n: usize = xs.len();
    let v: &str = t.as_str();
    s.len()
}

/// Past `if c { r? } else { r? }` (each side leaving on `Err`), `r` is
/// `Ok`: TS joins what both sides narrowed.
pub fn joined_sides(r: Result<i32, i32>, c: bool, b: i32) -> Result<i32, i32> {
    if c {
        let x: i32 = r.map_err(|_| -1i32)?;
        if x > b {
            return Ok(x);
        }
    } else {
        let y: i32 = r?;
        if y < b {
            return Ok(y);
        }
    }
    let z: i32 = r.map(|v| v + 1)?;
    Ok(z)
}

/// Narrowed before a loop, `r` stays narrowed inside it and past it.
pub fn narrowed_through_loop(r: Result<i32, i32>, n: i32) -> Result<i32, i32> {
    let mut k: i32 = r.map_err(|e| e + 1)?;
    let mut i: i32 = 0;
    while i < n % 4 {
        i += 1;
        k += r.map(|v| v * 2)?;
    }
    let last: i32 = r?;
    Ok(k + last)
}

/// Past `if c { return }`, `c` is `false`, so `!c != (o.is_some() && c)`
/// is decided.
#[allow(unused_parens)]
pub fn decided_and(c: bool, o: Option<i32>, n: i32) -> i32 {
    let mut k: i32 = 0;
    for i in 0..n % 5 {
        if c {
            return i;
        }
        if ((!c) != matches!(o, Some(_) if c)) {
            k += 1;
        }
    }
    k
}

/// `{ let t = if c { r } else { Err(..) }; t }?` inside `if c`, past `r?`:
/// decided as `r`, which never leaves.
#[allow(unused_parens, unused_variables)]
pub fn decided_try(r: Result<i32, i32>, c: bool) -> Result<i32, i32> {
    let x: i32 = r.map_err(|_| 1i32)?;
    if c {
        let y: i32 = {
            let t: Result<i32, i32> = (if c { r } else { Err(5i32) });
            t
        }?;
        return Ok(y + x);
    }
    Ok(x)
}

/// The same with a left side that may panic: `100 / a > 0 && c` still runs
/// the division (panicking for `a == 0`) though `c` decides the `&&`.
#[allow(unused_parens)]
pub fn decided_and_runs_left(c: bool, a: i32) -> i32 {
    if c {
        return 1;
    }
    if ((!c) != (100 / a > 0 && c)) {
        return 2;
    }
    3
}

/// Past `if c { return }` and `if o.is_some() { return }`, `o.is_none()`
/// and `c && c` are decided.
pub fn decided_after_returns(o: Option<i32>, c: bool) -> bool {
    if c {
        return o.is_some();
    }
    if o.is_some() {
        return !c;
    }
    o.is_none() != (c && c)
}

/// Inside `if c || c`, `c` is `true`.
#[allow(clippy::nonminimal_bool)]
pub fn decided_or(c: bool, a: i32) -> i32 {
    if c || c {
        if (c && c) == !c {
            return a;
        }
    }
    0
}

/// Inside `if matches!(o, Some(_) if !c)`, `o` is `Some` and `c` is
/// `false`.
pub fn decided_matches(o: Option<i32>, c: bool, n: i32) -> i32 {
    let mut k: i32 = 0;
    if matches!(o, Some(_) if !c) {
        while k < n % 5 && (matches!(o, Some(_) if c) != !c) {
            k += 1;
        }
    }
    k
}

/// `let v = true;` is `true` to TS, and `v == c` is decided where `c` is.
#[allow(clippy::eq_op)]
pub fn decided_literal_binding(c: bool, n: i32) -> i32 {
    let mut k: i32 = 0;
    if !c {
        let v: bool = true;
        while k < n % 5 && ((v == v) == (v == c)) {
            k += 1;
        }
    }
    k
}

/// `t == ","` holding, `t` is `","`: `t == ""` is decided.
pub fn decided_string(t: String, s: &str) -> bool {
    (t == "," && t == "") && s.is_empty()
}

/// Where `!(x || w)` holds, `x` is `false`: `x && r.ok().is_some()` is
/// `false`, and its right side, which TS checks with `r` known `Err`, goes.
pub fn decided_left(r: Result<i32, i32>, v: bool, w: bool) -> bool {
    match r {
        Ok(_) => v,
        Err(_) => {
            let mut x: bool = v;
            if !(x || w) {
                x = x && r.ok().is_some();
            }
            x
        }
    }
}

/// A place the loop tests and writes later in its body: at its head, `x`
/// is whatever any pass left, never what it held on the way in.
pub fn late_write(o: Option<i32>, n: i32) -> i32 {
    let mut x: Option<i32> = o;
    let mut k: i32 = 0;
    let mut i: i32 = 0;
    while i < n % 6 {
        i += 1;
        k += match x {
            Some(v) => v % 7,
            None => 1,
        };
        if i == 2 {
            x = None;
        }
        if i == 4 {
            x = Some(3);
        }
    }
    k + x.unwrap_or(0)
}

/// `match { let t = None; t } { .. }` (what an `if` of two `None`s folds
/// to) takes its `None` arm: no `null ?? a`.
#[allow(unused_parens, unused_variables)]
pub fn none_scrutinee(r: Result<i32, i32>, a: i32) -> i32 {
    match ({
        let t: Option<i32> = (match r {
            Ok(_) => None,
            Err(_) => None,
        });
        t
    }) {
        Some(v) => v,
        None => a,
    }
}

/// `o.map(f).is_none()` in a loop that writes what `o` is chosen by: the
/// test reads the `match` itself, with no temporary TS would type from
/// itself.
#[allow(unused_parens)]
pub fn mapped_test_in_loop(o: Option<i32>, n: i32) -> bool {
    let mut v: bool = false;
    let mut k: i32 = 0;
    while k < n % 4 {
        k += 1;
        v = ({
            let t: Option<i32> = (if v { None } else { o });
            t
        })
        .map(|_| 3i32)
        .is_none();
    }
    v
}

/// A `matches!` whose guard is decided `false` runs its scrutinee alone.
#[allow(unused_variables)]
pub fn decided_guard_runs(o: Option<i32>, a: i32, b: i32) -> i32 {
    let mut k: i32 = 0;
    if o.is_some() {
        if matches!(a.checked_mul(b), Some(_) if o.map(|_| 2i32).is_none()) {
            k = 1;
        }
    }
    k
}

/// `v = x?` where `x` is known `None` leaves: what follows never runs.
#[allow(unused_assignments)]
pub fn assigned_try_leaves(o: Option<i32>, c: bool) -> Option<i32> {
    let mut v: i32 = o?;
    let b: Option<i32> = None;
    if c {
        v = b?;
        v += 1;
    }
    Some(v)
}
