// `cmp` without naming `Ordering`: the enum is added, internal, for the
// methods read off the result.

pub fn before(a: &str, b: &str) -> bool {
    a.cmp(b).is_lt()
}

pub fn settled(a: u64, b: u64, c: u64) -> bool {
    a.cmp(&b).then(b.cmp(&c)).is_eq()
}
