// Shapes the printer got wrong while 0.25.0 was made, each now as Rust
// computes it: a typed `let` in a block that an argument needs statements
// for, an integer widened from an `unwrap_or`, a test a `const` aliases
// that an earlier test of the `const` left either way, a block printed
// flat whose name a later loop binds again, an `if` narrowing decided
// that is all its arm holds, and a `!` of a test narrowing decided.

#[derive(Clone, Copy)]
pub enum Mark {
    At(i32),
    Gone,
}

fn count(xs: &[Option<Mark>]) -> usize {
    xs.len()
}

fn first_at(xs: &[Option<Mark>]) -> i32 {
    if xs.is_empty() {
        return 0;
    }
    match xs[0] {
        Some(Mark::At(k)) => k,
        Some(Mark::Gone) => -1,
        None => 0,
    }
}

pub fn typed_in_block(o: Option<i32>, k: i32) -> i32 {
    let mut n = 0;
    if k > 0 {
        n = first_at(&{
            let e: Vec<Option<Mark>> = vec![Some(Mark::At(match o {
                Some(_) => k,
                None => 1,
            }))];
            e
        }[..]);
    }
    n + count(&{
        let e: Vec<Option<Mark>> = vec![match o {
            Some(v) if v > 0 => Some(Mark::At(v)),
            Some(_) => Some(Mark::Gone),
            None => None,
        }];
        e
    }[..]) as i32
}

pub struct Scaled {
    pub f: f64,
}

pub fn widened_default(o: Option<i32>, w: Option<i64>, c: bool) -> f64 {
    let s = Scaled { f: f64::from(o.unwrap_or(100)) };
    let chosen = f64::from(o.unwrap_or(if c { 1 } else { -2 }));
    let wide = i128::from(w.unwrap_or(7));
    s.f + chosen + f64::from(o.unwrap_or(3) / 2) + wide as f64
}

pub fn alias_tested_before(a: i32, o: Option<i32>, r: Result<i32, i32>, c: bool) -> Result<i32, i32> {
    let c: bool = !c || r.is_ok();
    let w: Option<i32> = if c { None } else { Some(a) };
    let v = match r {
        Ok(v) => v,
        Err(e) => return Err(e),
    };
    if c {
        return o.ok_or(v);
    }
    let m = match r {
        Ok(x) => x,
        Err(e) => e,
    };
    Ok(w.unwrap_or(m) + v)
}

pub fn alias_of_ok_tested_before(a: i32, o: Option<i32>, r: Result<i32, i32>, c: bool) -> Result<i32, i32> {
    let c: bool = !c || r.ok().is_some();
    let mut h = 0;
    for _ in 0..2 {
        let w: Option<i32> = if c { None } else { Some(a) };
        if o == w {
            h += 1;
        }
        let 0..=3 = r.map_err(|e| e + 1)? else {
            return Ok(h);
        };
        if c {
            return o.ok_or(h);
        }
        h += match r {
            Ok(x) => x,
            Err(e) => e,
        };
    }
    Ok(h)
}

pub fn block_before_loop_var(a: i32, b: i32, o: Option<i32>, r: Result<i32, i32>, c: bool) -> Option<i32> {
    let mut h0: i32 = 0;
    let mut n = 0;
    if o? > 100 {
        h0 += 1;
    }
    loop {
        match o {
            Some(v6) => {
                {
                    let b: i32 = if c { b } else { v6 };
                    if b > 0 {
                        h0 += 1;
                    }
                }
                match r {
                    Ok(v12) => {
                        let p = v12 % 2;
                        h0 += p;
                    }
                    Err(_) => {}
                }
            }
            None => {}
        }
        let mut k21: i32 = 0;
        while k21 < a % 3 {
            k21 += 1;
            for b in 0..a % 4 {
                h0 += b;
            }
        }
        n += 1;
        if h0 > 10 || n > 3 {
            return Some(h0);
        }
    }
}

#[allow(unused_assignments)]
pub fn decided_block_last(b: i32, o: Option<i32>, r: Result<i32, i32>) -> i32 {
    let mut h0: i32 = 0;
    if o.map(|v| match o { Some(_) => v, None => 1 }).is_none() {
        return -1;
    }
    let mut k = 0;
    while k < b % 5 {
        k += 1;
        let mut seen: bool = k > 2;
        match if seen { r } else { r } {
            Ok(v) => {
                if o != None {
                    let q: i32 = match o {
                        Some(q) => q,
                        None => -1,
                    };
                    h0 = h0.wrapping_add(q);
                }
                seen = v > 0;
            }
            Err(_) => continue,
        }
        h0 += 1;
    }
    h0
}

pub fn negated_decided(c: bool, n: i32) -> i32 {
    if c {
        return 1;
    }
    let v: bool = !(c && c);
    let mut h = n;
    for _ in 0..3 {
        if (v || c) == c {
            h += 1;
        }
    }
    h
}
