pub enum Event {
    Inc,
    Dec,
}

pub struct Step {
    pub n: i32,
}

pub fn step(s: Step, event: Event) -> Step {
    match event {
        Event::Inc => Step { n: s.n + 1 },
    }
}
