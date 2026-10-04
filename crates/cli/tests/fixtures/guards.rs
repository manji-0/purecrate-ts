// Match guards: arms tried in order, a guard run only when its pattern
// matched, bindings seen by the guard, tuple scrutinees evaluated once,
// a binding arm `n if ..`, and `matches!` with a guard.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rate {
    Standard,
    Reduced,
    Exempt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Pay { amount: i64 },
    Refund { amount: i64 },
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Open,
    Paid,
    Closed,
}

/// A guard over the bound field, then the unguarded arm for the same
/// variant, then the others.
pub fn tax(rate: Rate, amount: i64) -> i64 {
    match rate {
        Rate::Standard if amount >= 10_000 => amount / 10,
        Rate::Standard => amount * 10 / 100,
        Rate::Reduced if amount <= 0 => 0,
        Rate::Reduced => amount * 8 / 100,
        Rate::Exempt => 0,
    }
}

/// Guards on a tuple match, as the idiomatic payment writes transitions.
pub fn step(state: State, event: Event) -> Result<State, i64> {
    match (state, event) {
        (State::Open, Event::Pay { amount }) if amount > 0 => Ok(State::Paid),
        (State::Open, Event::Pay { amount }) => Err(amount),
        (State::Paid, Event::Refund { amount }) if amount > 1_000_000 => Err(amount),
        (State::Paid, Event::Refund { .. }) => Ok(State::Open),
        (_, Event::Cancel) => Ok(State::Closed),
        (_, _) => Err(-1),
    }
}

/// Binding arms with guards on an integer, and overflow inside a guard.
pub fn bucket(n: i32) -> i32 {
    match n {
        0 => 0,
        x if x < 0 => -1,
        x if x * 2 > 100 => 2,
        _ => 1,
    }
}

/// The guard counts how often it runs: only when its pattern matched.
pub fn guard_runs(xs: Vec<Option<u8>>) -> u32 {
    let mut runs = 0u32;
    let mut hits = 0u32;
    for x in &xs {
        runs += 1;
        match x {
            Some(v) if *v > 9 => hits += 100,
            Some(_) => hits += 1,
            None => {}
        }
    }
    runs * 1000 + hits
}

pub fn big_even(x: Option<u32>) -> bool {
    matches!(x, Some(v) if v % 2 == 0 && v > 10)
}

/// The scrutinee is a call: it runs once, whichever arm is taken.
pub fn classify(s: &str) -> u8 {
    match s.len() {
        n if n > 8 => 3,
        0 => 0,
        n if n % 2 == 0 => 2,
        _ => 1,
    }
}

/// The guard overflows for a large `amount`, but only runs for `Exempt`.
pub fn only_when_matched(rate: Rate, amount: i32) -> i32 {
    match rate {
        Rate::Exempt if amount * 1000 > 0 => 1,
        _ => 0,
    }
}

/// A guarded arm covers nothing: `_` after it takes the same variant.
pub fn wild_after(rate: Rate, amount: i64) -> i64 {
    match rate {
        Rate::Standard if amount > 5 => 1,
        _ => 2,
    }
}

/// A range and a literal inside it, each guarded: the literal's arm is
/// reached when the range's guard fails.
pub fn overlap(n: u8, strict: bool) -> u8 {
    match n {
        0..=9 if strict => 1,
        5 if !strict => 2,
        0..=9 => 3,
        _ => 4,
    }
}

/// A guard that overflows, run only where its tuple pattern matched.
pub fn guarded_overflow(state: State, n: u8) -> u8 {
    match (state, n) {
        (State::Paid, m) if m + 200 > 250 => 1,
        (State::Open, _) => 2,
        (_, m) => m,
    }
}

/// `bool` elements with guards, both values named.
pub fn flags(a: bool, b: bool, n: u32) -> u32 {
    match (a, b) {
        (true, true) if n > 3 => 1,
        (true, _) => 2,
        (false, true) if n == 0 => 3,
        (false, _) => 4,
    }
}

/// A bound name used only in the guard, and a guard on `Option`.
pub fn only_in_guard(x: Option<u32>) -> u32 {
    match x {
        Some(v) if v > 7 => 1,
        Some(_) => 2,
        None => 3,
    }
}

/// Two rows that bind one name to different fields, or a row whose name
/// another row declares later: each reads its own field.
#[derive(Debug, Clone, Copy)]
pub enum Pair {
    P(i32, i32),
    Q,
}

pub fn pick(p: Pair, first: bool) -> i32 {
    match p {
        Pair::P(x, _) if first => x,
        Pair::P(_, x) => x,
        Pair::Q => 0,
    }
}

pub fn shadowed_later(p: Pair, c: bool) -> i32 {
    match p {
        Pair::P(x, _) if c => x,
        Pair::P(y, _) => {
            let x = 1i32;
            y - x
        }
        Pair::Q => 0,
    }
}

pub enum Setting {
    Set(Option<i32>),
}

/// A guarded arm testing inside a field, then an arm binding the whole
/// field under the same name the tree gives it (`let v = v`): the printer
/// followed that alias forever.
pub fn guarded_field(e: Setting, b: i32) -> i32 {
    match e {
        Setting::Set(Some(_)) if b > 0 => 1,
        Setting::Set(v) => v.unwrap_or(b),
    }
}
