/// How a run of events ends: the configuration refused, an event refused
/// (its index, and the DISCONNECT code the client sends), or the last state
/// with every action in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Refused(ConfigError),
    Failed { index: usize, failure: Failure, code: u32 },
    Ran { connection: Connection, actions: Vec<Action> },
}

/// `start`, then each event of `events` until one is refused.
pub fn connect(config: Config, events: Vec<Event>) -> Outcome {
    match start(config) {
        Ok((c, first)) => play(c, first, events),
        Err(e) => Outcome::Refused(e),
    }
}

fn play(opened: Connection, first: Vec<Action>, events: Vec<Event>) -> Outcome {
    let mut c = opened;
    let mut actions: Vec<Action> = Vec::new();
    for a in first.iter() {
        actions.push(a.clone());
    }
    for (index, e) in events.iter().enumerate() {
        match step(c.clone(), e.clone()) {
            Ok((next, out)) => {
                c = next;
                for a in out.iter() {
                    actions.push(a.clone());
                }
            }
            Err(failure) => {
                let code = reason_code(reason_for(&failure));
                return Outcome::Failed { index, failure, code };
            }
        }
    }
    Outcome::Ran { connection: c, actions }
}
