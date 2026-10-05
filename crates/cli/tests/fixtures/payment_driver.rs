pub fn terms_of(code: u8) -> Terms {
    let capture = if code % 2u8 == 0u8 { CaptureMethod::Automatic } else { CaptureMethod::Manual };
    let confirmation = if code / 2u8 == 0u8 { ConfirmationMethod::Automatic } else { ConfirmationMethod::Manual };
    Terms { amount: Amount(2000i64), capture, confirmation }
}

pub fn card() -> PaymentMethod {
    PaymentMethod { id: PaymentMethodId(String::from("pm_card")), kind: MethodKind::Card }
}

pub fn bank() -> PaymentMethod {
    PaymentMethod { id: PaymentMethodId(String::from("pm_bank")), kind: MethodKind::BankDebit }
}

pub fn decode(code: u8) -> Event {
    if code == 0u8 {
        Event::AttachMethod(card())
    } else if code == 1u8 {
        Event::AttachMethod(bank())
    } else if code == 2u8 {
        Event::Confirm { method: None, outcome: Outcome::Authorized }
    } else if code == 3u8 {
        Event::Confirm { method: Some(card()), outcome: Outcome::ActionRequired }
    } else if code == 4u8 {
        Event::Confirm { method: Some(bank()), outcome: Outcome::Pending }
    } else if code == 5u8 {
        Event::Confirm { method: None, outcome: Outcome::Declined(DeclineCode::CardDeclined) }
    } else if code == 6u8 {
        Event::ActionHandled(Outcome::Authorized)
    } else if code == 7u8 {
        Event::ActionHandled(Outcome::Declined(DeclineCode::AuthenticationFailed))
    } else if code == 8u8 {
        Event::ProcessingSucceeded
    } else if code == 9u8 {
        Event::ProcessingFailed(DeclineCode::DebitFailed)
    } else if code == 10u8 {
        Event::Capture { amount_to_capture: Some(1500i64), application_fee: Some(5000i64) }
    } else {
        Event::Cancel(Some(CancellationReason::RequestedByCustomer))
    }
}

pub fn trace4(t: u8, a: u8, b: u8, c: u8, d: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = create(terms_of(t));
    let intent = step(intent, decode(a))?;
    let intent = step(intent, decode(b))?;
    let intent = step(intent, decode(c))?;
    step(intent, decode(d))
}

fn option_i64(code: u8) -> Option<i64> {
    if code == 0u8 {
        None
    } else if code == 1u8 {
        Some(-1i64)
    } else if code == 2u8 {
        Some(0i64)
    } else if code == 3u8 {
        Some(1i64)
    } else if code == 4u8 {
        Some(100i64)
    } else if code == 5u8 {
        Some(2000i64)
    } else {
        Some(2001i64)
    }
}

/// Capture from `requires_capture` with every pairing of boundary amounts
/// and fees.
pub fn capture(amount: u8, fee: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = create(terms_of(1u8));
    let intent = step(intent, Event::Confirm { method: Some(card()), outcome: Outcome::Authorized })?;
    step(intent, Event::Capture { amount_to_capture: option_i64(amount), application_fee: option_i64(fee) })
}

fn outcome_of(code: u8) -> Outcome {
    if code == 0u8 {
        Outcome::Authorized
    } else if code == 1u8 {
        Outcome::ActionRequired
    } else if code == 2u8 {
        Outcome::Pending
    } else {
        Outcome::Declined(DeclineCode::InsufficientFunds)
    }
}

/// Each outcome of a customer action, under both confirmation methods: a
/// bank debit under automatic capture, a card under manual.
pub fn action(t: u8, o: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = create(terms_of(t));
    let method = if t % 2u8 == 0u8 { bank() } else { card() };
    let intent = step(intent, Event::Confirm { method: Some(method), outcome: Outcome::ActionRequired })?;
    step(intent, Event::ActionHandled(outcome_of(o)))
}

fn reason(code: u8) -> Option<CancellationReason> {
    if code == 0u8 {
        None
    } else if code == 1u8 {
        Some(CancellationReason::Duplicate)
    } else if code == 2u8 {
        Some(CancellationReason::Fraudulent)
    } else if code == 3u8 {
        Some(CancellationReason::RequestedByCustomer)
    } else {
        Some(CancellationReason::Abandoned)
    }
}

/// Every cancellation reason, and cancelling twice.
pub fn cancel(code: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = step(create(terms_of(0u8)), Event::Cancel(reason(code)))?;
    step(intent, Event::Cancel(reason(code)))
}

pub fn amount_of(value: i64) -> Result<Amount, PaymentError> {
    Amount::new(value)
}

