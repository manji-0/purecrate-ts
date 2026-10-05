//! `examples/payment`, Stripe's PaymentIntent lifecycle written inside the
//! constraints of design/02, agrees with the generated package on every
//! four-event run under each capture and confirmation method, compared as
//! the whole final `PaymentIntent`. It also agrees with the same rules in
//! idiomatic Rust (`idiomatic`, the line count design/07 §2 compares
//! against) on the same runs. An intent read from outside goes through
//! `PaymentIntent::new`, which refuses amounts that disagree with the terms:
//! in Rust, in the generated function, and in every library's schema.

use crate::support;

use std::fs;
use std::process::Command;

use purecrate_check::accept;
use purecrate_emit_ts::WireSchema;
use purecrate_pack::assemble_with;
use purecrate_syntax::parse_source;

purecrate_canon::fixture!(mod payment = "../../../examples/payment/src/lib.rs", "fixtures/payment_driver.rs");

/// The same lifecycle as one would write it without the subset's
/// constraints: a tuple `match` with a wildcard, `Option` combinators, and
/// `min`. Not converted; the reference only.
mod idiomatic {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Kind {
        Card,
        BankDebit,
    }
    #[derive(Clone, Debug, PartialEq)]
    pub struct Method {
        pub id: String,
        pub kind: Kind,
    }
    #[derive(Clone, Copy, PartialEq)]
    pub enum Capture {
        Automatic,
        Manual,
    }
    #[derive(Clone, Copy, PartialEq)]
    pub enum Confirmation {
        Automatic,
        Manual,
    }
    pub struct Terms {
        pub amount: i64,
        pub capture: Capture,
        pub confirmation: Confirmation,
    }
    // Stripe's full lists; the runs reach only some of them.
    #[allow(dead_code)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Decline {
        CardDeclined,
        InsufficientFunds,
        AuthenticationFailed,
        DebitFailed,
    }
    #[allow(dead_code)]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Reason {
        Duplicate,
        Fraudulent,
        RequestedByCustomer,
        Abandoned,
    }

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

    pub enum Outcome {
        Authorized,
        ActionRequired,
        Pending,
        Declined(Decline),
    }

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
    pub enum Error {
        MissingPaymentMethod,
        InvalidCaptureAmount { capturable: i64 },
        NegativeApplicationFee,
        NotCancelable,
        InvalidTransition,
        ManualCaptureUnsupported,
    }

    fn usable(t: &Terms, method: Method) -> Result<Method, Error> {
        if t.capture == Capture::Manual && method.kind == Kind::BankDebit {
            return Err(Error::ManualCaptureUnsupported);
        }
        Ok(method)
    }

    fn attempt(t: &Terms, method: Method, outcome: Outcome) -> Status {
        match outcome {
            Outcome::Authorized if t.capture == Capture::Manual => {
                Status::RequiresCapture { method, capturable: t.amount }
            }
            Outcome::Authorized => Status::Succeeded { received: t.amount, application_fee: None },
            Outcome::ActionRequired => Status::RequiresAction { method },
            Outcome::Pending => Status::Processing { method },
            Outcome::Declined(code) => Status::RequiresPaymentMethod { last_error: Some(code) },
        }
    }

    pub fn step(t: &Terms, status: Status, event: Event) -> Result<Status, Error> {
        use Status::*;
        Ok(match (status, event) {
            (RequiresPaymentMethod { .. } | RequiresConfirmation { .. }, Event::AttachMethod(method)) => {
                RequiresConfirmation { method: usable(t, method)? }
            }
            (RequiresPaymentMethod { .. }, Event::Confirm { method, outcome }) => {
                attempt(t, usable(t, method.ok_or(Error::MissingPaymentMethod)?)?, outcome)
            }
            (RequiresConfirmation { method: current }, Event::Confirm { method, outcome }) => {
                attempt(t, usable(t, method.unwrap_or(current))?, outcome)
            }
            (RequiresAction { .. }, Event::ActionHandled(Outcome::Declined(code))) => {
                RequiresPaymentMethod { last_error: Some(code) }
            }
            (RequiresAction { method }, Event::ActionHandled(_)) if t.confirmation == Confirmation::Manual => {
                RequiresConfirmation { method }
            }
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
            (Processing { .. } | Succeeded { .. } | Canceled { .. }, Event::Cancel(_)) => {
                return Err(Error::NotCancelable)
            }
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
    codes
        .iter()
        .try_fold(Status::RequiresPaymentMethod { last_error: None }, |s, &c| step(&terms, s, idiomatic_event(c)))
}

/// Both sides as one text: variant names and numbers, through `Debug`.
fn same(
    a: &Result<payment::PaymentIntent, payment::PaymentError>,
    b: &Result<idiomatic::Status, idiomatic::Error>,
) -> bool {
    use crate::support::Show;
    let a = match a {
        Ok(intent) => format!("Ok({})", intent.status().show()),
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
        CODES.flat_map(move |a| CODES.flat_map(move |b| CODES.flat_map(move |c| CODES.map(move |d| (t, [a, b, c, d])))))
    })
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let differ: Vec<String> = support::quietly(|| {
        runs()
            .filter_map(|(t, [a, b, c, d])| {
                let constrained = payment::trace4(t, a, b, c, d);
                let reference = idiomatic_trace(t, [a, b, c, d]);
                (!same(&constrained, &reference)).then(|| {
                    format!(
                        "{t} {a} {b} {c} {d}: idiomatic {reference:?} / constrained {}",
                        support::Show::show(&constrained)
                    )
                })
            })
            .collect()
    });
    assert!(differ.is_empty(), "{} runs differ, first:\n{}", differ.len(), differ[..differ.len().min(10)].join("\n"));
}

#[test]
fn generated_payment_lifecycle_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases: Vec<_> = runs().map(|(t, [a, b, c, d])| case!(payment::trace4(t, a, b, c, d))).collect();
        grid!(cases, payment::capture; amount in 0u8..7, fee in 0u8..7);
        grid!(cases, payment::action; t in 0u8..4, o in 0u8..4);
        grid!(cases, payment::cancel; code in 0u8..5);
        grid!(cases, payment::read_intent; t in STATUS_TERMS, s in STATUSES);
        grid!(cases, payment::read_and_step; t in STATUS_TERMS, s in STATUSES, e in 0u8..12);
        grid!(cases, payment::confirm_with; t in 0u8..4, a in 0u8..3, m in 0u8..3, o in 0u8..4);
        grid!(cases, payment::capture_of; amount in [1000i64], to_capture in [999i64, 1000, 1001]);
        grid!(cases, payment::amount_of; v in [i64::MIN, -1, 0, 49, 50, 99_999_999, 100_000_000, i64::MAX]);
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
        "Err(PaymentError::InconsistentStatus)",
        "Err(PaymentError::ManualCaptureUnsupported)",
    ] {
        assert!(cases.iter().any(|c| c.rust.contains(reached)), "no run reaches {reached}");
    }
    support::assert_equivalent("payment", payment::SOURCE, &cases);
}

