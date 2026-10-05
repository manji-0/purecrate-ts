//! `examples/invoice`, consumption tax on a qualified invoice from the
//! National Tax Agency's Q&A (問57, 問59):
//!
//! - the Q&A's own worked examples come out as printed;
//! - totals are on one basis, converted once per rate (the model's choice;
//!   the Q&A leaves it open), and an ordinary tax-inclusive line among
//!   tax-exclusive ones needs a conversion;
//! - `tax` is the groups' taxes added and `total` the groups' bases and
//!   taxes, which after a conversion need not be the lines as priced;
//! - a figure past `i64::MAX` is `Err(Overflow)`, exactly at the boundary;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against, computing in `i128`) agree on every invoice below;
//! - the generated package agrees with Rust on every invoice below, whole
//!   `Summary` compared, overflow included.

use crate::support;

purecrate_canon::fixture!(mod invoice = "../../../examples/invoice/src/lib.rs");

/// The same rules without the subset's constraints. Not converted.
mod idiomatic {
    #[derive(Clone, Copy, PartialEq)]
    pub enum Rate {
        Standard,
        Reduced,
    }
    #[derive(Clone, Copy, PartialEq)]
    pub enum Pricing {
        Exclusive,
        Inclusive,
        FixedRetail,
    }
    #[derive(Clone, Copy)]
    pub enum Rounding {
        Down,
        Up,
        HalfUp,
    }
    #[derive(Clone, Copy)]
    pub enum Method {
        ToExclusive(Rounding),
        ToInclusive(Rounding),
        Separate,
    }
    pub struct Line {
        pub amount: i64,
        pub rate: Rate,
        pub pricing: Pricing,
    }
    #[derive(Debug, PartialEq)]
    pub struct Group {
        pub base: i64,
        pub tax: i64,
    }
    #[derive(Debug, PartialEq)]
    pub struct Summary {
        pub standard: Group,
        pub reduced: Group,
        pub standard_inclusive: Group,
        pub reduced_inclusive: Group,
        pub tax: i64,
        pub total: i64,
    }
    #[derive(Debug, PartialEq)]
    pub enum Error {
        NoLines,
        MixedPricing,
        Overflow,
    }

    impl Rate {
        fn percent(self) -> i128 {
            match self {
                Rate::Standard => 10,
                Rate::Reduced => 8,
            }
        }
    }

    fn divide(n: i128, d: i128, r: Rounding) -> i128 {
        match r {
            Rounding::Down => n / d,
            Rounding::Up => (n + d - 1) / d,
            Rounding::HalfUp => (2 * n + d) / (2 * d),
        }
    }

    /// Every sum and figure is exact in `i128`, and refused past `i64::MAX`.
    fn fit(v: i128) -> Result<i128, Error> {
        i64::try_from(v).map(i128::from).map_err(|_| Error::Overflow)
    }

    pub fn summarize(lines: &[Line], rounding: Rounding, method: Method) -> Result<Summary, Error> {
        if lines.is_empty() {
            return Err(Error::NoLines);
        }
        let has = |p: Pricing| lines.iter().any(|l| l.pricing == p);
        if matches!(method, Method::Separate) && has(Pricing::Exclusive) && has(Pricing::Inclusive) {
            return Err(Error::MixedPricing);
        }
        let groups = |rate: Rate| -> Result<(Group, Group), Error> {
            let p = rate.percent();
            let sum = |inclusive: bool| {
                fit(lines
                    .iter()
                    .filter(|l| l.rate == rate && (l.pricing != Pricing::Exclusive) == inclusive)
                    .map(|l| i128::from(l.amount))
                    .sum())
            };
            let (e, i) = (sum(false)?, sum(true)?);
            let (e, i) = match method {
                Method::ToExclusive(c) => (fit(e + fit(divide(i * 100, 100 + p, c))?)?, 0),
                Method::ToInclusive(c) => (0, fit(i + fit(divide(e * (100 + p), 100, c))?)?),
                Method::Separate => (e, i),
            };
            let group = |base: i128, d: i128| -> Result<Group, Error> {
                let tax = fit(divide(base * p, d, rounding))?;
                Ok(Group { base: base as i64, tax: tax as i64 })
            };
            Ok((group(e, 100)?, group(i, 100 + p)?))
        };
        let (s, si) = groups(Rate::Standard)?;
        let (r, ri) = groups(Rate::Reduced)?;
        let total = [s.base, s.tax, r.base, r.tax, si.base, ri.base].iter().map(|&v| i128::from(v)).sum();
        let total = fit(total)? as i64;
        let tax = fit([s.tax, r.tax, si.tax, ri.tax].iter().map(|&v| i128::from(v)).sum())? as i64;
        Ok(Summary { standard: s, reduced: r, standard_inclusive: si, reduced_inclusive: ri, tax, total })
    }
}

