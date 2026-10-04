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

fn doubled(x: i32) -> i32 {
    x * 2
}

/// A choice that `ok()` or `map(f)` prints as `?:`, read by `is_some`,
/// `is_none`, `==`, `!`, or a member: the test is of the whole choice.
pub fn ok_is_some(r: Result<i32, i32>) -> i32 {
    if r.ok().is_some() { 1 } else { 0 }
}

pub fn mapped_is_none(o: Option<i32>) -> bool {
    o.map(|x| x > 0).is_none()
}

pub fn same_presence(o: Option<i32>, p: Option<i32>) -> bool {
    o.is_some() == p.map(doubled).is_some()
}

pub fn not_mapped(o: Option<i32>) -> bool {
    !o.map(doubled).is_some()
}

pub fn len_or_empty(o: Option<Vec<u8>>) -> usize {
    o.unwrap_or(vec![]).len()
}
