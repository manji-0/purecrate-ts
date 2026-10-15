// `impl Display` becomes `to_string`: the three shapes of a text fixed per
// value (design/01 §7.11); `write!` with arguments, a `match self` of
// writes, writes joined with `?` and `Ok(())`, and fields of crate types
// printed through their own `Display` (§7.25), called by `format!` and
// `to_string`, with arguments that overflow and `fmt`s that panic in Rust's
// order; `write_str` of a computed text; and two it skips (a float printed, a `let` in `fmt`), which must
// leave the crate accepted.

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

pub struct Money {
    pub cents: i64,
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02} €", self.cents / 100, (self.cents % 100).abs())
    }
}

pub enum Token {
    Num(i32),
    Word { text: String, upper: bool },
    Pair(char, char),
    Sep,
    Blank,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Num(n) => write!(f, "#{n:>4}|{:<3}|{n:*^7}", n + 1),
            Token::Word { text, upper } => {
                write!(f, "{{{}}}", text)?;
                f.write_str("/")?;
                write!(f, "{upper:>6}")?;
                Ok(())
            }
            Token::Pair(a, b) => write!(f, "{1}{0}{s}\u{301}", a, b, s = a),
            Token::Sep => f.write_str("`${sep}` \\ é"),
            Token::Blank => Ok(()),
        }
    }
}

pub struct Line {
    pub price: Money,
    pub token: Token,
    pub qty: u8,
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let qty = self.qty + 1;
        write!(f, "{} x{qty} [{}]", self.price, self.token)
    }
}

pub enum Tree {
    Leaf(u32),
    Node(Box<Tree>, Box<Tree>),
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tree::Leaf(n) => write!(f, "{n}"),
            Tree::Node(l, r) => write!(f, "({l} {r})"),
        }
    }
}

pub struct Boom {
    pub n: i32,
}

impl fmt::Display for Boom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "boom {}", self.n + 1)?;
        write!(f, " {}", 100 / self.n)
    }
}

pub struct Empty {}

impl fmt::Display for Empty {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

pub struct Rate {
    pub r: f64,
}

impl fmt::Display for Rate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.r)
    }
}

pub struct Twice {
    pub n: i32,
}

impl fmt::Display for Twice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.n * 2;
        let (a, b) = (m, self.n);
        write!(f, "{a}={b}*2")
    }
}

pub struct Signed {
    pub n: i32,
}

impl fmt::Display for Signed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.n < 0 {
            write!(f, "-{}", -self.n)
        } else {
            write!(f, "+{}", self.n)
        }
    }
}

pub fn show_money(cents: i64) -> String {
    let m = Money { cents };
    format!("{}|{m}|{m:}", m.to_string())
}

pub fn show_token(t: Token) -> String {
    t.to_string()
}

pub fn show_line(cents: i64, n: i32, qty: u8) -> String {
    let line = Line { price: Money { cents }, token: Token::Num(n), qty };
    format!("<{line}>")
}

pub fn show_tree(depth: u32) -> String {
    let mut t = Tree::Leaf(0);
    let mut i = 1;
    while i <= depth {
        t = Tree::Node(Box::new(t), Box::new(Tree::Leaf(i)));
        i += 1;
    }
    format!("{}", t)
}

pub fn first_panic(a: i32, b: i32) -> String {
    let x = Boom { n: a };
    let y = Boom { n: b };
    format!("{1} {0}", x, y)
}

pub fn computed_first(a: i32, b: i32) -> String {
    let x = Boom { n: a };
    format!("{} {}", b + 1, x)
}

pub fn show_empty() -> String {
    format!("[{}]", Empty {})
}

pub fn show_twice(n: i32) -> String {
    let t = Twice { n };
    format!("{t}")
}

pub fn skipped(r: f64, n: i32) -> f64 {
    let s = Signed { n };
    Rate { r }.r + f64::from(s.n)
}

// `write_str` of a computed text writes that text: a field, a `String` a
// method builds, a `&str` a method returns, a slice that panics off a char
// boundary, and a local, alone or among other writes.
pub struct Code(pub String);

impl Code {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn shout(&self) -> String {
        self.0.to_uppercase()
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Loud(pub Code);

impl fmt::Display for Loud {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.shout())
    }
}

pub enum Label {
    Code(Code),
    Tail { text: String, n: u8 },
    Pair(String, String),
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Label::Code(c) => f.write_str(c.as_str()),
            Label::Tail { text, n } => {
                f.write_str("<")?;
                f.write_str(&text[1..])?;
                write!(f, ">{}", n + 1)
            }
            Label::Pair(a, b) => {
                let t = format!("{a}+{b}");
                f.write_str(&t)?;
                f.write_str(a.trim())?;
                Ok(())
            }
        }
    }
}

pub fn show_code(s: String) -> String {
    let c = Code(s);
    format!("{}[{c}]", c.to_string())
}

pub fn show_loud(s: String) -> String {
    Loud(Code(s)).to_string()
}

pub fn show_label(l: Label) -> String {
    format!("{l}")
}
