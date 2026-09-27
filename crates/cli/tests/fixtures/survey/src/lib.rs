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

pub fn abs_of(n: i32) -> i32 {
    n.abs()
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {}
}
