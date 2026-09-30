// Bindings the Rust leaves unused. rustc only warns (or, with a leading
// `_`, is silent); the generated TS must pass `noUnusedLocals` and
// `noUnusedParameters` and still evaluate every value Rust evaluates.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    Circle { r: i32 },
    Rect(i32, i32),
    Empty,
}

#[allow(unused_variables)]
pub fn ignored_param(x: i32, y: i32) -> i32 {
    y
}

pub fn underscored_param(_x: i32, y: i32) -> i32 {
    y
}

/// The value still overflows, as in Rust.
#[allow(unused_variables)]
pub fn unused_let(a: i32, b: i32) -> i32 {
    let sum = a + b;
    let _product = a * b;
    b
}

/// `?` on a value whose payload is never read.
pub fn unused_try(flag: i32) -> Result<i32, i32> {
    let r: Result<i32, i32> = if flag == 0i32 { Ok(1i32) } else { Err(flag) };
    let _n = r?;
    Ok(1i32)
}

#[allow(unused_variables)]
pub fn unused_arm(s: Shape) -> i32 {
    match s {
        Shape::Circle { r } => 1i32,
        Shape::Rect(w, h) => w,
        Shape::Empty => 0i32,
    }
}

#[allow(unused_variables)]
pub fn unused_tuple_arm(a: Option<i32>, b: Option<i32>) -> i32 {
    match (a, b) {
        (Some(x), Some(y)) => y,
        (Some(x), None) => 1i32,
        (None, _) => 0i32,
    }
}

#[allow(unused_variables)]
pub fn unused_closure_param(a: i32) -> i32 {
    let k = |x: i32, y: i32| y + 1i32;
    k(a, a)
}

#[allow(unused_variables)]
pub fn unused_if_let(a: Option<i32>) -> i32 {
    if let Some(x) = a {
        1i32
    } else {
        0i32
    }
}

#[allow(unused_variables)]
pub fn unused_loop_var(n: u8) -> u32 {
    let mut count = 0u32;
    for i in 0u8..n {
        count += 1u32;
    }
    count
}

#[allow(unused_variables)]
pub fn unused_char(s: &str) -> u32 {
    let mut count = 0u32;
    for c in s.chars() {
        count += 1u32;
    }
    count
}

/// Written but never read: the write's value is still computed.
#[allow(unused_assignments, unused_variables)]
pub fn write_only(a: i32, b: i32) -> i32 {
    let mut last = 0i32;
    last = a * b;
    if a > 0i32 {
        last = 1i32;
    }
    b
}
