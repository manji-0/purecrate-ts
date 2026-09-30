//! `examples/payment`, Stripe's PaymentIntent lifecycle written inside the
//! constraints of design/02, agrees with the generated package on every
//! four-event run under each capture and confirmation method, compared as
//! the whole final `PaymentIntent`. It also agrees with the same rules in
//! idiomatic Rust (`idiomatic`, the line count design/07 §2 compares
//! against) on the same runs.

use crate::support;


purecrate_canon::fixture!(mod payment = "../../../examples/payment/src/lib.rs", "fixtures/payment_driver.rs");

const SOURCE: &str = payment::SOURCE;

/// The same lifecycle as one would write it without the subset's
/// constraints: a tuple `match` with a wildcard, `Option` combinators, and
/// `min`. Not converted; the reference only.
mod idiomatic {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Kind { Card, BankDebit }
    #[derive(Clone, Debug, PartialEq)]
    pub struct Method { pub id: String, pub kind: Kind }
    #[derive(Clone, Copy, PartialEq)]
    pub enum Capture { Automatic, Manual }
    #[derive(Clone, Copy, PartialEq)]
    pub enum Confirmation { Automatic, Manual }
    pub struct Terms { pub amount: i64, pub capture: Capture, pub confirmation: Confirmation }
    // Stripe's full lists; the runs reach only some of them.
    #[allow(dead_code)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Decline { CardDeclined, InsufficientFunds, AuthenticationFailed, DebitFailed }
    #[allow(dead_code)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Reason { Duplicate, Fraudulent, RequestedByCustomer, Abandoned }

    #[derive(Debug, PartialEq)]
    pub enum Status {
        RequiresPaymentMethod { last_error: Option<Decline> },
        RequiresConfirmation { method: Method },
        RequiresAction { method: Method },
        Processing { method: Method },
        RequiresCapture { method: Method, capturable: i64 },
        Succeeded { received: i64, application_fee: Option<i64> },
        Canceled { reason: Option<Reason> },
    }

    pub enum Outcome { Authorized, ActionRequired, Pending, Declined(Decline) }

    pub enum Event {
        AttachMethod(Method),
        Confirm { method: Option<Method>, outcome: Outcome },
        ActionHandled(Outcome),
        ProcessingSucceeded,
        ProcessingFailed(Decline),
        Capture { amount_to_capture: Option<i64>, application_fee: Option<i64> },
        Cancel(Option<Reason>),
    }

    #[derive(Debug, PartialEq)]
    pub enum Error { MissingPaymentMethod, InvalidCaptureAmount { capturable: i64 }, NegativeApplicationFee, NotCancelable, InvalidTransition }

    fn attempt(t: &Terms, method: Method, outcome: Outcome) -> Status {
        match outcome {
            Outcome::Authorized if t.capture == Capture::Manual => Status::RequiresCapture { method, capturable: t.amount },
            Outcome::Authorized => Status::Succeeded { received: t.amount, application_fee: None },
            Outcome::ActionRequired => Status::RequiresAction { method },
            Outcome::Pending => Status::Processing { method },
            Outcome::Declined(code) => Status::RequiresPaymentMethod { last_error: Some(code) },
        }
    }

    pub fn step(t: &Terms, status: Status, event: Event) -> Result<Status, Error> {
        use Status::*;
        Ok(match (status, event) {
            (RequiresPaymentMethod { .. } | RequiresConfirmation { .. }, Event::AttachMethod(method)) => RequiresConfirmation { method },
            (RequiresPaymentMethod { .. }, Event::Confirm { method, outcome }) => attempt(t, method.ok_or(Error::MissingPaymentMethod)?, outcome),
            (RequiresConfirmation { method: current }, Event::Confirm { method, outcome }) => attempt(t, method.unwrap_or(current), outcome),
            (RequiresAction { .. }, Event::ActionHandled(Outcome::Declined(code))) => RequiresPaymentMethod { last_error: Some(code) },
            (RequiresAction { method }, Event::ActionHandled(_)) if t.confirmation == Confirmation::Manual => RequiresConfirmation { method },
            (RequiresAction { method }, Event::ActionHandled(outcome)) => attempt(t, method, outcome),
            (Processing { method }, Event::ProcessingSucceeded) => attempt(t, method, Outcome::Authorized),
            (Processing { .. }, Event::ProcessingFailed(code)) => RequiresPaymentMethod { last_error: Some(code) },
            (RequiresCapture { capturable, .. }, Event::Capture { amount_to_capture, application_fee }) => {
                let received = amount_to_capture.unwrap_or(capturable);
                if !(1..=capturable).contains(&received) {
                    return Err(Error::InvalidCaptureAmount { capturable });
                }
                if application_fee.is_some_and(|f| f < 0) {
                    return Err(Error::NegativeApplicationFee);
                }
                Succeeded { received, application_fee: application_fee.map(|f| f.min(received)) }
            }
            (Processing { method }, Event::Cancel(reason)) if method.kind == Kind::BankDebit => Canceled { reason },
            (Processing { .. } | Succeeded { .. } | Canceled { .. }, Event::Cancel(_)) => return Err(Error::NotCancelable),
            (_, Event::Cancel(reason)) => Canceled { reason },
            _ => return Err(Error::InvalidTransition),
        })
    }
}

