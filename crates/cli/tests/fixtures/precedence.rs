// Operators that JS groups differently from how the printer builds them:
// `??` beside `?:`, `||`, `&&`, and a value whose comment the printer
// leaves out.

pub fn coalesce_choice(o: Option<i32>, c: bool) -> i32 {
    match o {
        Some(v) => v,
        None => if c { 1 } else { 2 },
    }
}

pub fn coalesce_or(o: Option<bool>, a: bool, b: bool) -> bool {
    match o {
        Some(v) => v,
        None => a || b,
    }
}

pub fn or_coalesce(c: bool, o: Option<bool>, d: bool) -> bool {
    c || match o {
        Some(v) => v,
        None => d,
    }
}

pub fn and_commented(c: bool, a: bool, b: bool) -> bool {
    c && {
        // Either one.
        a || b
    }
}

pub fn first_two(x: Option<(i32, i32, i32)>) -> (i32, i32) {
    match x {
        Some((a, b, _)) => (a, b),
        None => (0, 0),
    }
}
