pub enum State {
    Idle,
    Running { ticks: u32 },
    Paused(u32),
    Done,
}

pub enum Event {
    Start,
    Tick,
    Pause,
    Resume,
    Stop,
    Reset,
}

pub enum Fault {
    Invalid,
    Finished,
}

/// `match (state, event)`: bindings on both sides, `_` and a binding as
/// elements, `|` inside an element and between whole tuples.
pub fn step(state: State, event: Event) -> Result<State, Fault> {
    match (state, event) {
        (_, Event::Reset) => Ok(State::Idle),
        (State::Idle, Event::Start) => Ok(State::Running { ticks: 0 }),
        (State::Running { ticks }, Event::Tick) => Ok(State::Running { ticks: ticks + 1 }),
        (State::Running { ticks }, Event::Pause) => Ok(State::Paused(ticks)),
        (State::Paused(t), Event::Resume) => Ok(State::Running { ticks: t }),
        (State::Running { .. } | State::Paused(_), Event::Stop) => Ok(State::Done),
        (State::Done, Event::Tick) | (State::Done, Event::Start) => Err(Fault::Finished),
        (s, _) => {
            if matches!(s, State::Done) {
                Err(Fault::Finished)
            } else {
                Err(Fault::Invalid)
            }
        }
    }
}

fn event_of(code: u8) -> Event {
    match code {
        0 => Event::Start,
        1 => Event::Tick,
        2 => Event::Pause,
        3 => Event::Resume,
        4 => Event::Stop,
        _ => Event::Reset,
    }
}

pub fn run4(a: u8, b: u8, c: u8, d: u8) -> Result<State, Fault> {
    let s = step(State::Idle, event_of(a))?;
    let s = step(s, event_of(b))?;
    let s = step(s, event_of(c))?;
    step(s, event_of(d))
}

/// `Option` and `Result` elements, with bindings.
pub fn merge(a: Option<i64>, b: Option<u8>, fail: bool) -> i64 {
    let r: Result<u8, String> = match b {
        Some(y) => Ok(y),
        None => Err(String::from(if fail { "x" } else { "" })),
    };
    match (a, r) {
        (Some(x), Ok(y)) => x + i64::from(y),
        (Some(x), Err(_)) => x,
        (None, Ok(y)) => i64::from(y),
        (None, Err(e)) => {
            if e.is_empty() {
                -1
            } else {
                -2
            }
        }
    }
}

/// Literal and range elements on `bigint`, `number`, `char`, and `&str`, in
/// expression position.
pub fn grid(x: i64, y: u8, c: char, s: &str) -> u32 {
    let a: u32 = match (x, y) {
        (0, 0) => 1,
        (1..=9, _) => 2,
        (-5..0, 1 | 2) => 3,
        (_, 200..=255) => 4,
        _ => 5,
    };
    a * 10
        + match (c, s) {
            ('a'..='z', "x") => 1,
            ('\u{e000}'..='\u{ffff}', _) => 2,
            (_, "") => 3,
            _ => 4,
        }
}

/// Elements that are not places are evaluated first, left to right: the
/// left overflow panics before the right one.
pub fn order(a: u8, b: u8) -> u8 {
    match (a + 200, b * 2) {
        (0..=100, _) => 1,
        (_, 0) => 2,
        _ => 3,
    }
}

fn split(n: u32) -> (u32, bool) {
    (n / 10, n % 2 == 0)
}

/// A tuple from a call; a last arm of bindings only.
pub fn digits(n: u32) -> u32 {
    match split(n) {
        (0, _) => 0,
        (q, even) => {
            if even {
                q
            } else {
                q + 100
            }
        }
    }
}

pub struct Player {
    pub hp: i32,
    pub state: State,
}

/// Fields as elements.
pub fn hurt(hp: i32, code: u8) -> i32 {
    let p = Player { hp, state: start(code) };
    match (p.state, p.hp) {
        (State::Done, _) => 0,
        (State::Running { ticks }, 0) => -1 - ticks_i32(ticks),
        (State::Idle, -3..=3) => 100,
        _ => p.hp,
    }
}

fn start(code: u8) -> State {
    match step(State::Idle, event_of(code)) {
        Ok(s) => s,
        Err(_) => State::Done,
    }
}

fn ticks_i32(t: u32) -> i32 {
    match t {
        0 => 0,
        _ => 1,
    }
}

/// A statement `match` on a tuple in a loop, and `matches!` with a tuple.
pub fn count(codes: Vec<u8>) -> u32 {
    let mut n = 0u32;
    for i in 0..codes.len() {
        match (event_of(codes[i]), i % 2) {
            (Event::Tick, 0) => n += 1u32,
            (Event::Tick, _) => n += 2u32,
            (Event::Stop | Event::Reset, _) => n += 100u32,
            _ => {}
        }
        if matches!((event_of(codes[i]), i), (Event::Start, 0)) {
            n += 1000u32;
        }
    }
    n
}
