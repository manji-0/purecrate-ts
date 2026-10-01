// `impl Display` whose text is fixed per value becomes `to_string`: the
// three shapes `lower_display` takes, and one it skips (formatting
// arguments), which must leave the crate accepted.

use std::fmt;

pub enum Fixed {
    A,
    B,
}

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Fixed::A => "a",
            Fixed::B => "b {x} \u{e9}",
        })
    }
}

pub enum Arms {
    One,
    Two { n: i32 },
}

impl fmt::Display for Arms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arms::One => write!(f, "one {{braces}}"),
            Arms::Two { .. } => f.write_str("two"),
        }
    }
}

pub enum Bound {
    X,
    Y,
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Bound::X => "x",
            Bound::Y => "y",
        };
        f.write_str(text)
    }
}

pub struct Plain {
    pub n: i32,
}

impl fmt::Display for Plain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "plain")
    }
}

pub enum Formatted {
    N(i32),
}

impl fmt::Display for Formatted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Formatted::N(n) => write!(f, "n = {n}"),
        }
    }
}

pub fn show_fixed(c: u8) -> String {
    let v = if c == 0 { Fixed::A } else { Fixed::B };
    v.to_string()
}

pub fn show_arms(c: u8) -> String {
    let v = if c == 0 { Arms::One } else { Arms::Two { n: 1 } };
    v.to_string()
}

pub fn show_bound(c: u8) -> String {
    let v = if c == 0 { Bound::X } else { Bound::Y };
    v.to_string()
}

pub fn show_plain(n: i32) -> String {
    Plain { n }.to_string()
}

pub fn show_formatted(v: Formatted) -> i32 {
    match v {
        Formatted::N(m) => m,
    }
}
