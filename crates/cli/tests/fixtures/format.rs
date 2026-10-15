// `format!`: `{}` on every integer width, `bool`, `char`, `&str`, `String`,
// and `Uuid`; widths counted in `char`s, each alignment with and without a
// fill, zero padding after the sign; positional, named, and captured
// arguments; escapes the template literal reads as its own; and the
// arguments evaluated in Rust's order.

use uuid::Uuid;

pub fn ints(a: i8, b: u8, c: i32, d: u32, e: i64, f: u64, g: i128, h: u128, i: usize) -> String {
    format!("{a} {b} {c} {d} {e} {f} {g} {h} {i}")
}

pub fn padded_int(n: i64) -> String {
    format!("[{:5}|{:<5}|{:^5}|{:>5}|{:*^7}|{:05}|{:<05}|{:01}|{:0>4}]", n, n, n, n, n, n, n, n, n)
}

pub fn padded_wide(n: i128) -> String {
    format!("[{n:42}|{n:042}|{n:<3}]")
}

pub fn padded_text(s: &str, c: char) -> String {
    format!("[{s:6}|{s:>6}|{s:^6}|{s:^7}|{s:🎉<4}|{c:3}|{c:-^4}]")
}

pub fn flags(ok: bool, n: u16) -> String {
    format!("{ok}/{ok:>6}/{}/{n:03}", !ok)
}

pub fn escapes(s: String) -> String {
    format!("{{{s}}} `x` ${{y}} $ \\ \n\t\u{2028}")
}

pub fn ids(u: Uuid, label: &str) -> String {
    format!("{label}:{u}:{}", label)
}

pub fn named(qty: u32, price: i64) -> String {
    format!("{q} x {p} = {total}", q = qty, p = price, total = i64::from(qty) * price)
}

pub fn positional(a: u32, b: u32) -> String {
    format!("{0}-{1}-{0}-{1}", a, b)
}

// Each argument may panic; Rust evaluates them as written, and the template
// places them in the same order.
pub fn order(a: u8, b: u8) -> String {
    format!("{}+{}", a + 1, b * 2)
}

pub fn invoice_no(year: u16, seq: u32) -> String {
    let prefix = String::from("INV");
    format!("{prefix}-{year}-{seq:06}")
}

pub fn built(xs: Vec<u32>) -> String {
    let mut out = String::new();
    for x in &xs {
        out.push_str(&format!("{x:>3},"));
    }
    out
}

pub fn only_text() -> String {
    format!("plain {{text}}")
}

pub fn sizes(n: usize, ok: bool) -> String {
    format!("[{n:5}|{n:05}|{n:<3}|{ok:6}|{ok:>6}|{ok:^7}]")
}

// A literal argument is the template's text, escaped as the text around it;
// a padded one stays an argument. An argument that overflows still runs in
// Rust's order among them.
pub fn literals(n: u8) -> String {
    format!("{}{n}{}|{}|{}|{}|{}|{:>3}|{}|{}", "b", 'c', 5u8, true, -3i64, "`${\\\u{2028}", 'é', n + 1, 0x0fu8)
}

pub const BASE: u16 = 0x0f;

pub fn constants(n: u8) -> String {
    let b = b'@';
    format!("{BASE}-{}-{b}-{}", b'@', n)
}

// A string placed alone is that string, not a template.
pub fn alone(s: String, t: &str, c: char) -> String {
    let a = format!("{}", s);
    let b = format!("{t}");
    let d = format!("{}", c);
    let mut out = a;
    out.push_str(&b);
    out.push_str(&d);
    out.push_str(&format!("{}", "lit"));
    out.push_str(&7u8.to_string());
    out
}

// A `format!` placed once in another is its pieces in place, its arguments
// still run in Rust's order; a padded one stays an argument. A value padded
// alone is the padding's text.
pub fn nested(n: u8, s: &str) -> String {
    let a = format!("<{}>{}", format!("{}-{s}", n + 1), n.to_string());
    let b = format!("{}", format!("{n}"));
    let c = format!("{:>6}|{}", format!("{s}!"), format!("{}", format!("x")));
    let d = format!("{:*^5}", s);
    let e = format!("{:03}", n);
    format!("{a}/{b}/{c}/{d}/{e}")
}