use invoice::{summarize, Invoice, InvoiceError, Line, Method, Pricing, Rate, Rounding, Summary, Yen};

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

/// 0 is `Separate`, 1..4 `ToExclusive`, 4..7 `ToInclusive`.
fn method_of(code: u8) -> Method {
    match code {
        0 => Method::Separate,
        c @ 1..=3 => Method::ToExclusive { conversion: rounding_of(c - 1) },
        c => Method::ToInclusive { conversion: rounding_of(c - 4) },
    }
}

/// Kind 0..6: rate by the low bit, pricing by `kind / 2`.
fn kind_line(amount: i64, kind: u8) -> Line {
    let rate = if kind.is_multiple_of(2) { Rate::Standard } else { Rate::Reduced };
    let pricing = match kind / 2 {
        0 => Pricing::Exclusive,
        1 => Pricing::Inclusive,
        _ => Pricing::FixedRetail,
    };
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
            rate: match l.rate {
                Rate::Standard => idiomatic::Rate::Standard,
                Rate::Reduced => idiomatic::Rate::Reduced,
            },
            pricing: match l.pricing {
                Pricing::Exclusive => idiomatic::Pricing::Exclusive,
                Pricing::Inclusive => idiomatic::Pricing::Inclusive,
                Pricing::FixedRetail => idiomatic::Pricing::FixedRetail,
            },
        })
        .collect();
    let method = match &inv.method {
        Method::Separate => idiomatic::Method::Separate,
        Method::ToExclusive { conversion } => idiomatic::Method::ToExclusive(rounding(conversion)),
        Method::ToInclusive { conversion } => idiomatic::Method::ToInclusive(rounding(conversion)),
    };
    (lines, rounding(&inv.rounding), method)
}

fn project(s: &Summary) -> idiomatic::Summary {
    let g = |g: &invoice::Group| idiomatic::Group { base: g.base.value(), tax: g.tax.value() };
    idiomatic::Summary {
        standard: g(&s.standard),
        reduced: g(&s.reduced),
        standard_inclusive: g(&s.standard_inclusive),
        reduced_inclusive: g(&s.reduced_inclusive),
        tax: s.tax.value(),
        total: s.total.value(),
    }
}

const AMOUNTS: [i64; 8] = [0, 1, 5, 99, 218, 580, 23_894, 60_000];

/// The largest 10% amount that fits rounded down both ways: tax-exclusive,
/// it plus its tax is exactly `i64::MAX`; converted to tax-inclusive
/// (× 110/100), so is the result.
const LARGEST: i64 = 8_384_883_669_867_978_007;

/// Amounts near the overflow boundary of some sum or figure.
const LARGE: [i64; 6] = [i64::MAX, i64::MAX - 1, i64::MAX / 2, LARGEST, LARGEST + 1, i64::MAX / 10];

/// Every invoice of one or two lines over `AMOUNTS` and the six kinds, under
/// every method (each with one rounding); pseudo-random three- and five-line
/// ones, and ones of large amounts, under every rounding and method.
fn invoices() -> Vec<Invoice> {
    let mut small: Vec<Vec<(i64, u8)>> = Vec::new();
    let mut every: Vec<Vec<(i64, u8)>> = Vec::new();
    for a in AMOUNTS {
        for ka in 0..6u8 {
            small.push(vec![(a, ka)]);
            for b in AMOUNTS {
                for kb in 0..6u8 {
                    small.push(vec![(a, ka), (b, kb)]);
                }
            }
        }
    }
    let mut rng = support::Rng::new(0x2545_f491_4f6c_dd1d);
    for len in [3usize, 5] {
        for _ in 0..200 {
            every.push((0..len).map(|_| (rng.below(100_000) as i64, rng.below(6) as u8)).collect());
        }
    }
    for _ in 0..150 {
        let len = 1 + rng.below(3) as usize;
        let shape = (0..len)
            .map(|_| {
                let a =
                    if rng.below(4) == 0 { rng.below(i64::MAX as u64) as i64 } else { LARGE[rng.below(6) as usize] };
                (a, rng.below(6) as u8)
            })
            .collect();
        every.push(shape);
    }
    let mut out = Vec::new();
    let mut push = |shape: &Vec<(i64, u8)>, r: u8, m: u8| {
        out.push(Invoice {
            lines: shape.iter().map(|&(a, k)| kind_line(a, k)).collect(),
            rounding: rounding_of(r),
            method: method_of(m),
        });
    };
    for shape in &small {
        for m in 0..7u8 {
            push(shape, (3 - m % 3) % 3, m);
        }
    }
    for shape in &every {
        for r in 0..3u8 {
            for m in 0..7u8 {
                push(shape, r, m);
            }
        }
    }
    out.push(Invoice { lines: vec![], rounding: Rounding::Down, method: Method::Separate });
    out
}

