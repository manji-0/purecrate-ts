// The canonical text both sides of a differential case print (design/01
// §8), for std's types, and the call that prints a result or a panic. One
// file for every Rust baseline: `test`'s harness splices it into its module
// (`harness::PRELUDE`), the differential tests `include!` it
// (`tests/it/support/values.rs`), and the generated tests' programs start
// with it (`tests/it/support/generated.rs`). So it has no dependencies, no
// `use`, and no inner attributes: it is valid as a module's body and as a
// file. `purecrate_canon` and the harness derive `Show` for a crate's own
// structs and enums (`derive.rs`).

/// The canonical text of a value. Floats are their bits, so `-0` and `0`
/// differ.
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
show_plain!(i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, usize, bool);

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

impl Show for String {
    fn show(&self) -> String {
        self.as_str().show()
    }
}

/// As a string, between `'`: the TS side cannot tell a one-code-point
/// string from a `char` otherwise.
impl Show for char {
    fn show(&self) -> String {
        format!("'{}'", self.to_string().show())
    }
}

impl Show for () {
    fn show(&self) -> String {
        "()".into()
    }
}

impl Show for std::num::ParseIntError {
    fn show(&self) -> String {
        "ParseIntError".into()
    }
}

/// As a crate enum's: the TS side prints the `Ordering` the parser adds as
/// one.
impl Show for std::cmp::Ordering {
    fn show(&self) -> String {
        format!("Ordering::{self:?}")
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

/// `{"a": 1, "b": 2}`, in key order; `{9: 1, 10: 2}` for another key.
impl<K: Show, V: Show> Show for std::collections::BTreeMap<K, V> {
    fn show(&self) -> String {
        format!(
            "{{{}}}",
            self.iter().map(|(k, v)| format!("{}: {}", k.show(), v.show())).collect::<Vec<_>>().join(", ")
        )
    }
}

impl<K: Show> Show for std::collections::BTreeSet<K> {
    fn show(&self) -> String {
        format!("{{{}}}", self.iter().map(|k| k.show()).collect::<Vec<_>>().join(", "))
    }
}

/// A reference, `Box`, and `Arc` are erased in TS: the value they hold.
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
        self.lock().map(|v| v.show()).unwrap_or_else(|_| "poisoned".into())
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
show_tuple!(A 0);
show_tuple!(A 0, B 1);
show_tuple!(A 0, B 1, C 2);
show_tuple!(A 0, B 1, C 2, D 3);
show_tuple!(A 0, B 1, C 2, D 3, E 4);

/// `f`'s result as canonical text, or `panic(message)`.
pub fn shown<T: Show>(f: impl FnOnce() -> T) -> String {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => v.show(),
        Err(p) => {
            let message =
                p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            format!("panic({})", message.unwrap_or_default())
        }
    }
}

/// A float as a JS literal: `{:?}` writes `NaN`, `inf`, and `-inf`; JS
/// spells the last two `Infinity` and `-Infinity`.
pub fn js_float(x: f64) -> String {
    if x.is_infinite() {
        if x > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        format!("{x:?}")
    }
}

/// A string as a JS literal: Rust's escapes, which JS reads, but U+0000 as
/// `\u{0}`: `{:?}` writes `\0`, which before a digit is a legacy octal
/// escape node refuses.
pub fn js_str(s: &str) -> String {
    let body: String =
        s.chars().map(|c| if c == '\0' { "\\u{0}".to_string() } else { c.escape_debug().to_string() }).collect();
    format!("\"{body}\"")
}
