//! Rust values as TS arguments (`Js`) and as the canonical text both
//! sides print (`Show`).

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
js_bigint!(i64, u64);

/// `{:?}` writes `NaN`, `inf`, and `-inf`; JS spells the last two
/// `Infinity` and `-Infinity`.
fn js_float(x: f64) -> String {
    if x.is_infinite() {
        if x > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        format!("{x:?}")
    }
}

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

/// `{:?}` is a JS string literal too: JS reads its `\u{…}` escapes, and a
/// code point Rust prints as is stays as is.
/// Rust's escapes, but U+0000 as `\u{0}`: `{:?}` writes `\0`, which
/// before a digit is a legacy octal escape node refuses.
impl Js for str {
    fn js(&self) -> String {
        let body: String =
            self.chars().map(|c| if c == '\0' { "\\u{0}".to_string() } else { c.escape_debug().to_string() }).collect();
        format!("\"{body}\"")
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

impl<T: Js> Js for Vec<T> {
    fn js(&self) -> String {
        self.as_slice().js()
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

/// A value as canonical text: the format `purecrate_canon::fixture!` gives
/// every struct and enum, and the TS driver prints from the same IR
/// (`ts_printer`). Floats are their bits, so `-0` and `0` differ.
pub trait Show {
    fn show(&self) -> String;
}

macro_rules! show_plain {
    ($($t:ty),*) => {$(
        impl Show for $t {
            fn show(&self) -> String {
                self.to_string()
            }
        }
    )*};
}
show_plain!(i8, i16, i32, i64, u8, u16, u32, u64, usize, bool);

impl Show for f32 {
    fn show(&self) -> String {
        f64::from(*self).to_bits().to_string()
    }
}

impl Show for f64 {
    fn show(&self) -> String {
        self.to_bits().to_string()
    }
}

/// Printable ASCII but `"` and `\` as is, any other code point `\u{hex}`.
impl Show for str {
    fn show(&self) -> String {
        let mut out = String::from("\"");
        for c in self.chars() {
            if (' '..='~').contains(&c) && c != '"' && c != '\\' {
                out.push(c);
            } else {
                out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
            }
        }
        out.push('"');
        out
    }
}

/// As a string, between `'`: the TS side cannot tell a one-code-point
/// string from a `char` otherwise.
impl Show for char {
    fn show(&self) -> String {
        format!("'{}'", self.to_string().show())
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

/// As `purecrate_canon` shows a crate enum: the TS side prints the
/// `Ordering` the parser adds as one.
impl Show for std::cmp::Ordering {
    fn show(&self) -> String {
        format!("Ordering::{self:?}")
    }
}

impl Show for String {
    fn show(&self) -> String {
        self.as_str().show()
    }
}

impl Show for () {
    fn show(&self) -> String {
        "()".into()
    }
}

impl<T: Show + ?Sized> Show for &T {
    fn show(&self) -> String {
        (**self).show()
    }
}

impl<T: Show + ?Sized> Show for Box<T> {
    fn show(&self) -> String {
        (**self).show()
    }
}

impl<T: Show + ?Sized> Show for std::sync::Arc<T> {
    fn show(&self) -> String {
        (**self).show()
    }
}

impl<T: Show> Show for std::sync::Mutex<T> {
    fn show(&self) -> String {
        self.lock().expect("lock").show()
    }
}

impl<T: Show> Show for Option<T> {
    fn show(&self) -> String {
        match self {
            Some(v) => format!("Some({})", v.show()),
            None => "None".into(),
        }
    }
}

impl<T: Show, E: Show> Show for Result<T, E> {
    fn show(&self) -> String {
        match self {
            Ok(v) => format!("Ok({})", v.show()),
            Err(e) => format!("Err({})", e.show()),
        }
    }
}

impl<T: Show> Show for [T] {
    fn show(&self) -> String {
        format!("[{}]", self.iter().map(Show::show).collect::<Vec<_>>().join(", "))
    }
}

impl<T: Show> Show for Vec<T> {
    fn show(&self) -> String {
        self.as_slice().show()
    }
}

macro_rules! show_tuple {
    ($($t:ident $i:tt),+) => {
        impl<$($t: Show),+> Show for ($($t,)+) {
            fn show(&self) -> String {
                format!("({})", [$(self.$i.show()),+].join(", "))
            }
        }
    };
}
show_tuple!(A 0, B 1);
show_tuple!(A 0, B 1, C 2);
show_tuple!(A 0, B 1, C 2, D 3);
