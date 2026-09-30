// `true` and `false` as patterns: a `match` on a `bool` naming both or
// ending in `_`, in a tuple `match`, and in `matches!`.

pub fn pick(b: bool) -> u32 {
    match b {
        true => 1,
        false => 2,
    }
}

pub fn only_true(b: bool) -> u32 {
    match b {
        true => 7,
        _ => 0,
    }
}

pub fn either(b: bool) -> u32 {
    match b {
        false | true => 3,
    }
}

pub fn gate(fresh: bool, code: Option<u32>, strict: bool) -> u32 {
    match (fresh, code, strict) {
        (true, None, _) => 0,
        (true, Some(c), true) if c > 100 => c - 100,
        (true, Some(c), _) => c,
        (false, _, true) => 1,
        (false, _, false) => 2,
    }
}

pub fn is_on(b: bool) -> bool {
    matches!(b, true)
}