fn base_and_tax(g: &invoice::Group) -> (i64, i64) {
    (g.base.value(), g.tax.value())
}

/// 問59's receipt: pen 218 (10%), coffee 120 (8%), tobacco 580 (10%).
fn receipt(tobacco: Pricing) -> Vec<Line> {
    vec![
        line(218, Rate::Standard, Pricing::Exclusive),
        line(120, Rate::Reduced, Pricing::Exclusive),
        line(580, Rate::Standard, tobacco),
    ]
}

#[test]
fn the_tax_agencys_examples_come_out_as_printed() {
    let invoice = |lines, rounding, method| Invoice { lines, rounding, method };

    // 問57: a month's tax-inclusive invoice, 60,000 at 10% and 40,000 at 8%,
    // 60,000 × 10/110 ≒ 5,454 and 40,000 × 8/108 ≒ 2,962. One basis, so
    // nothing to convert.
    let month = || {
        vec![
            line(5_000, Rate::Reduced, Pricing::Inclusive),
            line(8_000, Rate::Reduced, Pricing::Inclusive),
            line(27_000, Rate::Reduced, Pricing::Inclusive),
            line(2_000, Rate::Standard, Pricing::Inclusive),
            line(58_000, Rate::Standard, Pricing::Inclusive),
        ]
    };
    for method in [Method::Separate, Method::ToInclusive { conversion: Rounding::Up }] {
        let s = summarize(&invoice(month(), Rounding::Down, method)).unwrap();
        assert_eq!(base_and_tax(&s.standard_inclusive), (60_000, 5_454));
        assert_eq!(base_and_tax(&s.reduced_inclusive), (40_000, 2_962));
        assert_eq!(s.tax.value(), 8_416);
        assert_eq!(s.total.value(), 100_000);
    }

    // 問57 (参考): four sellers' tax-exclusive amounts, one rounding per rate.
    let s = summarize(&invoice(
        vec![
            line(11_345, Rate::Standard, Pricing::Exclusive),
            line(9_987, Rate::Reduced, Pricing::Exclusive),
            line(12_549, Rate::Standard, Pricing::Exclusive),
            line(12_345, Rate::Reduced, Pricing::Exclusive),
        ],
        Rounding::Down,
        Method::Separate,
    ))
    .unwrap();
    assert_eq!(base_and_tax(&s.standard), (23_894, 2_389));
    assert_eq!(base_and_tax(&s.reduced), (22_332, 1_786));
    // Rounding each line, which 問57 (注) rules out, would give 1,134 + 1,254 and 798 + 987.
    assert_ne!(s.standard.tax.value(), 1_134 + 1_254);
    assert_ne!(s.reduced.tax.value(), 798 + 987);

    // 問59 ①, unified tax-exclusive: 580 × 100/110 ≒ 527, so 745 at 10% with
    // tax 74, and 120 with tax 9. Any tax-inclusive price may be converted.
    for tobacco in [Pricing::FixedRetail, Pricing::Inclusive] {
        let to_exclusive = Method::ToExclusive { conversion: Rounding::Down };
        let one = summarize(&invoice(receipt(tobacco), Rounding::Down, to_exclusive)).unwrap();
        assert_eq!(base_and_tax(&one.standard), (745, 74));
        assert_eq!(base_and_tax(&one.reduced), (120, 9));
        assert_eq!(base_and_tax(&one.standard_inclusive), (0, 0));
        assert_eq!(one.tax.value(), 74 + 9);
        assert_eq!(one.total.value(), 948);
    }
    // 問59 ②, the fixed price not converted: 218 with tax 21, 580 containing
    // 52, 120 with tax 9.
    let two = summarize(&invoice(receipt(Pricing::FixedRetail), Rounding::Down, Method::Separate)).unwrap();
    assert_eq!(base_and_tax(&two.standard), (218, 21));
    assert_eq!(base_and_tax(&two.standard_inclusive), (580, 52));
    assert_eq!(base_and_tax(&two.reduced), (120, 9));
    assert_eq!(two.tax.value(), 21 + 52 + 9);
    assert_eq!(two.total.value(), 948);

    // The conversion's rounding is the seller's; rounded up, 580 × 100/110
    // ≒ 528, so 746 at 10% with tax 74, and the total is 949, not the
    // receipt's 948: `total` is the groups' figures, not the lines as priced.
    let up = Method::ToExclusive { conversion: Rounding::Up };
    let s = summarize(&invoice(receipt(Pricing::FixedRetail), Rounding::Down, up)).unwrap();
    assert_eq!(base_and_tax(&s.standard), (746, 74));
    assert_eq!(s.tax.value(), 74 + 9);
    assert_eq!(s.total.value(), 949);
}

