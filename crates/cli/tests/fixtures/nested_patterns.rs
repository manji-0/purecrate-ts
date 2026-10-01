// Patterns inside patterns: a literal, a range, a variant, or `Some`/`Ok`/
// `Err` in a variant's field or a payload, to any depth, with bindings
// beside them, guards after them, a last `_`, in a tuple `match`, and in
// `matches!`. Arms are tried in order: an earlier arm that covers part of a
// later one wins.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Card,
    Bank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Login {
    Started,
    PasswordChecked { verified: bool, attempts: u8 },
    Paid(Method, Option<u32>),
}

/// oidc's shape: a `bool` literal and `..` in a struct variant.
pub fn next(login: Login) -> u32 {
    match login {
        Login::PasswordChecked { verified: false, attempts: 0 } => 10,
        Login::PasswordChecked { verified: false, attempts } => 20 + u32::from(attempts),
        Login::PasswordChecked { verified: true, .. } => 30,
        Login::Started => 40,
        Login::Paid(Method::Card, Some(0)) => 50,
        Login::Paid(Method::Card, Some(n)) => n,
        Login::Paid(_, None) => 60,
        Login::Paid(Method::Bank, Some(1..=9)) => 70,
        Login::Paid(Method::Bank, Some(_)) => 80,
    }
}

/// A last `_` takes what the nested arms leave, including part of a case
/// they name.
pub fn card_amount(login: Login) -> u32 {
    match login {
        Login::Paid(Method::Card, Some(n)) => n,
        _ => 0,
    }
}

/// A guard after a nested pattern: false goes on to the next arm.
pub fn guarded(login: &Login) -> u32 {
    match login {
        Login::PasswordChecked { verified: true, attempts } if *attempts > 3 => 1,
        Login::PasswordChecked { verified: true, .. } => 2,
        Login::Paid(Method::Bank, Some(n)) if *n > 100 => 3,
        _ => 4,
    }
}

/// `Option` and `Result` nest in each other, and an enum in both.
pub fn deep(r: Result<Option<Method>, u8>) -> u32 {
    match r {
        Ok(Some(Method::Card)) => 1,
        Ok(Some(Method::Bank)) => 2,
        Ok(None) => 3,
        Err(0) => 4,
        Err(e) => 100 + u32::from(e),
    }
}

/// `Some` of a literal, of a range, and `|` inside.
pub fn small(x: Option<i64>) -> i64 {
    match x {
        Some(0) => 0,
        Some(1 | 2) => 1,
        Some(-5..=-1) => -1,
        Some(n) => n * 2,
        None => -100,
    }
}

/// A `char` and a string inside a case.
pub fn text(c: Option<char>, s: Option<&str>) -> u32 {
    let a: u32 = match c {
        Some('a'..='z') => 1,
        Some(_) => 2,
        None => 3,
    };
    let b: u32 = match s {
        Some("yes") => 10,
        _ => 20,
    };
    a + b
}

/// A tuple `match` whose elements nest.
pub fn pair(a: Option<Method>, b: Login) -> u32 {
    match (a, b) {
        (Some(Method::Card), Login::PasswordChecked { verified: true, .. }) => 1,
        (Some(_), Login::PasswordChecked { verified: false, attempts: 5 }) => 2,
        (None, Login::Started) => 3,
        _ => 4,
    }
}

/// `matches!` with a nested pattern, and with a guard on it.
pub fn is_verified(login: &Login) -> bool {
    matches!(login, Login::PasswordChecked { verified: true, .. })
}

pub fn is_big_card(login: &Login) -> bool {
    matches!(login, Login::Paid(Method::Card, Some(n)) if *n >= 1000)
}

fn paid(card: bool, amount: u32) -> Login {
    if card {
        Login::Paid(Method::Card, Some(amount))
    } else {
        Login::Paid(Method::Bank, None)
    }
}

/// A value that is not a place is evaluated once, then tested inside.
pub fn free_card(card: bool, amount: u32) -> bool {
    matches!(paid(card, amount), Login::Paid(Method::Card, Some(0)))
}

pub fn fee(card: bool, amount: u32) -> u32 {
    match paid(card, amount) {
        Login::Paid(Method::Card, Some(0)) => 0,
        Login::Paid(Method::Card, Some(n)) => n / 100,
        _ => 50,
    }
}

/// A side of `|` that tests inside its variant: the sides are tried in
/// order, each with the same body, and a later arm takes the rest of the
/// case.
pub fn either(login: Login) -> u32 {
    match login {
        Login::Started | Login::Paid(Method::Card, Some(0)) => 1,
        Login::PasswordChecked { verified: true, .. } | Login::Paid(_, None) => 2,
        Login::Paid(Method::Card, Some(n)) => n,
        _ => 3,
    }
}

/// `|` inside a payload, with a guard on the arm.
pub fn either_guarded(login: &Login) -> u32 {
    match login {
        Login::Paid(Method::Card | Method::Bank, Some(1 | 2)) => 1,
        Login::PasswordChecked { verified: false, attempts: 0 } | Login::Started if true => 2,
        _ => 3,
    }
}

/// A tuple inside a payload with a literal beside a name, and a tuple
/// inside a tuple.
pub fn split(s: Option<(u32, u32)>) -> u32 {
    match s {
        Some((0, b)) => b,
        Some((a, 0)) => a * 2,
        Some((a, b)) => a + b,
        None => 7,
    }
}

pub fn quad(x: (u32, (bool, u32))) -> u32 {
    match x {
        (0, (true, n)) => n,
        (a, (_, 2) | (false, _)) => a + 100,
        (_, (_, m)) => m + 1000,
    }
}
