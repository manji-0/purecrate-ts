//! `examples/invoice`, consumption tax on a qualified invoice from the
//! National Tax Agency's Q&A (問57, 問59):
//!
//! - the Q&A's own worked examples come out as printed;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against) agree on every invoice below;
//! - the generated package agrees with Rust on every invoice below, whole
//!   `Summary` compared, overflow panics included.

use crate::support;


purecrate_canon::fixture!(mod invoice = "../../../examples/invoice/src/lib.rs");

const SOURCE: &str = invoice::SOURCE;

/// The same rules without the subset's constraints. Not converted.
mod idiomatic {
    #[derive(Clone, Copy, PartialEq)]
    pub enum Rate { Standard, Reduced }
    #[derive(Clone, Copy, PartialEq)]
    pub enum Pricing { Exclusive, Inclusive }
    #[derive(Clone, Copy)]
    pub enum Rounding { Down, Up, HalfUp }
    #[derive(Clone, Copy)]
    pub enum Method { Separate, ToExclusive(Rounding) }
    pub struct Line { pub amount: i64, pub rate: Rate, pub pricing: Pricing }
    #[derive(Debug, PartialEq)]
    pub struct Group { pub base: i64, pub tax: i64 }
    #[derive(Debug, PartialEq)]
    pub struct Summary { pub standard: Group, pub reduced: Group, pub standard_inclusive: Group, pub reduced_inclusive: Group, pub total: i64 }

    impl Rate {
        fn percent(self) -> i64 {
            match self { Rate::Standard => 10, Rate::Reduced => 8 }
        }
    }

    fn divide(n: i64, d: i64, r: Rounding) -> i64 {
        match r {
            Rounding::Down => n / d,
            Rounding::Up => (n + d - 1) / d,
            Rounding::HalfUp => (2 * n + d) / (2 * d),
        }
    }

    pub fn summarize(lines: &[Line], rounding: Rounding, method: Method) -> Option<Summary> {
        if lines.is_empty() {
            return None;
        }
        let group = |rate: Rate, apart: bool| {
            let base: i64 = lines
                .iter()
                .filter(|l| l.rate == rate)
                .filter_map(|l| match (l.pricing, method) {
                    (Pricing::Inclusive, Method::Separate) => apart.then_some(l.amount),
                    (Pricing::Inclusive, Method::ToExclusive(c)) => (!apart).then(|| divide(l.amount * 100, 100 + rate.percent(), c)),
                    (Pricing::Exclusive, _) => (!apart).then_some(l.amount),
                })
                .sum();
            let d = if apart { 100 + rate.percent() } else { 100 };
            Group { base, tax: divide(base * rate.percent(), d, rounding) }
        };
        let (s, r) = (group(Rate::Standard, false), group(Rate::Reduced, false));
        let (si, ri) = (group(Rate::Standard, true), group(Rate::Reduced, true));
        let total = s.base + s.tax + r.base + r.tax + si.base + ri.base;
        Some(Summary { standard: s, reduced: r, standard_inclusive: si, reduced_inclusive: ri, total })
    }
}

use invoice::{Invoice, Line, Method, Pricing, Rate, Rounding, Yen};

fn yen(n: i64) -> Yen {
    Yen::new(n).expect("non-negative")
}

fn line(amount: i64, rate: Rate, pricing: Pricing) -> Line {
    Line { amount: yen(amount), rate, pricing }
}

fn rounding_of(code: u8) -> Rounding {
    match code {
        0 => Rounding::Down,
        1 => Rounding::Up,
        _ => Rounding::HalfUp,
    }
}

fn method_of(code: u8) -> Method {
    match code {
        0 => Method::Separate,
        c => Method::ToExclusive { conversion: rounding_of(c - 1) },
    }
}

/// Kind 0..4: rate by the low bit, pricing by the high bit.
fn kind_line(amount: i64, kind: u8) -> Line {
    let rate = if kind.is_multiple_of(2) { Rate::Standard } else { Rate::Reduced };
    let pricing = if kind / 2 == 0 { Pricing::Exclusive } else { Pricing::Inclusive };
    line(amount, rate, pricing)
}

