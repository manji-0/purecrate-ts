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

/// A `matches!` of many byte literals inside `&&`: an array of numbers,
/// filled as oxfmt fills it.
pub fn zero_low_bits(b: Vec<u8>) -> bool {
    !b.is_empty()
        && matches!(
            b[0],
            b'A' | b'E' | b'I' | b'M' | b'Q' | b'U' | b'Y' | b'c' | b'g' | b'k' | b'o' | b's' | b'w' | b'0' | b'4' | b'8'
        )
}

pub enum Stage {
    Waiting { count: u32, done: bool },
    Idle,
}

pub enum Msg {
    A,
    B,
    C(u32),
}

/// A field tested by a literal pattern, named like an outer local: the
/// field's read takes another name, so it hides nothing (`no-shadow`).
pub fn unshadowed(stage: Stage, m: Msg) -> u32 {
    let done = matches!(stage, Stage::Waiting { done: true, .. });
    let busy = !done;
    match (stage, m) {
        (Stage::Waiting { count, done: false }, Msg::A) => count,
        (Stage::Idle, Msg::B) => 1,
        (_, Msg::C(n)) => {
            if busy {
                n
            } else {
                0
            }
        }
        (_, _) => 2,
    }
}

/// A byte literal's comment before the parentheses of its cast.
pub fn wild(x: u32, b: u32) -> bool {
    x == u32::from(b'?') || x == b
}

/// A long `else if` test, and a long logical value assigned to a name, laid
/// out as oxfmt lays them out.
pub fn scan(pattern: Vec<u32>, name: Vec<u32>, any: u32) -> u32 {
    let mut p: usize = 0;
    let mut n: usize = 0;
    let mut k: u32 = 0;
    while n < name.len() && k < 100 {
        k += 1;
        if p >= pattern.len() {
            n += 1;
        } else if p < pattern.len() && (pattern[p] == any || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else {
            n += 1;
        }
    }
    k
}

fn listed(xs: &Vec<u32>, x: u32) -> bool {
    xs.iter().any(|y| *y == x)
}

pub fn strictness(ours: Vec<u32>, theirs: Vec<u32>, c: bool) -> bool {
    let mut strict = false;
    if c {
        strict = (listed(&ours, 1000000001) && listed(&theirs, 2000000002))
            || (listed(&ours, 3000000003) && listed(&theirs, 4000000004));
    }
    strict
}

/// A tuple `let` whose value returns in one arm.
pub fn tuple_let(x: Option<u32>) -> u32 {
    let (a, b) = match x {
        Some(v) => (v, v + 1),
        None => return 0,
    };
    a + b
}