/// 問59: the totals are "いずれかに統一して", all tax-exclusive or all
/// tax-inclusive. An ordinary tax-inclusive price among tax-exclusive ones is
/// converted, never taxed apart; that exception is for fixed retail prices.
#[test]
fn one_basis_unless_the_price_is_fixed() {
    let separate = |lines| Invoice { lines, rounding: Rounding::Down, method: Method::Separate };
    assert!(matches!(summarize(&separate(receipt(Pricing::Inclusive))), Err(InvoiceError::MixedPricing)));
    // Mixed across rates is still mixed: the whole invoice is on one basis.
    let across = vec![line(218, Rate::Standard, Pricing::Exclusive), line(120, Rate::Reduced, Pricing::Inclusive)];
    assert!(matches!(summarize(&separate(across)), Err(InvoiceError::MixedPricing)));
    // Tax-inclusive with fixed prices is one basis: 1,580 contains 143.
    let inclusive =
        vec![line(1_000, Rate::Standard, Pricing::Inclusive), line(580, Rate::Standard, Pricing::FixedRetail)];
    let s = summarize(&separate(inclusive)).unwrap();
    assert_eq!(base_and_tax(&s.standard_inclusive), (1_580, 143));

    // Unified tax-inclusive: 218 × 110/100 ≒ 239, plus 580, is 819 at 10%
    // containing 74; 120 × 108/100 ≒ 129 containing 9. The total is 948 as
    // on the Q&A's two receipts.
    let s = summarize(&Invoice {
        lines: receipt(Pricing::FixedRetail),
        rounding: Rounding::Down,
        method: Method::ToInclusive { conversion: Rounding::Down },
    })
    .unwrap();
    assert_eq!(base_and_tax(&s.standard_inclusive), (819, 74));
    assert_eq!(base_and_tax(&s.reduced_inclusive), (129, 9));
    assert_eq!(base_and_tax(&s.standard), (0, 0));
    assert_eq!(s.total.value(), 948);
}

/// The model converts once per rate on the total (its choice: 問59 leaves
/// the conversion's rounding to the seller and does not say how lines are
/// grouped), so its rounding does not add up over many small lines.
#[test]
fn many_small_lines_convert_once() {
    // Eleven 1-yen tax-inclusive lines at 10%: 11 × 100/110 = 10, tax 1.
    // Each line converted (1 × 100/110 ≒ 0) would give 0 with tax 0.
    let s = summarize(&Invoice {
        lines: (0..11).map(|_| line(1, Rate::Standard, Pricing::Inclusive)).collect(),
        rounding: Rounding::Down,
        method: Method::ToExclusive { conversion: Rounding::Down },
    })
    .unwrap();
    assert_eq!(base_and_tax(&s.standard), (10, 1));
    assert_eq!(s.total.value(), 11);

    // Ten 1-yen tax-exclusive lines at 8%, converted rounding up:
    // 10 × 108/100 = 10.8 ≒ 11. Each line converted (1.08 ≒ 2) would give 20.
    let s = summarize(&Invoice {
        lines: (0..10).map(|_| line(1, Rate::Reduced, Pricing::Exclusive)).collect(),
        rounding: Rounding::Down,
        method: Method::ToInclusive { conversion: Rounding::Up },
    })
    .unwrap();
    assert_eq!(base_and_tax(&s.reduced_inclusive), (11, 0));
}

