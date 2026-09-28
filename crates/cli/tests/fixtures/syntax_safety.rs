pub struct P {
    pub a: i32,
    pub b: i32,
}

pub fn special(s: &str) -> bool {
    s == "q\"\\\n\r\t\u{0}\u{1b}\u{7f}\u{2028}\u{2029}é"
}

fn mk(a: i32) -> Box<P> {
    Box::new(P { a, b: a })
}

pub fn boxed_sum(a: i32) -> i32 {
    let p = mk(a);
    p.a + p.b
}

fn some_p(a: i32) -> Option<P> {
    Some(P { a, b: 1 })
}

pub fn some_sum(a: i32) -> i32 {
    match some_p(a) {
        Some(p) => p.a + p.b,
        None => 0,
    }
}

#[allow(unused_must_use)]
pub fn boxed_effect(a: i32) -> i32 {
    Box::new(P { a, b: a });
    a
}

fn id(x: i32) -> i32 {
    x
}

fn id_opt(x: Option<i32>) -> Option<i32> {
    x
}

pub fn lift_temp(x: Option<i32>) -> Option<i32> {
    let q1 = x?;
    let r = q1 + id(x?);
    Some(r)
}

pub fn match_temp(x: Option<i32>) -> Option<i32> {
    let m1 = x?;
    match id_opt(x) {
        Some(v) => Some(v + m1),
        None => None,
    }
}

pub enum Error {
    Code(i32),
}

impl Error {
    pub fn code(n: i32) -> Error {
        Error::Code(n)
    }
}

pub fn at(xs: Vec<i32>, i: usize) -> Result<i32, Error> {
    if i < 5 {
        Ok(xs[i])
    } else {
        Err(Error::code(1))
    }
}
