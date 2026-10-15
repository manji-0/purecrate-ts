//! Rust values as TS arguments (`Js`) and as the canonical text both
//! sides print (`Show`, std's in `show.rs`, which `test`'s harness and the
//! generated tests' baselines share).

include!("../../../src/difftest/show.rs");

/// A Rust value as a TS argument expression.
pub trait Js {
    fn js(&self) -> String;
}

macro_rules! js_number {
    ($($t:ty),*) => {$(
        impl Js for $t {
            fn js(&self) -> String {
                self.to_string()
            }
        }
    )*};
}
js_number!(i8, i16, i32, u8, u16, u32, usize, bool);

macro_rules! js_bigint {
    ($($t:ty),*) => {$(
        impl Js for $t {
            fn js(&self) -> String {
                format!("{self}n")
            }
        }
    )*};
}
js_bigint!(i64, u64, i128, u128);

impl Js for f32 {
    fn js(&self) -> String {
        format!("Math.fround({})", js_float(f64::from(*self)))
    }
}

impl Js for f64 {
    fn js(&self) -> String {
        js_float(*self)
    }
}

impl<T: Js + ?Sized> Js for &T {
    fn js(&self) -> String {
        (**self).js()
    }
}

impl Js for str {
    fn js(&self) -> String {
        js_str(self)
    }
}

impl Js for String {
    fn js(&self) -> String {
        self.as_str().js()
    }
}

/// A one-code-point string; the `Char` brand exists only in types.
impl Js for char {
    fn js(&self) -> String {
        self.to_string().js()
    }
}

/// The canonical form, as the generated code holds a `Uuid`.
impl Js for uuid::Uuid {
    fn js(&self) -> String {
        self.hyphenated().to_string().js()
    }
}

/// A fieldless variant, as the generated code holds std's `Ordering`.
impl Js for std::cmp::Ordering {
    fn js(&self) -> String {
        format!("({{ kind: \"{self:?}\" }})")
    }
}

impl<T: Js> Js for [T] {
    fn js(&self) -> String {
        format!("[{}]", self.iter().map(Js::js).collect::<Vec<_>>().join(", "))
    }
}

impl<T: Js, const N: usize> Js for [T; N] {
    fn js(&self) -> String {
        self.as_slice().js()
    }
}

impl<T: Js> Js for Vec<T> {
    fn js(&self) -> String {
        self.as_slice().js()
    }
}

/// Built in the order the test gives, which need not be key order.
impl<K: Js, V: Js> Js for std::collections::BTreeMap<K, V> {
    fn js(&self) -> String {
        format!(
            "new Map([{}])",
            self.iter().rev().map(|(k, v)| format!("[{}, {}]", k.js(), v.js())).collect::<Vec<_>>().join(", ")
        )
    }
}

impl<K: Js> Js for std::collections::BTreeSet<K> {
    fn js(&self) -> String {
        format!("new Set([{}])", self.iter().rev().map(|k| k.js()).collect::<Vec<_>>().join(", "))
    }
}

impl<T: Js> Js for Option<T> {
    fn js(&self) -> String {
        match self {
            Some(v) => v.js(),
            None => "null".into(),
        }
    }
}

/// The generated `Result`: `{ kind, value }` or `{ kind, error }`.
impl<T: Js, E: Js> Js for Result<T, E> {
    fn js(&self) -> String {
        match self {
            Ok(v) => format!("{{ kind: \"Ok\", value: {} }}", v.js()),
            Err(e) => format!("{{ kind: \"Err\", error: {} }}", e.js()),
        }
    }
}

/// `Box` and `Arc` are erased in TS.
impl<T: Js + ?Sized> Js for Box<T> {
    fn js(&self) -> String {
        (**self).js()
    }
}

impl<T: Js + ?Sized> Js for std::sync::Arc<T> {
    fn js(&self) -> String {
        (**self).js()
    }
}

impl<T: Js> Js for std::sync::Mutex<T> {
    fn js(&self) -> String {
        self.lock().expect("unpoisoned").js()
    }
}

impl Js for () {
    fn js(&self) -> String {
        "undefined".into()
    }
}

impl<A: Js, B: Js> Js for (A, B) {
    fn js(&self) -> String {
        format!("[{}, {}]", self.0.js(), self.1.js())
    }
}

impl<A: Js, B: Js, C: Js> Js for (A, B, C) {
    fn js(&self) -> String {
        format!("[{}, {}, {}]", self.0.js(), self.1.js(), self.2.js())
    }
}

/// `Uuid("…")` with the canonical form: the TS side holds only that form.
impl Show for uuid::Uuid {
    fn show(&self) -> String {
        format!("Uuid({})", self.hyphenated().to_string().show())
    }
}

impl Show for uuid::Error {
    fn show(&self) -> String {
        "UuidError".into()
    }
}