fn to_idiomatic(inv: &Invoice) -> (Vec<idiomatic::Line>, idiomatic::Rounding, idiomatic::Method) {
    let rounding = |r: &Rounding| match r {
        Rounding::Down => idiomatic::Rounding::Down,
        Rounding::Up => idiomatic::Rounding::Up,
        Rounding::HalfUp => idiomatic::Rounding::HalfUp,
    };
    let lines = inv
        .lines
        .iter()
        .map(|l| idiomatic::Line {
            amount: l.amount.value(),
            rate: match l.rate { Rate::Standard => idiomatic::Rate::Standard, Rate::Reduced => idiomatic::Rate::Reduced },
            pricing: match l.pricing { Pricing::Exclusive => idiomatic::Pricing::Exclusive, Pricing::Inclusive => idiomatic::Pricing::Inclusive },
        })
        .collect();
    let method = match &inv.method {
        Method::Separate => idiomatic::Method::Separate,
        Method::ToExclusive { conversion } => idiomatic::Method::ToExclusive(rounding(conversion)),
    };
    (lines, rounding(&inv.rounding), method)
}

fn project(s: &invoice::Summary) -> idiomatic::Summary {
    let g = |g: &invoice::Group| idiomatic::Group { base: g.base.value(), tax: g.tax.value() };
    idiomatic::Summary {
        standard: g(&s.standard),
        reduced: g(&s.reduced),
        standard_inclusive: g(&s.standard_inclusive),
        reduced_inclusive: g(&s.reduced_inclusive),
        total: s.total.value(),
    }
}

const AMOUNTS: [i64; 8] = [0, 1, 5, 99, 218, 580, 23_894, 60_000];

/// Every invoice of one or two lines over `AMOUNTS` and the four kinds, and
/// pseudo-random three- and five-line ones, under every rounding and method.
fn invoices() -> Vec<Invoice> {
    let mut shapes: Vec<Vec<(i64, u8)>> = Vec::new();
    for a in AMOUNTS {
        for ka in 0..4u8 {
            shapes.push(vec![(a, ka)]);
            for b in AMOUNTS {
                for kb in 0..4u8 {
                    shapes.push(vec![(a, ka), (b, kb)]);
                }
            }
        }
    }
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    for len in [3usize, 5] {
        for _ in 0..300 {
            shapes.push((0..len).map(|_| (next(100_000) as i64, next(4) as u8)).collect());
        }
    }
    let mut out = Vec::new();
    for shape in &shapes {
        for r in 0..3u8 {
            for m in 0..4u8 {
                if shape.len() <= 2 && (r, m) != (0, 0) && (r + m) % 3 != 0 {
                    continue; // keep the exhaustive part to a third of the pairs
                }
                out.push(Invoice {
                    lines: shape.iter().map(|&(a, k)| kind_line(a, k)).collect(),
                    rounding: rounding_of(r),
                    method: method_of(m),
                });
            }
        }
    }
    out.push(Invoice { lines: vec![], rounding: Rounding::Down, method: Method::Separate });
    // `amount * 100` overflows: a panic on both sides.
    out.push(Invoice { lines: vec![kind_line(i64::MAX / 50, 2)], rounding: Rounding::Down, method: method_of(1) });
    out
}

