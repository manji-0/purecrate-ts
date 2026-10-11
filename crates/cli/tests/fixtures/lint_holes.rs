// Shapes whose output oxlint refused, each now as Rust computes it: a test
// or a sum that nothing reads, a loop whose body is left with nothing, a
// name read only to write itself, an `if` run for what its sides do, a
// closure whose parameter is read only by a binding nothing reads, a
// negated test turned round, an arm that returns once the flow decides its
// test, an `else` that is one guard, an arm that keeps an assigned name, a
// `p?` left alone by an unread assignment, which TS narrows `p` by, and a
// closure parameter read only where the flow decided the code away.

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

#[allow(unused_variables)]
pub fn unread_try_last(a: i32, r: Result<i32, i32>) -> Result<i32, i32> {
    let b: i32 = r.ok().unwrap_or(r?);
    let x: i32 = a * r.map_err(|e| e + 1)?;
    Ok(a)
}

#[allow(unused_variables)]
pub fn closure_param_decided(a: i32, b: i32, c: bool) -> i32 {
    let f = |a: i32| {
        if c {
            let d: i32 = a & (if c { b } else { a + a });
        }
        0i32
    };
    if c { f(a) } else { b }
}

// Assertions lint takes as unneeded: on what a function taking any number
// is passed, on a value asserted again through `number`, and on the sides
// of a choice asserted so; and unread `f64` sums, conversions, and copies.

pub fn literal_to_float(x: f64, n: i64) -> f64 {
    let a: f64 = (7i64 as f64).abs() + (n as f64);
    let r: f64 = f64::from((1.5f32).abs());
    a + r + x
}

pub fn byte_widened(b: u8) -> u32 {
    let open: u32 = u32::from(b'[');
    open + u32::from(b)
}

pub fn literal_cast(k: i32) -> u8 {
    (300i32 as u8).wrapping_add(k as u8)
}

pub fn choice_cast(c: bool, m: u32) -> usize {
    (if c { m } else { 100u32 }) as usize
}

pub fn small_big_float(n: i64) -> f32 {
    (n as f32) + (7i64 as f32)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Px {
    pub c: u8,
    pub f: f64,
}

pub fn deep_literal(ps: Vec<Px>) -> usize {
    ps.iter().filter(|p| p.c == 3u8).count()
}

#[allow(unused_variables, unused_assignments)]
pub fn unread_float(x: f64, y: f64, m: u32, xs: Vec<i32>) -> f64 {
    let mut a: f64 = 100.0;
    a = y + (a % y);
    let b: f64 = x * 1e300;
    let n: f64 = f64::NAN;
    let w: f64 = f64::from(m);
    let copy: Vec<i32> = xs.clone();
    x
}

// An unread collect runs the stage it collects, whose closure may panic.
#[allow(unused_variables, unused_assignments)]
pub fn unread_collect(ps: Vec<Px>, k: i32) -> usize {
    let mut kept: Vec<Px> = Vec::new();
    kept = ps.iter().filter(|p| i32::from(p.c) * 16777216 > k).cloned().collect::<Vec<Px>>();
    ps.len()
}

// An assignment that keeps the name on a side the flow leaves, an
// assignment of a name to itself, a loop emptied by the flow whose bounds
// were named first, and a temporary inside a loop whose variable looks
// like one.

#[allow(clippy::self_assignment, clippy::nonminimal_bool)]
pub fn keeps_on_a_side(a: i32, o: Option<i32>, r: Result<i32, i32>, c: bool) -> Option<i32> {
    let mut v: Option<i32> = o;
    let mut f: bool = c;
    if o.is_some() {
        v = if v.map(|x| x).is_none() { Some(a.checked_add(1)?) } else { v.map(|x| x) };
        f = !(!f);
    }
    v = match r {
        Ok(_) => v,
        Err(e) => Some(e.checked_mul(2)?),
    };
    if f { v } else { None }
}

#[allow(unused_variables)]
pub fn emptied_named_loop(a: i32, r: Result<i32, i32>) -> Result<i32, i32> {
    let k: i32 = r?;
    for v in a.wrapping_mul(3) % 4..(r? % 6) {
        let x: i32 = r?;
    }
    Ok(a.wrapping_add(k))
}

#[allow(unused_parens)]
pub fn loop_value2(a: i32, c: bool) -> i32 {
    let value: i32 = a % 5;
    let mut h: i32 = value;
    for value2 in 0..(a % 4) {
        if (if !c { matches!(a % 7, 0..=9 | 100) } else { a > 0 }) {
            h += value2;
        }
    }
    h
}

// A name read only to decide its own `x = x || r`, and a choice of plain
// numbers widened to `f64`.

#[allow(unused_assignments)]
pub fn guards_itself(xs: Vec<i32>, i: usize, k: i32, c: bool) -> bool {
    let mut flag: bool = false;
    if c {
        return false;
    }
    flag = flag || xs[i] > k;
    flag = flag && xs[i % 2] < k;
    true
}

pub fn choice_of_numbers(a: f32, z: f32) -> f64 {
    let r: f64 = f64::from(if a == 2.0 * z { -(a % z) } else { 2.5 - a / z });
    r
}
