// Shapes whose output oxlint refused, each now as Rust computes it: a test
// or a sum that nothing reads, a loop whose body is left with nothing, a
// name read only to write itself, an `if` run for what its sides do, a
// closure whose parameter is read only by a binding nothing reads, a
// negated test turned round, an arm that returns once the flow decides its
// test, an `else` that is one guard, a name assigned to itself, and a
// `p?` left alone by an unread assignment, which TS narrows `p` by.

#[allow(unused_variables, unused_assignments)]
pub fn unread_tests(a: i32, b: i32, c: bool) -> i32 {
    let mut v: bool = c;
    v = a.checked_sub(b).is_some();
    let w: bool = !c || a.checked_add(b).map(|x| x % b).is_none();
    a
}

#[allow(unused_variables)]
pub fn emptied_loop(a: i32, b: i32) -> i32 {
    for v in a % 4..b % 6 {
        let big: bool = v > 0;
    }
    a
}

#[allow(unused_assignments)]
pub fn reads_itself(a: i32, b: i32, c: bool) -> i32 {
    let mut f: bool = a > b;
    f = c && f;
    a
}

#[allow(unused_variables)]
pub fn sides_run(a: i32, b: i32, c: bool) -> i32 {
    let x: i32 = if c { a.abs() } else { b.abs() };
    a
}

#[allow(unused_variables)]
pub fn param_unread(a: i32, b: i32) -> i32 {
    let g = |a: i32| {
        let a: i32 = a | a;
        b
    };
    g(a)
}

pub fn turned_round(a: i32, c: bool) -> i32 {
    let mut h: i32 = 0;
    if !c {
    } else {
        h += a;
    }
    h
}

pub fn decided_return(r: Result<i32, i32>, a: i32) -> i32 {
    let mut h: i32 = 1;
    match r {
        Ok(v) => {
            if r.ok().is_some() {
                return v;
            }
        }
        Err(_) => {
            let x: i32 = a.abs();
            h += x;
        }
    }
    h + 1
}

#[allow(clippy::collapsible_else_if)]
pub fn else_guard(a: i32, c: bool) -> Option<i32> {
    let mut h: i32 = 0;
    if c {
        h += 1;
    } else {
        if a > 0 {
            return Some(h);
        }
    }
    Some(h + a)
}

pub fn keeps_itself(a: i32, o: Option<i32>, r: Result<i32, i32>, c: bool) -> Option<i32> {
    let mut v: Option<i32> = o;
    v = if c { Some(a) } else { v };
    v = match r {
        Ok(_) => v,
        Err(_) => None,
    };
    v
}

#[allow(unused_assignments, unused_variables)]
pub fn unread_try(a: i32, b: i32, r: Result<i32, i32>) -> Result<i32, i32> {
    let mut v: i32 = a << 0;
    v = r?;
    let x: i32 = b * r.map_err(|e| e)?;
    v &= a;
    Ok(a)
}
