/// Events to play in order. The subset cannot index a `Vec` by value, so a
/// run's events are a list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Script {
    End,
    Then(Event, Box<Script>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunError {
    Refused(AuthorizationError),
    Step { index: u32, error: FlowError },
}

fn play(flow: Flow, script: Script, index: u32, policy: &Policy) -> Result<Flow, RunError> {
    match script {
        Script::End => Ok(flow),
        Script::Then(event, rest) => match step(flow, event, policy) {
            Ok(next) => play(next, *rest, index + 1, policy),
            Err(error) => Err(RunError::Step { index, error }),
        },
    }
}

/// `begin`, then each event of `script` until one is refused.
pub fn trace(
    params: &AuthorizationParams,
    client: &Option<Client>,
    session: &Option<Session>,
    consent_on_file: bool,
    now: i64,
    policy: &Policy,
    script: Script,
) -> Result<Flow, RunError> {
    let flow = match begin(params, client, session, consent_on_file, now) {
        Ok(f) => f,
        Err(e) => return Err(RunError::Refused(e)),
    };
    play(flow, script, 0, policy)
}
