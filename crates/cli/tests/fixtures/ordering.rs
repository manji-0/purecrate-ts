// `std::cmp::Ordering`: `cmp` on every type that has it here, string
// ordering by code point, `Ordering`'s methods (with the evaluation order of
// `then` and `then_with`), `==`, `as`, `match` with guards, `matches!`, and
// the fully qualified forms.

use std::cmp::{Ordering};
use uuid::Uuid;

use Size::*;

pub fn cmp_str(a: &str, b: &str) -> Ordering {
    a.cmp(b)
}

pub fn cmp_string(a: String, b: String) -> Ordering {
    a.cmp(&b)
}

pub fn ops_str(a: &str, b: &str) -> (bool, bool, bool, bool) {
    (a < b, a <= b, a > b, a >= b)
}

pub fn ops_string(a: String, b: &str) -> (bool, bool, bool, bool) {
    (a.as_str() < b, a.as_str() <= b, b > a.as_str(), b >= a.as_str())
}

pub fn cmp_i8(a: i8, b: i8) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_u8(a: u8, b: u8) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_i16(a: i16, b: i16) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_u16(a: u16, b: u16) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_i32(a: i32, b: i32) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_u32(a: &u32, b: &u32) -> Ordering {
    a.cmp(b)
}

pub fn cmp_i64(a: i64, b: i64) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_u64(a: u64, b: u64) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_usize(a: usize, b: usize) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_char(a: char, b: char) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_bool(a: bool, b: bool) -> Ordering {
    a.cmp(&b)
}

pub fn cmp_uuid(a: Uuid, b: Uuid) -> core::cmp::Ordering {
    a.cmp(&b)
}

pub fn preds(o: Ordering) -> (bool, bool, bool) {
    (o.is_eq(), o.is_ne(), o.is_lt())
}

pub fn more_preds(o: Ordering) -> (bool, bool, bool) {
    (o.is_gt(), o.is_le(), o.is_ge())
}

pub fn reversed(o: std::cmp::Ordering) -> std::cmp::Ordering {
    o.reverse()
}

/// The receiver runs first, then the argument, each whatever the receiver
/// is: `a * 2` and `x + 1` can each overflow.
pub fn then_eager(a: i32, b: i32, x: i32) -> Ordering {
    (a * 2).cmp(&b).then((x + 1).cmp(&0))
}

/// `x + 1` runs only when `a == b`.
pub fn then_lazy(a: i32, b: i32, x: i32) -> Ordering {
    a.cmp(&b).then_with(|| (x + 1).cmp(&0))
}

fn tie() -> Ordering {
    core::cmp::Ordering::Greater
}

pub fn then_named(a: u8, b: u8) -> Ordering {
    a.cmp(&b).then_with(tie)
}

pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u64,
    pub pre: String,
}

/// A chain as SemVer precedence writes it.
pub fn compare(a: Version, b: Version) -> Ordering {
    a.major
        .cmp(&b.major)
        .then(a.minor.cmp(&b.minor))
        .then_with(|| a.patch.cmp(&b.patch))
        .then_with(|| match (a.pre.is_empty(), b.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => a.pre.cmp(&b.pre),
        })
}

pub fn classify(a: i32, b: i32, c: i32) -> u8 {
    match (a.cmp(&b), b.cmp(&c)) {
        (Ordering::Less, Ordering::Less) => 0,
        (Ordering::Equal, _) if a > 0 => 1,
        (Ordering::Equal, _) => 2,
        (_, std::cmp::Ordering::Greater) => 3,
        (Ordering::Greater, Ordering::Less | Ordering::Equal) => 4,
        (Ordering::Less, _) => 5,
    }
}

pub fn guarded(o: Ordering, n: u8) -> u8 {
    match o {
        Ordering::Less if n > 3 => 1,
        Ordering::Less | Ordering::Equal => 2,
        Ordering::Greater => 3,
    }
}

pub fn not_after(a: char, b: char) -> (bool, bool) {
    (
        matches!(a.cmp(&b), Ordering::Less | Ordering::Equal),
        matches!(b.cmp(&a), core::cmp::Ordering::Greater),
    )
}

pub fn same(a: Ordering, b: Ordering) -> (bool, bool, bool) {
    (a == b, a != b, a == Ordering::Less)
}

pub fn same_cmp(a: i64, b: i64, c: i64) -> bool {
    a.cmp(&b) == b.cmp(&c)
}

pub fn as_int(o: Ordering) -> (i32, i8) {
    (o as i32, o as i8)
}

/// Not std's `Less`: a crate enum's variant of that name, bare.
pub enum Size {
    Less(u8),
    More,
}

pub fn size_of(s: Size) -> u8 {
    match s {
        Less(n) => n,
        Size::More => 0,
    }
}

pub fn orderings(a: u32, b: u32) -> Vec<Ordering> {
    vec![a.cmp(&b), b.cmp(&a), a.cmp(&a)]
}
