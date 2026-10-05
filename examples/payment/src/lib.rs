// The status lifecycle of a Stripe PaymentIntent, from Stripe's API
// reference (the PaymentIntent object, capture, cancel) and "How intents
// work" (docs.stripe.com, read 2026-09-29), with "Place a hold on a payment
// method" and "Capture more than the authorized amount" (read 2026-10-05):
//
// - Created in `requires_payment_method`. Attaching a payment method moves it
//   to `requires_confirmation`; most integrations skip that state by sending
//   the method with the confirmation.
// - A confirmation attempt either needs customer action (3D Secure:
//   `requires_action`), is pending for asynchronous methods (`processing`),
//   is authorized, or fails, which returns the intent to
//   `requires_payment_method` so the payment can be retried.
// - Authorized funds are captured at once (`succeeded`) or, with manual
//   capture, held in `requires_capture`. `amount_to_capture` must not exceed
//   the capturable amount and defaults to all of it; the application fee is
//   capped at the amount captured.
// - Of this model's two kinds, only cards support separate authorization
//   and capture: bank debits (ACH and the like) do not ("Place a hold",
//   payment method limitations), so under `CaptureMethod::Manual` a `BankDebit`
//   method is refused where it is attached or confirmed
//   (`ManualCaptureUnsupported`), and an intent read from outside may not
//   hold one.
// - With manual confirmation, a customer action that succeeds returns the
//   intent to `requires_confirmation`, and the server confirms again; one
//   that fails returns it to `requires_payment_method`, as any failed
//   attempt does.
// - Cancelable in `requires_payment_method`, `requires_confirmation`,
//   `requires_action`, `requires_capture`, and, for bank debits only,
//   `processing`. `succeeded` and `canceled` are final.
// - `amount` is a positive integer in the smallest currency unit, at least
//   50 ($0.50) and at most eight digits.
//
// Not modeled: Stripe-internal cancellation reasons, among them the
// transition to `canceled` after too many failed confirmations and the
// expiry of an uncaptured authorization; multicapture; overcapture, which
// some online card payments allow when requested at confirmation
// (`request_overcapture`), up to a limit the card network sets by merchant
// category and Stripe reports per charge (`maximum_amount_capturable`), so
// here a capture above the capturable amount is refused
// (`InvalidCaptureAmount`); capture that goes to `processing`; the failure
// of a cancel during `processing`, which "How intents work" says may fail
// as its window is limited and varies, so here that cancel always succeeds
// and the caller sends `Cancel` once Stripe has accepted it; updating the
// method during `requires_action`; creating with a method or confirming at
// once (`create` always starts in `requires_payment_method`);
// `capture_method=automatic_async`, Stripe's default, whose states are
// those of `automatic`; a capture method per payment method type
// (`payment_method_options[card][capture_method]`), with which one intent
// holds a card and takes a bank debit at once; an `application_fee_amount`
// set at creation (a fee is given at capture only, so an automatic capture
// has none); and currencies: amounts are USD cents, with USD's minimum. A
// capture of 0 is refused, though Stripe states only the upper bound.
//
// The same types are the server's wire format: serde derives with no
// attributes, except that `Amount`, `PaymentMethodId`, and `PaymentIntent`
// are read through their checked constructors (`#[serde(try_from)]`), so
// JSON cannot carry an out-of-range amount, a malformed ID, or a status
// that disagrees with the terms (a negative fee, a capture above the
// amount, a bank debit under manual capture) on either side.
// `PaymentIntent` is read as `UncheckedIntent`, which has the same fields,
// so its JSON is what it was before the check.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize)]
#[serde(try_from = "i64")]
pub struct Amount(i64);

impl Amount {
    pub fn new(value: i64) -> Result<Amount, PaymentError> {
        if value < 50 || value > 99999999 {
            return Err(PaymentError::AmountOutOfRange);
        }
        Ok(Amount(value))
    }
}

impl TryFrom<i64> for Amount {
    type Error = PaymentError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Amount::new(value)
    }
}

/// A payment method ID: `pm_` followed by at least one character.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct PaymentMethodId(String);

impl PaymentMethodId {
    pub fn new(raw: String) -> Result<PaymentMethodId, PaymentError> {
        if raw.len() < 4 || !raw.starts_with("pm_") {
            return Err(PaymentError::InvalidPaymentMethodId);
        }
        Ok(PaymentMethodId(raw))
    }
}

impl TryFrom<String> for PaymentMethodId {
    type Error = PaymentError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        PaymentMethodId::new(raw)
    }
}

#[derive(Serialize, Deserialize)]
pub enum MethodKind {
    Card,
    BankDebit,
}

#[derive(Serialize, Deserialize)]
pub struct PaymentMethod {
    pub id: PaymentMethodId,
    pub kind: MethodKind,
}

#[derive(Serialize, Deserialize)]
pub enum CaptureMethod {
    Automatic,
    Manual,
}

