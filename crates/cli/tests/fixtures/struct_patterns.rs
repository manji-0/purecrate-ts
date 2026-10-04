// Struct patterns inside a `match` arm and `matches!`: a refutable field
// becomes a test before the arm's guard, a bound field a `let`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Card,
    Bank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Method {
    pub kind: Kind,
    pub id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Owner {
    pub method: Method,
    pub age: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Processing { method: Method },
    Held { owner: Owner },
    Done,
}

pub fn cancelable(s: Status) -> bool {
    match s {
        Status::Processing { method: Method { kind: Kind::Bank, .. } } => true,
        _ => false,
    }
}

/// A guard reads a bound field; a later arm tests another.
pub fn picked(s: Status) -> u32 {
    match s {
        Status::Processing { method: Method { id, .. } } if id > 3 => id,
        Status::Processing { method: Method { kind: Kind::Card, id } } => id + 100,
        Status::Held { owner: Owner { method: Method { kind: Kind::Bank, id }, age: 18..=99 } } => id + 1000,
        Status::Held { owner: Owner { age, .. } } => u32::from(age),
        _ => 0,
    }
}

pub fn is_card(s: &Status) -> bool {
    matches!(s, Status::Processing { method: Method { kind: Kind::Card, .. } })
}

pub fn step(s: Status, go: bool) -> u32 {
    match (s, go) {
        (Status::Processing { method: Method { kind: Kind::Bank, id } }, true) => id,
        (Status::Done, _) => 1,
        _ => 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bag {
    pub a: u32,
    pub xs: Vec<u32>,
}

/// A guard whose closure and inner `match` bind the field's name again:
/// there it is theirs, not the field.
pub fn shadowed(bag: Bag, b: u32) -> u32 {
    match bag {
        Bag { a, xs } if xs.iter().any(|a| *a > 3) => a,
        Bag { a, .. } if match b { a if a > 10 => true, _ => false } => a + 100,
        _ => 0,
    }
}
