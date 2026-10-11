// Shapes the statements generator found wrong once it drew blocks,
// shadowing, and names the translator makes up, each now as Rust computes
// it: a block's binding inside a branch, a loop, or a value (it ended past
// the block, or two blocks declared one name twice), a closure bound in a
// block folded into an expression, a `match` in a closure on a place its
// arm has decided, a `?` the arm has decided, an assignment of a `?` known
// to fail, a test of a `bool` that decides only when it fails, and an arm
// left empty.

#[allow(unused_variables)]
pub fn block_in_branch(a: i32, c: bool) -> i32 {
    let mut h: i32 = 0;
    if c {
        {
            let a: i32 = 100;
            h += a;
        }
        h += a;
    } else {
        {
            let a: i32 = 1;
        }
        h += a;
    }
    h
}

pub fn block_in_loop(a: i32, n: i32) -> i32 {
    let mut h: i32 = 0;
    for _ in 0..n {
        {
            let a: i32 = 100;
            h += a;
        }
        h += a;
    }
    h
}

#[allow(unused_variables)]
pub fn block_in_value(a: i32, b: i32) -> i32 {
    let y: bool = {
        {
            let b: i32 = 7;
        }
        b < 0
    };
    let b: i32 = b * a;
    if y {
        b
    } else {
        b + 1
    }
}

pub fn blocks_side_by_side(a: i32, o: Option<i32>) -> i32 {
    let mut h: i32 = 0;
    {
        let v: i32 = 1 + a;
        h += v;
    }
    {
        let v: i32 = o.unwrap_or(2) + a;
        h += v;
    }
    let v: i32 = h * 2;
    v
}

pub fn closure_in_value(a: i32) -> i32 {
    let y: i32 = {
        let g = |x: i32| x + a;
        g(1)
    };
    y
}

pub fn closure_in_arm(r: Result<i32, i32>) -> i32 {
    match r {
        Ok(v) => v,
        Err(_) => {
            let g = |x: i32| x + 1;
            g(1)
        }
    }
}

pub fn decided_in_closure(r: Result<i32, i32>) -> i32 {
    match r {
        Ok(v) => v,
        Err(_) => {
            let g = |x: i32| match r {
                Ok(a) => a,
                Err(e) => e + x,
            };
            g(1)
        }
    }
}

pub fn decided_try(c: bool, r: Result<i32, i32>) -> Result<i32, i32> {
    match r {
        Ok(_) => {
            let _v: i32 = match {
                let t: Result<i32, i32> = if c { r } else { Ok(-2) };
                t
            } {
                Ok(_) => r?,
                Err(e) => e,
            };
        }
        Err(_) => {}
    }
    Ok(1)
}

#[allow(unused_assignments)]
pub fn failing_try(a: i32) -> Option<i32> {
    let mut v: i32 = 0;
    v |= a.checked_mul(2)?;
    let w: Option<i32> = None;
    v = w?;
    Some(v)
}

pub fn twice_tried(r: Result<i32, i32>, b: i32) -> Result<i32, i32> {
    let a: i32 = (r?).checked_mul(b).ok_or(r?)?;
    let f = |x: i32| x + r.ok().unwrap_or(0);
    Ok(f(a))
}

pub fn decides_failing(r: Result<i32, i32>, c: bool) -> i32 {
    let d: bool = !c || r.ok().is_some();
    if !d {
        r.ok().unwrap_or(-7)
    } else {
        1
    }
}

pub fn empty_arm(a: i32, r: Result<i32, i32>) -> i32 {
    let mut h: i32 = a;
    match r {
        Ok(v) => {
            let _x: i32 = v;
        }
        Err(e) => {
            h += e;
        }
    }
    h
}