#[derive(Serialize, Deserialize)]
pub enum ConfirmationMethod {
    Automatic,
    Manual,
}

#[derive(Serialize, Deserialize)]
pub struct Terms {
    pub amount: Amount,
    pub capture: CaptureMethod,
    pub confirmation: ConfirmationMethod,
}

#[derive(Serialize, Deserialize)]
pub enum DeclineCode {
    CardDeclined,
    InsufficientFunds,
    AuthenticationFailed,
    DebitFailed,
}

#[derive(Serialize, Deserialize)]
pub enum CancellationReason {
    Duplicate,
    Fraudulent,
    RequestedByCustomer,
    Abandoned,
}

#[derive(Serialize, Deserialize)]
pub enum Status {
    RequiresPaymentMethod {
        last_error: Option<DeclineCode>,
    },
    RequiresConfirmation {
        method: PaymentMethod,
    },
    RequiresAction {
        method: PaymentMethod,
    },
    Processing {
        method: PaymentMethod,
    },
    RequiresCapture {
        method: PaymentMethod,
        capturable: i64,
    },
    Succeeded {
        received: i64,
        application_fee: Option<i64>,
    },
    Canceled {
        reason: Option<CancellationReason>,
    },
}

/// An intent whose amounts agree with its terms: `PaymentIntent::new`
/// checks them, and `step` keeps them (see `consistent`).
#[derive(Serialize, Deserialize)]
#[serde(try_from = "UncheckedIntent")]
pub struct PaymentIntent {
    terms: Terms,
    status: Status,
}

/// An intent as it arrives, before `PaymentIntent::new` checks it: the same
/// fields, so the JSON is the same.
#[derive(Serialize, Deserialize)]
pub struct UncheckedIntent {
    pub terms: Terms,
    pub status: Status,
}

impl PaymentIntent {
    pub fn new(raw: UncheckedIntent) -> Result<PaymentIntent, PaymentError> {
        if !consistent(&raw.terms, &raw.status) {
            return Err(PaymentError::InconsistentStatus);
        }
        Ok(PaymentIntent {
            terms: raw.terms,
            status: raw.status,
        })
    }

    pub fn terms(&self) -> &Terms {
        &self.terms
    }

    pub fn status(&self) -> &Status {
        &self.status
    }
}

impl TryFrom<UncheckedIntent> for PaymentIntent {
    type Error = PaymentError;

    fn try_from(raw: UncheckedIntent) -> Result<Self, Self::Error> {
        PaymentIntent::new(raw)
    }
}

/// Stripe's amounts against the intent's: `amount_capturable` and
/// `amount_received` are at most `amount` (and at least 1, as a capture of 0
/// is refused here); `requires_capture` happens with manual capture only;
/// an automatic capture receives the whole amount and carries no fee; the
/// application fee is not negative and is capped at the amount captured;
/// and manual capture holds no bank debit.
fn consistent(terms: &Terms, status: &Status) -> bool {
    let amount = terms.amount.0;
    let manual = matches!(terms.capture, CaptureMethod::Manual);
    let method = match status {
        Status::RequiresConfirmation { method } => Some(method),
        Status::RequiresAction { method } => Some(method),
        Status::Processing { method } => Some(method),
        Status::RequiresCapture { method, .. } => Some(method),
        _ => None,
    };
    if manual && matches!(method, Some(PaymentMethod { kind: MethodKind::BankDebit, .. })) {
        return false;
    }
    match status {
        Status::RequiresCapture { capturable, .. } => {
            manual && *capturable >= 1 && *capturable <= amount
        }
        Status::Succeeded {
            received,
            application_fee,
        } => {
            let fee_ok = match application_fee {
                Some(fee) => manual && *fee >= 0 && *fee <= *received,
                None => true,
            };
            fee_ok && *received >= 1 && *received <= amount && (manual || *received == amount)
        }
        _ => true,
    }
}

/// What Stripe reports for a confirmation attempt or a completed action.
#[derive(Serialize, Deserialize)]
pub enum Outcome {
    Authorized,
    ActionRequired,
    Pending,
    Declined(DeclineCode),
}

#[derive(Serialize, Deserialize)]
pub enum Event {
    AttachMethod(PaymentMethod),
    Confirm {
        method: Option<PaymentMethod>,
        outcome: Outcome,
    },
    ActionHandled(Outcome),
    ProcessingSucceeded,
    ProcessingFailed(DeclineCode),
    Capture {
        amount_to_capture: Option<i64>,
        application_fee: Option<i64>,
    },
    Cancel(Option<CancellationReason>),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PaymentError {
    AmountOutOfRange,
    InvalidPaymentMethodId,
    MissingPaymentMethod,
    InvalidCaptureAmount { capturable: i64 },
    NegativeApplicationFee,
    NotCancelable,
    InvalidTransition,
    InconsistentStatus,
    /// A bank debit under manual capture, which it does not support.
    ManualCaptureUnsupported,
}

/// For the server's logs and serde's `try_from` errors; the client gets the
/// same text from `PaymentError.toString`.
impl fmt::Display for PaymentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            PaymentError::AmountOutOfRange => "amount must be 50 to 99999999",
            PaymentError::InvalidPaymentMethodId => "payment method ID must be pm_ and more",
            PaymentError::MissingPaymentMethod => "no payment method attached",
            PaymentError::InvalidCaptureAmount { .. } => "capture amount out of range",
            PaymentError::NegativeApplicationFee => "negative application fee",
            PaymentError::NotCancelable => "not cancelable",
            PaymentError::InvalidTransition => "invalid transition",
            PaymentError::InconsistentStatus => "status inconsistent with the terms",
            PaymentError::ManualCaptureUnsupported => "bank debits do not support manual capture",
        };
        f.write_str(text)
    }
}