/// A sum or figure past `i64::MAX` is `Err(Overflow)`, not a panic, and only
/// then: an intermediate product does not overflow early.
#[test]
fn overflow_is_an_error_at_the_boundary() {
    let at = |lines: Vec<Line>, rounding, method| Invoice { lines, rounding, method };
    let largest = |extra: i64| vec![line(LARGEST + extra, Rate::Standard, Pricing::Exclusive)];

    // Tax-exclusive: LARGEST plus its tax, rounded down, is i64::MAX; one
    // more yen is not.
    let to_exclusive = || Method::ToExclusive { conversion: Rounding::Down };
    let s = summarize(&at(largest(0), Rounding::Down, to_exclusive())).unwrap();
    assert_eq!(base_and_tax(&s.standard), (LARGEST, LARGEST / 10));
    assert_eq!(s.total.value(), i64::MAX);
    assert!(matches!(summarize(&at(largest(1), Rounding::Down, to_exclusive())), Err(InvoiceError::Overflow)));
    // Rounding the tax up instead passes i64::MAX at LARGEST already.
    assert!(matches!(summarize(&at(largest(0), Rounding::Up, to_exclusive())), Err(InvoiceError::Overflow)));

    // Converted to tax-inclusive: LARGEST × 110/100 ≒ i64::MAX; one more yen
    // is past it.
    let to_inclusive = || Method::ToInclusive { conversion: Rounding::Down };
    let s = summarize(&at(largest(0), Rounding::Down, to_inclusive())).unwrap();
    assert_eq!(s.standard_inclusive.base.value(), i64::MAX);
    assert_eq!(s.total.value(), i64::MAX);
    assert!(matches!(summarize(&at(largest(1), Rounding::Down, to_inclusive())), Err(InvoiceError::Overflow)));

    // i64::MAX tax-inclusive: the tax (MAX × 10/110) and the conversion
    // (MAX × 100/110) fit, though MAX × 10 and MAX × 100 do not.
    let max = || vec![line(i64::MAX, Rate::Standard, Pricing::Inclusive), line(0, Rate::Standard, Pricing::Inclusive)];
    let s = summarize(&at(max(), Rounding::HalfUp, Method::Separate)).unwrap();
    assert_eq!(base_and_tax(&s.standard_inclusive), (i64::MAX, 838_488_366_986_797_801));
    assert_eq!(s.total.value(), i64::MAX);
    let s = summarize(&at(max(), Rounding::HalfUp, to_exclusive())).unwrap();
    assert_eq!(base_and_tax(&s.standard), (8_384_883_669_867_978_006, 838_488_366_986_797_801));
    assert_eq!(s.total.value(), i64::MAX);

    // A rate's lines summing past i64::MAX.
    let over = vec![line(i64::MAX, Rate::Standard, Pricing::Inclusive), line(1, Rate::Standard, Pricing::Inclusive)];
    assert!(matches!(summarize(&at(over, Rounding::Down, Method::Separate)), Err(InvoiceError::Overflow)));
    // The invoice's total past i64::MAX though each rate fits.
    let across = vec![line(i64::MAX, Rate::Standard, Pricing::Inclusive), line(1, Rate::Reduced, Pricing::Inclusive)];
    assert!(matches!(summarize(&at(across, Rounding::Down, Method::Separate)), Err(InvoiceError::Overflow)));
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let differ: Vec<String> = support::quietly(|| {
        invoices()
            .iter()
            .filter_map(|inv| {
                let constrained = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| summarize(inv)));
                let (lines, rounding, method) = to_idiomatic(inv);
                let reference = idiomatic::summarize(&lines, rounding, method);
                let same = match (&constrained, &reference) {
                    (Ok(Ok(s)), Ok(r)) => project(s) == *r,
                    (Ok(Err(InvoiceError::NoLines)), Err(idiomatic::Error::NoLines)) => true,
                    (Ok(Err(InvoiceError::MixedPricing)), Err(idiomatic::Error::MixedPricing)) => true,
                    (Ok(Err(InvoiceError::Overflow)), Err(idiomatic::Error::Overflow)) => true,
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
    assert!(!cases.iter().any(|c| c.rust.starts_with("panic(")), "nothing panics");
    for err in ["NoLines", "MixedPricing", "Overflow"] {
        let prefix = format!("Err(InvoiceError::{err})");
        assert!(cases.iter().any(|c| c.rust.starts_with(&prefix)), "no {prefix} case");
    }
    support::assert_equivalent("invoice", invoice::SOURCE, &cases);
}