/// The statuses of the driver's `status_of` that agree with each capture
/// method's terms (amount 2000): `capturable` and `received` within
/// 1..=2000, a fee within 0..=received, and, with automatic capture, no
/// `requires_capture`, no fee, and the whole amount received.
/// Manual capture also holds no bank debit (16, 19 to 21).
const MANUAL_ACCEPTS: [u8; 11] = [0, 1, 5, 6, 10, 11, 13, 14, 15, 17, 18];
const AUTOMATIC_ACCEPTS: [u8; 7] = [5, 15, 16, 17, 18, 20, 21];
const STATUSES: std::ops::Range<u8> = 0..22;
const STATUS_TERMS: std::ops::Range<u8> = 0..4;

fn accepted(t: u8, s: u8) -> bool {
    if t.is_multiple_of(2) {
        AUTOMATIC_ACCEPTS.contains(&s)
    } else {
        MANUAL_ACCEPTS.contains(&s)
    }
}

#[test]
fn an_intent_from_outside_is_checked_against_its_terms() {
    support::quietly(|| {
        for t in 0u8..4 {
            for s in STATUSES {
                let read = payment::read_intent(t, s);
                if accepted(t, s) {
                    assert!(read.is_ok(), "terms {t} status {s} refused");
                } else {
                    assert!(
                        matches!(read, Err(payment::PaymentError::InconsistentStatus)),
                        "terms {t} status {s} accepted"
                    );
                }
            }
        }
        // Every state `step` reaches passes the same check.
        for (t, [a, b, c, d]) in runs() {
            if payment::trace4(t, a, b, c, d).is_ok() {
                assert!(payment::trace4_rechecked(t, a, b, c, d).is_ok(), "{t} {a} {b} {c} {d}");
            }
        }
    });
}