/// The driver's events, rebuilt for the idiomatic reference.
fn idiomatic_event(code: u8) -> idiomatic::Event {
    use idiomatic::*;
    let card = || Method { id: "pm_card".into(), kind: Kind::Card };
    let bank = || Method { id: "pm_bank".into(), kind: Kind::BankDebit };
    match code {
        0 => Event::AttachMethod(card()),
        1 => Event::AttachMethod(bank()),
        2 => Event::Confirm { method: None, outcome: Outcome::Authorized },
        3 => Event::Confirm { method: Some(card()), outcome: Outcome::ActionRequired },
        4 => Event::Confirm { method: Some(bank()), outcome: Outcome::Pending },
        5 => Event::Confirm { method: None, outcome: Outcome::Declined(Decline::CardDeclined) },
        6 => Event::ActionHandled(Outcome::Authorized),
        7 => Event::ActionHandled(Outcome::Declined(Decline::AuthenticationFailed)),
        8 => Event::ProcessingSucceeded,
        9 => Event::ProcessingFailed(Decline::DebitFailed),
        10 => Event::Capture { amount_to_capture: Some(1500), application_fee: Some(5000) },
        _ => Event::Cancel(Some(Reason::RequestedByCustomer)),
    }
}

fn idiomatic_trace(t: u8, codes: [u8; 4]) -> Result<idiomatic::Status, idiomatic::Error> {
    use idiomatic::*;
    let terms = Terms {
        amount: 2000,
        capture: if t.is_multiple_of(2) { Capture::Automatic } else { Capture::Manual },
        confirmation: if t / 2 == 0 { Confirmation::Automatic } else { Confirmation::Manual },
    };
    codes.iter().try_fold(Status::RequiresPaymentMethod { last_error: None }, |s, &c| step(&terms, s, idiomatic_event(c)))
}

/// Both sides as one text: variant names and numbers, through `Debug`.
fn same(a: &Result<payment::PaymentIntent, payment::PaymentError>, b: &Result<idiomatic::Status, idiomatic::Error>) -> bool {
    use crate::support::Show;
    let a = match a {
        Ok(intent) => format!("Ok({})", intent.status.show()),
        Err(e) => format!("Err({})", e.show()),
    };
    let b = format!("{b:?}")
        .replace("Status::", "")
        .replace("Error::", "")
        .replace("Decline::", "")
        .replace("Reason::", "");
    let a = a
        .replace("Status::", "")
        .replace("PaymentError::", "")
        .replace("DeclineCode::", "")
        .replace("CancellationReason::", "")
        .replace("PaymentMethodId(", "")
        .replace("\"), kind", "\", kind")
        .replace(": PaymentMethod {", ": Method {")
        .replace("MethodKind::", "");
    a == b
}

const CODES: std::ops::Range<u8> = 0..12;

fn runs() -> impl Iterator<Item = (u8, [u8; 4])> {
    (0u8..4).flat_map(|t| {
        CODES.flat_map(move |a| {
            CODES.flat_map(move |b| CODES.flat_map(move |c| CODES.map(move |d| (t, [a, b, c, d]))))
        })
    })
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let differ: Vec<String> = support::quietly(|| {
        runs()
            .filter_map(|(t, [a, b, c, d])| {
                let constrained = payment::trace4(t, a, b, c, d);
                let reference = idiomatic_trace(t, [a, b, c, d]);
                (!same(&constrained, &reference)).then(|| format!("{t} {a} {b} {c} {d}: idiomatic {reference:?} / constrained {}", support::Show::show(&constrained)))
            })
            .collect()
    });
    assert!(differ.is_empty(), "{} runs differ, first:\n{}", differ.len(), differ[..differ.len().min(10)].join("\n"));
}

#[test]
fn generated_payment_lifecycle_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases: Vec<_> = runs().map(|(t, [a, b, c, d])| case!(payment::trace4(t, a, b, c, d))).collect();
        for amount in 0u8..7 {
            for fee in 0u8..7 {
                cases.push(case!(payment::capture(amount, fee)));
            }
        }
        for t in 0u8..4 {
            for o in 0u8..4 {
                cases.push(case!(payment::action(t, o)));
            }
        }
        for code in 0u8..5 {
            cases.push(case!(payment::cancel(code)));
        }
        for v in [i64::MIN, -1, 0, 49, 50, 99_999_999, 100_000_000, i64::MAX] {
            cases.push(case!(payment::amount_of(v)));
        }
        for s in ["", "pm_", "pm_x", "pa_x", "PM_x", "pmx_", "pm_é", "é"] {
            cases.push(case!(payment::method_id(s.to_string())));
        }
        cases
    });
    for reached in [
        "Ok(PaymentIntent { terms: Terms { amount: Amount(2000), capture: CaptureMethod::Automatic, confirmation: ConfirmationMethod::Automatic }, status: Status::Succeeded { received: 2000, application_fee: None } })",
        "Status::Succeeded { received: 1500, application_fee: Some(1500) }",
        "Status::RequiresCapture {",
        "Status::RequiresAction {",
        "Status::Processing {",
        "Status::RequiresConfirmation {",
        "Status::RequiresPaymentMethod { last_error: Some(DeclineCode::DebitFailed) }",
        "Status::RequiresPaymentMethod { last_error: Some(DeclineCode::AuthenticationFailed) }",
        "Status::Canceled { reason: Some(CancellationReason::RequestedByCustomer) }",
        "Err(PaymentError::MissingPaymentMethod)",
        "Err(PaymentError::InvalidCaptureAmount { capturable: 2000 })",
        "Err(PaymentError::NegativeApplicationFee)",
        "Err(PaymentError::NotCancelable)",
        "Err(PaymentError::InvalidTransition)",
        "Err(PaymentError::AmountOutOfRange)",
        "Err(PaymentError::InvalidPaymentMethodId)",
    ] {
        assert!(cases.iter().any(|c| c.rust.contains(reached)), "no run reaches {reached}");
    }
    support::assert_equivalent("payment", SOURCE, &cases);
}
