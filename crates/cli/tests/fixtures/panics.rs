// `panic!`, `unreachable!`, `assert!`, and `Option::unwrap` / `expect`:
// each throws Rust's message, an `assert!` with no message the condition as
// rustc prints it; a message is built only when the panic happens, and
// `expect`'s argument is evaluated on `Some` too.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Red,
    Amber,
    Green,
}

pub fn half(n: u32) -> u32 {
    assert!(n % 2 == 0);
    n / 2
}

pub fn within(n: i32, cap: i32) -> i32 {
    assert!(n < cap && -n < cap, "{n} is out of {cap}");
    n * 2
}

/// The message names `n + 1`, which overflows only when the assertion
/// fails: Rust builds it then, so `n == 255` panics with the overflow.
pub fn lazy_message(n: u8) -> u8 {
    assert!(n < 200, "too big by {}", n + 1 - 200);
    n
}

pub fn shown(xs: Vec<u8>, k: usize) -> usize {
    assert!(!xs.is_empty() && xs[0] > (xs.len() as u8).wrapping_sub(1) * 2);
    assert!(k <= xs.len(), "k = {k}");
    assert!(xs.iter().all(|x| *x != 7), "no sevens");
    xs.len() - k
}

pub fn calls(a: i64, b: i64) -> i64 {
    assert!(a.abs() < b.max(1));
    assert!(-(a - 1) != b * 3);
    assert!(a.checked_mul(b).is_some());
    a * b
}

pub fn pick(k: u8) -> u8 {
    match k % 4 {
        0 => 10,
        1 => 20,
        2 => panic!("two: {k}"),
        3 => panic!(),
        _ => unreachable!(),
    }
}

pub fn odd(n: u64) -> u64 {
    if n % 2 == 0 {
        unreachable!("even {}", n)
    }
    n
}

pub fn next(l: Light, stop: bool) -> Light {
    match (l, stop) {
        (Light::Red, false) => Light::Green,
        (Light::Green, false) => Light::Amber,
        (Light::Amber, false) => Light::Red,
        (_, true) => panic!("stopped at {}", if l == Light::Red { "red" } else { "not red" }),
    }
}

/// A panic as a statement, after which the block still has a value.
pub fn checked_sum(xs: Vec<i32>) -> i32 {
    let mut total = 0i32;
    for x in xs {
        if x < 0 {
            panic!("negative: {x}");
        }
        total += x;
    }
    total
}

pub fn must(o: Option<i32>) -> i32 {
    o.unwrap() + 1
}

pub fn must_say(o: Option<String>, why: &str) -> String {
    o.expect(why)
}

pub fn must_literal(o: Option<u8>) -> u8 {
    o.expect("a byte")
}

/// The message is built on `Some` too: `format!` runs before `expect`.
pub fn must_built(o: Option<u8>, n: u8) -> u8 {
    o.expect(&format!("no byte after {}", n + 1))
}
