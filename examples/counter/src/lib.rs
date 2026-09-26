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
