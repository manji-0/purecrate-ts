mod shapes;
mod util;

pub mod inline {
    pub fn twice(n: i32) -> i32 {
        n * 2
    }
}

pub fn area_of(s: shapes::Shape) -> i32 {
    shapes::area(s)
}

pub fn lookup(t: Table) -> i32 {
    0
}

pub struct Table {
    pub rows: HashMap<String, i32>,
}

/// Read through `impl TryFrom`, spelled with `Self` as idiomatic code does.
#[derive(serde::Deserialize)]
#[serde(try_from = "u8")]
pub struct Percent(u8);

pub enum PercentError {
    Over,
}

impl TryFrom<u8> for Percent {
    type Error = PercentError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value > 100 {
            return Err(PercentError::Over);
        }
        Ok(Self(value))
    }
}

pub fn abs_of(n: i32) -> i32 {
    n.abs()
}

/// Four causes: two the parser meets (`format!`, `loop`) and two the type
/// check meets (`trim`, `as` on an integer).
pub fn many(n: i32, s: &str) -> i64 {
    let text = format!("{n}");
    let k = s.trim().len();
    loop {
        break;
    }
    n as i64
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {}
}