#[test]
fn the_tax_agencys_examples_come_out_as_printed() {
    use invoice::summarize;
    let separate = |lines, rounding| Invoice { lines, rounding, method: Method::Separate };

    // 問57: a month's tax-inclusive invoice, 60,000 at 10% and 40,000 at 8%.
    let s = summarize(&separate(
        vec![line(5_000, Rate::Reduced, Pricing::Inclusive), line(8_000, Rate::Reduced, Pricing::Inclusive),
             line(27_000, Rate::Reduced, Pricing::Inclusive), line(2_000, Rate::Standard, Pricing::Inclusive),
             line(58_000, Rate::Standard, Pricing::Inclusive)],
        Rounding::Down,
    ))
    .unwrap();
    assert_eq!((s.standard_inclusive.base.value(), s.standard_inclusive.tax.value()), (60_000, 5_454));
    assert_eq!((s.reduced_inclusive.base.value(), s.reduced_inclusive.tax.value()), (40_000, 2_962));
    assert_eq!(s.total.value(), 100_000);

    // 問57 (参考): four sellers' tax-exclusive amounts, one rounding per rate.
    let s = summarize(&separate(
        vec![line(11_345, Rate::Standard, Pricing::Exclusive), line(9_987, Rate::Reduced, Pricing::Exclusive),
             line(12_549, Rate::Standard, Pricing::Exclusive), line(12_345, Rate::Reduced, Pricing::Exclusive)],
        Rounding::Down,
    ))
    .unwrap();
    assert_eq!((s.standard.base.value(), s.standard.tax.value()), (23_894, 2_389));
    assert_eq!((s.reduced.base.value(), s.reduced.tax.value()), (22_332, 1_786));
    // Rounding each line, which the Q&A rules out, would give 1,134 + 1,254 and 798 + 987.
    assert_ne!(s.standard.tax.value(), 1_134 + 1_254);
    assert_ne!(s.reduced.tax.value(), 798 + 987);

    // 問59: pen 218 (10%), coffee 120 (8%), tobacco 580 tax-inclusive (10%).
    let receipt = || vec![line(218, Rate::Standard, Pricing::Exclusive), line(120, Rate::Reduced, Pricing::Exclusive),
                          line(580, Rate::Standard, Pricing::Inclusive)];
    // Method 1: 580 × 100/110 ≒ 527, so 745 at 10% with tax 74, and 120 with tax 9.
    let one = summarize(&Invoice { lines: receipt(), rounding: Rounding::Down, method: Method::ToExclusive { conversion: Rounding::Down } }).unwrap();
    assert_eq!((one.standard.base.value(), one.standard.tax.value()), (745, 74));
    assert_eq!((one.reduced.base.value(), one.reduced.tax.value()), (120, 9));
    assert_eq!(one.total.value(), 948);
    // Method 2: 218 with tax 21, 580 containing 52, 120 with tax 9.
    let two = summarize(&separate(receipt(), Rounding::Down)).unwrap();
    assert_eq!((two.standard.base.value(), two.standard.tax.value()), (218, 21));
    assert_eq!((two.standard_inclusive.base.value(), two.standard_inclusive.tax.value()), (580, 52));
    assert_eq!((two.reduced.base.value(), two.reduced.tax.value()), (120, 9));
    assert_eq!(two.total.value(), 948);
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let differ: Vec<String> = support::quietly(|| {
        invoices()
            .iter()
            .filter_map(|inv| {
                let constrained = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| invoice::summarize(inv)));
                let (lines, rounding, method) = to_idiomatic(inv);
                let reference = std::panic::catch_unwind(|| idiomatic::summarize(&lines, rounding, method));
                let same = match (&constrained, &reference) {
                    (Ok(Ok(s)), Ok(Some(r))) => project(s) == *r,
                    (Ok(Err(invoice::InvoiceError::NoLines)), Ok(None)) => true,
                    (Err(_), Err(_)) => true,
                    _ => false,
                };
                (!same).then(|| support::Show::show(inv))
            })
            .collect()
    });
    assert!(differ.is_empty(), "{} differ, first: {:?}", differ.len(), differ.first());
}

#[test]
fn generated_invoice_matches_rust() {
    let invoices = invoices();
    let cases = support::quietly(|| invoices.iter().map(|inv| case!(invoice::summarize(inv))).collect::<Vec<_>>());
    assert!(cases.iter().any(|c| c.rust.starts_with("panic(")), "the overflow case must panic");
    assert!(cases.iter().any(|c| c.rust.starts_with("Err(InvoiceError::NoLines)")));
    support::assert_equivalent("invoice", SOURCE, &cases);
}