impl std::error::Error for PaymentError {}

pub fn create(terms: Terms) -> PaymentIntent {
    PaymentIntent {
        terms,
        status: Status::RequiresPaymentMethod { last_error: None },
    }
}

pub fn step(intent: PaymentIntent, event: Event) -> Result<PaymentIntent, PaymentError> {
    let terms = intent.terms;
    let status = match (intent.status, event) {
        (
            Status::RequiresPaymentMethod { .. } | Status::RequiresConfirmation { .. },
            Event::AttachMethod(method),
        ) => Status::RequiresConfirmation {
            method: usable(&terms, method)?,
        },
        (Status::RequiresPaymentMethod { .. }, Event::Confirm { method, outcome }) => {
            let method = method.ok_or(PaymentError::MissingPaymentMethod)?;
            attempt(&terms, usable(&terms, method)?, outcome)
        }
        (Status::RequiresConfirmation { method: current }, Event::Confirm { method, outcome }) => {
            attempt(&terms, usable(&terms, method.unwrap_or(current))?, outcome)
        }
        // With manual confirmation the server confirms again; a decline
        // still returns the intent to `requires_payment_method`.
        (Status::RequiresAction { method }, Event::ActionHandled(outcome))
            if matches!(terms.confirmation, ConfirmationMethod::Manual)
                && !matches!(outcome, Outcome::Declined(_)) =>
        {
            Status::RequiresConfirmation { method }
        }
        (Status::RequiresAction { method }, Event::ActionHandled(outcome)) => {
            attempt(&terms, method, outcome)
        }
        (Status::Processing { method }, Event::ProcessingSucceeded) => {
            attempt(&terms, method, Outcome::Authorized)
        }
        (Status::Processing { .. }, Event::ProcessingFailed(code)) => {
            Status::RequiresPaymentMethod {
                last_error: Some(code),
            }
        }
        (
            Status::RequiresCapture { capturable, .. },
            Event::Capture {
                amount_to_capture,
                application_fee,
            },
        ) => {
            if matches!(amount_to_capture, Some(amount) if amount < 1 || amount > capturable) {
                return Err(PaymentError::InvalidCaptureAmount { capturable });
            }
            if matches!(application_fee, Some(fee) if fee < 0) {
                return Err(PaymentError::NegativeApplicationFee);
            }
            let received = amount_to_capture.unwrap_or(capturable);
            Status::Succeeded {
                received,
                application_fee: application_fee.map(|fee| fee.min(received)),
            }
        }
        (
            Status::Processing {
                method: PaymentMethod { kind: MethodKind::BankDebit, .. },
            },
            Event::Cancel(reason),
        ) => Status::Canceled { reason },
        (
            Status::Processing { .. } | Status::Succeeded { .. } | Status::Canceled { .. },
            Event::Cancel(_),
        ) => return Err(PaymentError::NotCancelable),
        (_, Event::Cancel(reason)) => Status::Canceled { reason },
        _ => return Err(PaymentError::InvalidTransition),
    };
    Ok(PaymentIntent { terms, status })
}

/// `method`, unless the terms' capture method does not support it: bank
/// debits have no manual capture.
fn usable(terms: &Terms, method: PaymentMethod) -> Result<PaymentMethod, PaymentError> {
    if matches!(terms.capture, CaptureMethod::Manual) && matches!(method.kind, MethodKind::BankDebit) {
        return Err(PaymentError::ManualCaptureUnsupported);
    }
    Ok(method)
}

fn attempt(terms: &Terms, method: PaymentMethod, outcome: Outcome) -> Status {
    match outcome {
        Outcome::Authorized if matches!(terms.capture, CaptureMethod::Manual) => {
            Status::RequiresCapture {
                method,
                capturable: terms.amount.0,
            }
        }
        Outcome::Authorized => Status::Succeeded {
            received: terms.amount.0,
            application_fee: None,
        },
        Outcome::ActionRequired => Status::RequiresAction { method },
        Outcome::Pending => Status::Processing { method },
        Outcome::Declined(code) => Status::RequiresPaymentMethod {
            last_error: Some(code),
        },
    }
}
