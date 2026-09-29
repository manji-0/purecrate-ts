// The status lifecycle of a Stripe PaymentIntent, from Stripe's API
// reference (the PaymentIntent object, capture, cancel) and "How intents
// work" (docs.stripe.com, read 2026-09-29):
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
// - With manual confirmation, the intent returns to `requires_confirmation`
//   after the customer's action, and the server confirms again.
// - Cancelable in `requires_payment_method`, `requires_confirmation`,
//   `requires_action`, `requires_capture`, and, for bank debits only,
//   `processing`. `succeeded` and `canceled` are final.
// - `amount` is a positive integer in the smallest currency unit, at least
//   50 ($0.50) and at most eight digits.
//
// Not modeled: Stripe-internal cancellation reasons, multicapture, capture
// that goes to `processing`, updating the method during `requires_action`,
// and the currency-specific minimums other than USD's.
//
// The same types are the server's wire format: serde derives with no
// attributes, except that `Amount` and `PaymentMethodId` are read through
// their checked constructors (`#[serde(try_from)]`), so JSON cannot carry an
// out-of-range amount or a malformed ID on either side.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize)]
#[serde(try_from = "i64")]
pub struct Amount(i64);

impl Amount {
    pub fn new(value: i64) -> Result<Amount, PaymentError> {
        if value < 50i64 || value > 99999999i64 {
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
        if raw.len() < 4usize || !raw.starts_with("pm_") {
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

#[derive(Serialize, Deserialize)]
pub struct PaymentIntent {
    pub terms: Terms,
    pub status: Status,
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
}

/// For the server's logs and serde's `try_from` errors. Not translated: the
/// client shows its own wording for each variant.
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
    let status = match intent.status {
        Status::RequiresPaymentMethod { .. } => requires_payment_method(&terms, event)?,
        Status::RequiresConfirmation { method } => requires_confirmation(&terms, method, event)?,
        Status::RequiresAction { method } => requires_action(&terms, method, event)?,
        Status::Processing { method } => processing(&terms, method, event)?,
        Status::RequiresCapture { capturable, .. } => requires_capture(capturable, event)?,
        Status::Succeeded { .. } | Status::Canceled { .. } => return Err(final_state(event)),
    };
    Ok(PaymentIntent { terms, status })
}

fn authorized(terms: &Terms, method: PaymentMethod) -> Status {
    match terms.capture {
        CaptureMethod::Automatic => Status::Succeeded {
            received: terms.amount.0,
            application_fee: None,
        },
        CaptureMethod::Manual => Status::RequiresCapture {
            method,
            capturable: terms.amount.0,
        },
    }
}

fn attempt(terms: &Terms, method: PaymentMethod, outcome: Outcome) -> Status {
    match outcome {
        Outcome::Authorized => authorized(terms, method),
        Outcome::ActionRequired => Status::RequiresAction { method },
        Outcome::Pending => Status::Processing { method },
        Outcome::Declined(code) => Status::RequiresPaymentMethod {
            last_error: Some(code),
        },
    }
}

fn requires_payment_method(terms: &Terms, event: Event) -> Result<Status, PaymentError> {
    match event {
        Event::AttachMethod(method) => Ok(Status::RequiresConfirmation { method }),
        Event::Confirm { method, outcome } => match method {
            Some(method) => Ok(attempt(terms, method, outcome)),
            None => Err(PaymentError::MissingPaymentMethod),
        },
        Event::Cancel(reason) => Ok(Status::Canceled { reason }),
        _ => Err(PaymentError::InvalidTransition),
    }
}

fn requires_confirmation(
    terms: &Terms,
    current: PaymentMethod,
    event: Event,
) -> Result<Status, PaymentError> {
    match event {
        Event::AttachMethod(method) => Ok(Status::RequiresConfirmation { method }),
        Event::Confirm { method, outcome } => match method {
            Some(method) => Ok(attempt(terms, method, outcome)),
            None => Ok(attempt(terms, current, outcome)),
        },
        Event::Cancel(reason) => Ok(Status::Canceled { reason }),
        _ => Err(PaymentError::InvalidTransition),
    }
}

fn requires_action(
    terms: &Terms,
    method: PaymentMethod,
    event: Event,
) -> Result<Status, PaymentError> {
    match event {
        Event::ActionHandled(outcome) => match terms.confirmation {
            ConfirmationMethod::Automatic => Ok(attempt(terms, method, outcome)),
            ConfirmationMethod::Manual => match outcome {
                Outcome::Declined(code) => Ok(Status::RequiresPaymentMethod {
                    last_error: Some(code),
                }),
                _ => Ok(Status::RequiresConfirmation { method }),
            },
        },
        Event::Cancel(reason) => Ok(Status::Canceled { reason }),
        _ => Err(PaymentError::InvalidTransition),
    }
}

fn processing(terms: &Terms, method: PaymentMethod, event: Event) -> Result<Status, PaymentError> {
    match event {
        Event::ProcessingSucceeded => Ok(authorized(terms, method)),
        Event::ProcessingFailed(code) => Ok(Status::RequiresPaymentMethod {
            last_error: Some(code),
        }),
        Event::Cancel(reason) => match method.kind {
            MethodKind::BankDebit => Ok(Status::Canceled { reason }),
            MethodKind::Card => Err(PaymentError::NotCancelable),
        },
        _ => Err(PaymentError::InvalidTransition),
    }
}

fn requires_capture(capturable: i64, event: Event) -> Result<Status, PaymentError> {
    match event {
        Event::Capture {
            amount_to_capture,
            application_fee,
        } => {
            let received = match amount_to_capture {
                Some(amount) => {
                    if amount < 1i64 || amount > capturable {
                        return Err(PaymentError::InvalidCaptureAmount { capturable });
                    }
                    amount
                }
                None => capturable,
            };
            let application_fee = match application_fee {
                Some(fee) => {
                    if fee < 0i64 {
                        return Err(PaymentError::NegativeApplicationFee);
                    }
                    if fee > received {
                        Some(received)
                    } else {
                        Some(fee)
                    }
                }
                None => None,
            };
            Ok(Status::Succeeded {
                received,
                application_fee,
            })
        }
        Event::Cancel(reason) => Ok(Status::Canceled { reason }),
        _ => Err(PaymentError::InvalidTransition),
    }
}

fn final_state(event: Event) -> PaymentError {
    match event {
        Event::Cancel(_) => PaymentError::NotCancelable,
        _ => PaymentError::InvalidTransition,
    }
}