pub fn method_id(raw: String) -> Result<PaymentMethodId, PaymentError> {
    PaymentMethodId::new(raw)
}

fn status_of(code: u8) -> Status {
    if code == 0u8 {
        Status::RequiresCapture { method: card(), capturable: 2000i64 }
    } else if code == 1u8 {
        Status::RequiresCapture { method: card(), capturable: 1i64 }
    } else if code == 2u8 {
        Status::RequiresCapture { method: card(), capturable: 0i64 }
    } else if code == 3u8 {
        Status::RequiresCapture { method: card(), capturable: 2001i64 }
    } else if code == 4u8 {
        Status::RequiresCapture { method: bank(), capturable: -1i64 }
    } else if code == 5u8 {
        Status::Succeeded { received: 2000i64, application_fee: None }
    } else if code == 6u8 {
        Status::Succeeded { received: 1500i64, application_fee: None }
    } else if code == 7u8 {
        Status::Succeeded { received: 0i64, application_fee: None }
    } else if code == 8u8 {
        Status::Succeeded { received: 2001i64, application_fee: None }
    } else if code == 9u8 {
        Status::Succeeded { received: 1500i64, application_fee: Some(-1i64) }
    } else if code == 10u8 {
        Status::Succeeded { received: 1500i64, application_fee: Some(0i64) }
    } else if code == 11u8 {
        Status::Succeeded { received: 1500i64, application_fee: Some(1500i64) }
    } else if code == 12u8 {
        Status::Succeeded { received: 1500i64, application_fee: Some(1501i64) }
    } else if code == 13u8 {
        Status::Succeeded { received: 2000i64, application_fee: Some(100i64) }
    } else if code == 14u8 {
        Status::Succeeded { received: 1i64, application_fee: Some(1i64) }
    } else if code == 15u8 {
        Status::RequiresPaymentMethod { last_error: Some(DeclineCode::CardDeclined) }
    } else if code == 16u8 {
        Status::Processing { method: bank() }
    } else if code == 17u8 {
        Status::RequiresConfirmation { method: card() }
    } else if code == 18u8 {
        Status::Canceled { reason: None }
    } else if code == 19u8 {
        Status::RequiresCapture { method: bank(), capturable: 2000i64 }
    } else if code == 20u8 {
        Status::RequiresConfirmation { method: bank() }
    } else {
        Status::RequiresAction { method: bank() }
    }
}

/// An intent as a client might send it: terms `t` (as `terms_of`) with
/// status `s`, consistent or not.
pub fn unchecked(t: u8, s: u8) -> UncheckedIntent {
    UncheckedIntent { terms: terms_of(t), status: status_of(s) }
}

pub fn read_intent(t: u8, s: u8) -> Result<PaymentIntent, PaymentError> {
    PaymentIntent::new(unchecked(t, s))
}

/// `trace4`'s result put through the check again: every state `step`
/// reaches is one the check accepts.
pub fn trace4_rechecked(t: u8, a: u8, b: u8, c: u8, d: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = trace4(t, a, b, c, d)?;
    PaymentIntent::new(UncheckedIntent { terms: intent.terms, status: intent.status })
}

/// A checked intent, then one event on it.
pub fn read_and_step(t: u8, s: u8, e: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = PaymentIntent::new(unchecked(t, s))?;
    step(intent, decode(e))
}

/// A confirmation with method `m` (a card, a bank debit, or none) and
/// outcome `o` (as `outcome_of`) on a fresh intent with terms `t`, after a
/// card (`a` = 0), a bank debit (1), or nothing (2) was attached.
pub fn confirm_with(t: u8, a: u8, m: u8, o: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = create(terms_of(t));
    let intent = if a == 0u8 {
        step(intent, Event::AttachMethod(card()))?
    } else if a == 1u8 {
        step(intent, Event::AttachMethod(bank()))?
    } else {
        intent
    };
    let method = if m == 0u8 {
        Some(card())
    } else if m == 1u8 {
        Some(bank())
    } else {
        None
    };
    step(intent, Event::Confirm { method, outcome: outcome_of(o) })
}

/// Stripe's capture example scaled: a manual intent of `amount`,
/// authorized by card, then captured `to_capture`.
pub fn capture_of(amount: i64, to_capture: i64) -> Result<PaymentIntent, PaymentError> {
    let terms = Terms { amount: Amount::new(amount)?, capture: CaptureMethod::Manual, confirmation: ConfirmationMethod::Automatic };
    let intent = step(create(terms), Event::Confirm { method: Some(card()), outcome: Outcome::Authorized })?;
    step(intent, Event::Capture { amount_to_capture: Some(to_capture), application_fee: None })
}
