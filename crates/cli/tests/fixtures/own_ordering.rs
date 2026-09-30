// A crate with its own `Ordering` that never names std's: it stays the
// crate's enum, with variants std's does not have.

pub enum Ordering {
    Before,
    Same,
    After,
}

pub fn order(a: i32, b: i32) -> Ordering {
    if a < b {
        Ordering::Before
    } else if a == b {
        Ordering::Same
    } else {
        Ordering::After
    }
}

pub fn flip(o: Ordering) -> Ordering {
    match o {
        Ordering::Before => Ordering::After,
        Ordering::Same => Ordering::Same,
        Ordering::After => Ordering::Before,
    }
}
