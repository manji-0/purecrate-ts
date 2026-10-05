// Shapes the examples met while fixing their reviews, each once printed in
// a way tsc, oxlint, or oxfmt refused, or refused by `check` where rustc
// accepts it.

pub enum P {
    A,
    B,
}

/// `enumerate` over enum elements: the loop iterates the element itself,
/// with no cast to undo a narrowing it never had.
pub fn counted(ps: Vec<P>) -> usize {
    let mut n: usize = 0;
    for (i, p) in ps.iter().enumerate() {
        if i > 0 && matches!(p, P::A) {
            n += 1;
        }
    }
    n
}

/// `is_eq` of a call on the right of `&&`: one test of `kind`.
pub fn same(a: bool, x: Vec<String>, y: Vec<String>) -> bool {
    a && x.cmp(&y).is_eq()
}

/// A `match` on the right of `&&` or `||`: an `if`, not an inline function.
pub fn mixed(a: bool, n: u64) -> bool {
    a && match n % 3 {
        0 => true,
        1 => n > 5,
        _ => n == 2,
    }
}

pub fn either(a: bool, n: u64) -> bool {
    let b = a || match n % 3 {
        0 => n > 3,
        _ => false,
    };
    b
}

/// A pushed `match`: `s += ..`.
pub fn digits(n: u64) -> String {
    let mut s = String::new();
    s.push(match n % 10 {
        0 => '0',
        1 => '1',
        _ => '2',
    });
    s
}

/// `_` and a `_`-named variable in `for`.
pub fn zeros(xs: Vec<u32>) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    for _ in 0..2usize {
        v.push(0);
    }
    for _ in xs.iter() {
        v.push(1);
    }
    for _high in 0..2usize {
        v.push(2);
    }
    let high: u32 = 3;
    for _high in 0..1usize {
        v.push(high);
    }
    v
}

pub enum E {
    A,
    B,
}

pub struct W {
    pub xs: Vec<u32>,
    pub n: u32,
}

fn g(xs: &Vec<u32>) -> Result<u32, u8> {
    if xs.len() > 3 {
        Err(3u8)
    } else {
        Ok(1u32)
    }
}

/// A guarded arm that returns, before one that needs statements: no `else`.
pub fn guarded(e: E, xs: Vec<u32>) -> Result<W, u8> {
    let next = match e {
        E::A if xs.is_empty() => return Err(1u8),
        E::A => W { xs: xs.clone(), n: g(&xs)? },
        E::B => return Err(2u8),
    };
    Ok(next)
}

/// A literal branch takes the other branch's type.
pub fn clamp(second: i64) -> i64 {
    let kept = if second == 60 { 59 } else { second };
    kept
}

/// A test made of a payload check and a `match` (`Some(a)` then a test that
/// needs statements): the `&&` stays an expression, so TS keeps `a`
/// narrowed in the branch it chooses.
pub fn narrowed(e: Option<i32>, c: bool, b: i32) -> Result<i32, i32> {
    match e {
        Some(a) => {
            if ({
                let t: Result<i32, i32> = if c { Ok(10i32) } else { Ok(0i32) };
                t
            })
            .ok()
            .is_some()
            {
                Ok(2i32 & a)
            } else {
                Err(b)
            }
        }
        None => Err(b),
    }
}