/// "Place a hold on a payment method": bank debits do not support manual
/// capture, so under it a bank debit is refused where it is attached or
/// confirmed, and nothing else changes; under automatic capture it goes
/// through as before.
#[test]
fn manual_capture_refuses_a_bank_debit() {
    use payment::PaymentError::ManualCaptureUnsupported;
    let status = |r: &Result<payment::PaymentIntent, payment::PaymentError>| {
        support::Show::show(&r.as_ref().map(|i| i.status()))
    };
    support::quietly(|| {
        for t in [1u8, 3] {
            for o in 0u8..4 {
                // Confirming with a bank debit, attached first or sent with
                // the confirmation (over a card attached before).
                for (a, m) in [(1u8, 2u8), (2, 1), (0, 1)] {
                    let r = payment::confirm_with(t, a, m, o);
                    assert!(matches!(r, Err(ManualCaptureUnsupported)), "{t} {a} {m} {o}: {}", status(&r));
                }
                // A card sent with the confirmation goes through.
                let r = payment::confirm_with(t, 2, 0, o);
                assert!(r.is_ok(), "{t} {o}: {}", status(&r));
            }
            // The finding's run: manual terms, a bank debit authorized.
            let r = payment::trace4(t, 1, 2, 11, 11);
            assert!(matches!(r, Err(ManualCaptureUnsupported)), "{}", status(&r));
        }
        for t in [0u8, 2] {
            let r = payment::confirm_with(t, 2, 1, 0);
            assert!(status(&r).starts_with("Ok(Status::Succeeded { received: 2000"), "{}", status(&r));
            let r = payment::confirm_with(t, 2, 1, 2);
            assert!(status(&r).starts_with("Ok(Status::Processing {"), "{}", status(&r));
        }
        // And from outside: a bank debit in any status that holds a method
        // is inconsistent with manual capture.
        for s in [16u8, 19, 20, 21] {
            assert!(matches!(payment::read_intent(1, s), Err(payment::PaymentError::InconsistentStatus)), "{s}");
        }
    });
}

/// Overcapture is not modeled: a capture above the authorized amount is
/// refused (manual 1000, authorized, `amount_to_capture` 1001), and up to
/// it goes through.
#[test]
fn a_capture_above_the_authorized_amount_is_refused() {
    let received = |r: Result<payment::PaymentIntent, payment::PaymentError>| match r.as_ref().map(|i| i.status()) {
        Ok(payment::Status::Succeeded { received, .. }) => Some(*received),
        _ => None,
    };
    assert_eq!(received(payment::capture_of(1000, 999)), Some(999));
    assert_eq!(received(payment::capture_of(1000, 1000)), Some(1000));
    assert!(matches!(
        payment::capture_of(1000, 1001),
        Err(payment::PaymentError::InvalidCaptureAmount { capturable: 1000 })
    ));
}

/// With manual confirmation, a handled action that succeeds returns to
/// `requires_confirmation`, and a declined one to `requires_payment_method`,
/// as the header says.
#[test]
fn a_handled_action_under_manual_confirmation() {
    let status =
        |r: Result<payment::PaymentIntent, payment::PaymentError>| support::Show::show(&r.as_ref().map(|i| i.status()));
    for t in [2u8, 3] {
        for o in 0u8..3 {
            assert!(status(payment::action(t, o)).starts_with("Ok(Status::RequiresConfirmation {"), "{t} {o}");
        }
        assert_eq!(
            status(payment::action(t, 3)),
            "Ok(Status::RequiresPaymentMethod { last_error: Some(DeclineCode::InsufficientFunds) })"
        );
    }
}

