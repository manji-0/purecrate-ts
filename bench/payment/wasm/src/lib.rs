//! Three ways a JS caller reaches the same Rust `step`:
//!
//! - `step_json`: serde JSON text in and out; the caller stringifies and
//!   parses.
//! - `step_value`: plain JS objects in and out, converted by
//!   serde-wasm-bindgen.
//! - `Intent`: the state stays in WASM memory behind a handle; only the
//!   event crosses.

use wasm_bindgen::prelude::*;

#[allow(dead_code)]
mod payment {
    include!(concat!(env!("OUT_DIR"), "/payment.rs"));
}

#[cfg(feature = "json")]
use payment::PaymentError;
use payment::{Event, PaymentIntent, Terms};

#[cfg(feature = "json")]
#[wasm_bindgen]
pub fn create_json(terms: &str) -> String {
    let terms: Terms = serde_json::from_str(terms).expect("terms");
    serde_json::to_string(&payment::create(terms)).expect("write")
}

#[cfg(feature = "json")]
#[wasm_bindgen]
pub fn step_json(intent: &str, event: &str) -> String {
    let intent: PaymentIntent = serde_json::from_str(intent).expect("intent");
    let event: Event = serde_json::from_str(event).expect("event");
    let result: Result<PaymentIntent, PaymentError> = payment::step(intent, event);
    serde_json::to_string(&result).expect("write")
}

#[wasm_bindgen]
pub fn step_value(intent: JsValue, event: JsValue) -> Result<JsValue, JsValue> {
    let intent: PaymentIntent = serde_wasm_bindgen::from_value(intent)?;
    let event: Event = serde_wasm_bindgen::from_value(event)?;
    let result = payment::step(intent, event);
    Ok(serde_wasm_bindgen::to_value(&result)?)
}

#[wasm_bindgen]
pub struct Intent(Option<PaymentIntent>);

#[wasm_bindgen]
impl Intent {
    #[wasm_bindgen(constructor)]
    pub fn new(terms: JsValue) -> Result<Intent, JsValue> {
        let terms: Terms = serde_wasm_bindgen::from_value(terms)?;
        Ok(Intent(Some(payment::create(terms))))
    }

    /// Applies `event`; on `Err` the state is gone, as with `step`.
    pub fn step(&mut self, event: JsValue) -> Result<bool, JsValue> {
        let event: Event = serde_wasm_bindgen::from_value(event)?;
        let intent = self.0.take().ok_or_else(|| JsValue::from_str("consumed"))?;
        match payment::step(intent, event) {
            Ok(next) => {
                self.0 = Some(next);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    pub fn state(&self) -> Result<JsValue, JsValue> {
        Ok(serde_wasm_bindgen::to_value(&self.0)?)
    }
}

/// The real serde on the example's own derives: `try_from` rejects what the
/// checked constructors reject, and the shapes are the ones the generated
/// readers and `toJson` use (design/04).
#[cfg(test)]
mod tests {
    use super::payment::{Amount, Event, PaymentMethodId};

    #[test]
    fn serde_try_from_goes_through_the_constructors() {
        assert!(serde_json::from_str::<PaymentMethodId>("\"pm_card\"").is_ok());
        for bad in ["\"\"", "\"pm_\"", "\"pa_x\"", "7"] {
            assert!(serde_json::from_str::<PaymentMethodId>(bad).is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<Amount>("50").is_ok());
        for bad in ["49", "100000000", "\"50\"", "50.5"] {
            assert!(serde_json::from_str::<Amount>(bad).is_err(), "{bad}");
        }
        let bad_event = r#"{"AttachMethod":{"id":"xx","kind":"Card"}}"#;
        let err = serde_json::from_str::<Event>(bad_event).err().expect("rejected");
        assert!(err.to_string().contains("payment method ID must be pm_"), "{err}");
        let good = r#"{"AttachMethod":{"id":"pm_card","kind":"Card"}}"#;
        let event: Event = serde_json::from_str(good).unwrap();
        assert_eq!(serde_json::to_string(&event).unwrap(), good);
    }
}
