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
