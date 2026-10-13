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
