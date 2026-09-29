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

/// Each outcome of a customer action, under both confirmation methods.
pub fn action(t: u8, o: u8) -> Result<PaymentIntent, PaymentError> {
    let intent = create(terms_of(t));
    let intent = step(intent, Event::Confirm { method: Some(bank()), outcome: Outcome::ActionRequired })?;
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