/// JS string literal.
fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Writes the package with `schema`, runs `script` on node, and returns its
/// stdout; `None` when node is skipped (`PURECRATE_SKIP_NODE`).
fn run_node_with(schema: WireSchema, script: &str) -> Option<String> {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return None;
    }
    let dir = support::scratch(&format!("payment-intent-wire-{}", schema.runtime_dep()));
    let krate = parse_source("payment", payment::SOURCE).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("payment rejected: {d:#?}"));
    support::write_package(&dir, &assemble_with(&typed, Some(schema)));
    let lib = schema.runtime_dep();
    let package = format!("boundary-{lib}");
    support::link(&dir, lib, &format!("{package}/node_modules/{lib}"));
    if schema == WireSchema::Arktype {
        support::link(&dir, "@ark", &format!("{package}/node_modules/@ark"));
    }
    support::typecheck(&dir);
    fs::write(dir.join("driver.ts"), script).expect("write driver");
    let output = Command::new("node")
        .arg(format!("--conditions={}", support::SOURCE_CONDITION))
        .arg("driver.ts")
        .current_dir(&dir)
        .output()
        .expect("node");
    assert!(output.status.success(), "node in {}:\n{}", dir.display(), String::from_utf8_lossy(&output.stderr));
    let _ = fs::remove_dir_all(&dir);
    Some(String::from_utf8(output.stdout).expect("utf8"))
}

/// `PaymentIntent` is read through `#[serde(try_from = "UncheckedIntent")]`:
/// every library's schema reads the JSON of an intent exactly when
/// `PaymentIntent::new` accepts it, writes back the same bytes, and names
/// the refusal as serde does (`PaymentIntent: ` and the error's `Display`).
/// `UncheckedIntent` reads them all, so the refusals are not about shape.
#[test]
fn every_schema_refuses_an_inconsistent_intent() {
    let mut rows = Vec::new();
    let mut refused = 0;
    for t in 0u8..4 {
        for s in STATUSES {
            let text = serde_json::to_string(&payment::unchecked(t, s)).unwrap();
            let want = match payment::read_intent(t, s) {
                Ok(intent) => serde_json::to_string(&intent).unwrap(),
                Err(_) => "Err".to_string(),
            };
            // The wire form is unchanged: a checked intent is written as
            // the fields it was read from.
            assert!(want == "Err" || want == text, "{text} written as {want}");
            assert_eq!(want != "Err", accepted(t, s), "{text}");
            refused += usize::from(want == "Err");
            rows.push(format!("[{},{}]", js(&text), js(&want)));
        }
    }
    assert!(refused == 11 * 2 + 15 * 2, "{refused} refused");
    let rows = rows.join(",");
    for schema in [WireSchema::Zod, WireSchema::Valibot, WireSchema::Arktype] {
        let (import, refusal) = match schema {
            WireSchema::Zod => ("", "(x) => { const r = w.PaymentIntent.safeParse(x); return r.success ? null : r.error.issues[0].message; }"),
            WireSchema::Valibot => (
                "import * as v from \"valibot\";\n",
                "(x) => { const r = v.safeParse(w.PaymentIntent, x); return r.success ? null : r.issues[0].message; }",
            ),
            WireSchema::Arktype => (
                "import { type } from \"arktype\";\n",
                "(x) => { const r = w.PaymentIntent(x); return r instanceof type.errors ? r.summary : null; }",
            ),
        };
        let script = format!(
            "{import}import {{ parseJson }} from \"./src/purecrate-runtime.ts\";\n\
             import * as w from \"./src/purecrate-wire.ts\";\n\
             const refusal = {refusal};\n\
             const out = [];\n\
             for (const [text, want] of [{rows}]) {{\n\
               const x = parseJson(text);\n\
               if (w.toJson.UncheckedIntent(w.fromJson.UncheckedIntent(text)) !== text) out.push(`unchecked ${{text}}`);\n\
               const why = refusal(x);\n\
               const got = why === null ? w.toJson.PaymentIntent(w.fromJson.PaymentIntent(text)) : \"Err\";\n\
               if (got !== want) out.push(`${{text}}: want ${{want}} got ${{got}}`);\n\
               if (why !== null && !why.includes(\"PaymentIntent: status inconsistent with the terms\")) out.push(`${{text}}: ${{why}}`);\n\
               if (why !== null) {{ try {{ w.fromJson.PaymentIntent(text); out.push(`${{text}}: fromJson read it`); }} catch {{}} }}\n\
             }}\n\
             console.log(out.length === 0 ? \"ok\" : out.join(\"\\n\"));\n"
        );
        if let Some(out) = run_node_with(schema, &script) {
            assert_eq!(out.trim(), "ok", "{schema:?}:\n{out}");
        }
    }
}
