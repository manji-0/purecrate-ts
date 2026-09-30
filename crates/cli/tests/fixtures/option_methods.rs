// `Option::unwrap_or`, `ok_or`, and `map`: eager arguments (evaluated, and
// overflowing, even on `Some`), `map` calling its closure only on `Some`,
// function names, chains, and falsy payloads.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Amount,
}

pub fn or_default(x: Option<u8>, d: u8) -> u8 {
    x.unwrap_or(d)
}

/// `d + 1` is evaluated whether or not `x` is `Some`.
pub fn eager(x: Option<u8>, d: u8) -> u8 {
    x.unwrap_or(d + 1)
}

pub fn required(x: Option<i64>) -> Result<i64, Missing> {
    x.ok_or(Missing::Amount)
}

/// `n * 2` runs only on `Some`.
pub fn doubled(x: Option<i32>, n: i32) -> Option<i32> {
    x.map(|v| v * 2 + n * 0)
}

fn halve(v: u32) -> u32 {
    v / 2
}

pub fn chain(x: Option<u32>) -> u32 {
    x.map(halve).map(|v| v + 1).unwrap_or(0)
}

pub fn total(a: Option<i64>, b: Option<i64>) -> Result<i64, Missing> {
    let a = a.ok_or(Missing::Amount)?;
    let b = b.unwrap_or(0);
    Ok(a + b)
}

pub fn falsy(x: Option<bool>) -> bool {
    x.map(|b| !b).unwrap_or(false)
}
