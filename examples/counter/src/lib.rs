// A counter: `Inc` adds one, `Dec` subtracts one, and `Reset` sets it to
// zero, whatever it held.
//
// The count is an `i32`, and `step` uses the plain `+` and `-`: at
// `i32::MAX` an `Inc`, and at `i32::MIN` a `Dec`, panics with "attempt to
// add with overflow" (or "subtract"), as a Rust debug build does, and the
// generated TS throws `Panic` with the same message. A release build wraps
// instead (`i32::MAX + 1` is `i32::MIN`) unless its profile sets
// `overflow-checks = true`, so a server built with `--release` agrees with
// the TS only with that setting (design/01-equivalence.md §1, and §3's
// "Release wrapping"). Every other count moves by one, in both.

pub enum Event {
    Inc,
    Dec,
    Reset,
}

pub struct State {
    pub n: i32,
}

pub fn step(state: State, event: Event) -> State {
    match event {
        Event::Inc => State { n: state.n + 1 },
        Event::Dec => State { n: state.n - 1 },
        Event::Reset => State { n: 0 },
    }
}
