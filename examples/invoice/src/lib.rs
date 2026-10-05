// Consumption tax on a qualified invoice (適格請求書), as the National Tax
// Agency states it (インボイスQ&A 問57 and 問59, revised 2024-04; read
// 2026-10-05):
//
// - 問57: the tax is rounded to a yen once per rate, 10% (standard) and 8%
//   (reduced), per invoice (消令70の10, 基通1-8-15), on the total of that
//   rate's amounts. Rounding up, down, or half up is the seller's choice.
//   Rounding each line and adding the results is not allowed; this type has
//   no way to do it.
// - 問59: the totals per rate are all tax-exclusive (tax = total × rate) or
//   all tax-inclusive (tax = total × rate / (100 + rate)), one or the other
//   for the whole invoice. Lines priced on the other basis are converted
//   first; that conversion's rounding is the seller's choice and is not the
//   tax rounding. `Method::ToExclusive` and `Method::ToInclusive` do this.
//   The conversion is made once per rate on the total of the lines to
//   convert, not line by line, so its rounding cannot add up over many small
//   lines either.
// - 問59, ただし: a price the law fixes tax-inclusive (tobacco, designated
//   garbage bags, goods under resale price maintenance;
//   `Pricing::FixedRetail`) may instead stay unconverted among tax-exclusive
//   lines: its rate's fixed prices are totalled and taxed apart
//   (`Method::Separate`), the one case with two roundings in a rate. Under
//   `Separate` an ordinary tax-inclusive line among tax-exclusive ones is
//   refused (`MixedPricing`): the exception covers fixed prices only.
//
// Amounts are yen, an integer; there is no decimal. A sum or a figure that
// does not fit in an `i64` is refused (`Overflow`), not wrapped or panicked
// on.
//
// Not modeled: the invoice's other required items (the issuer's
// registration number, T and 13 digits, the date, the parties), and lines
// that are exempt, non-taxable, or zero-rated.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize)]
#[serde(try_from = "i64")]
pub struct Yen(i64);

impl Yen {
    pub fn new(value: i64) -> Result<Yen, InvoiceError> {
        if value < 0 {
            return Err(InvoiceError::NegativeAmount);
        }
        Ok(Yen(value))
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for Yen {
    type Error = InvoiceError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Yen::new(value)
    }
}

#[derive(Serialize, Deserialize)]
pub enum Rate {
    Standard,
    Reduced,
}

#[derive(Serialize, Deserialize)]
pub enum Pricing {
    Exclusive,
    Inclusive,
    /// Tax-inclusive at a price the law fixes (問59, ただし).
    FixedRetail,
}

#[derive(Serialize, Deserialize)]
pub enum Rounding {
    Down,
    Up,
    HalfUp,
}

/// 問59: the one basis the invoice's totals are on.
#[derive(Serialize, Deserialize)]
pub enum Method {
    /// Every total tax-exclusive; each rate's tax-inclusive lines are
    /// totalled and converted once.
    ToExclusive { conversion: Rounding },
    /// Every total tax-inclusive; each rate's tax-exclusive lines are
    /// totalled and converted once.
    ToInclusive { conversion: Rounding },
    /// Nothing converted: fixed retail prices are totalled and taxed apart
    /// from tax-exclusive lines. Tax-inclusive lines are allowed only with no
    /// tax-exclusive line.
    Separate,
}

#[derive(Serialize, Deserialize)]
pub struct Line {
    pub amount: Yen,
    pub rate: Rate,
    pub pricing: Pricing,
}

#[derive(Serialize, Deserialize)]
pub struct Invoice {
    pub lines: Vec<Line>,
    pub rounding: Rounding,
    pub method: Method,
}

/// One rate and basis: the total of its amounts and the tax on it.
#[derive(Serialize, Deserialize)]
pub struct Group {
    pub base: Yen,
    pub tax: Yen,
}

#[derive(Serialize, Deserialize)]
pub struct Summary {
    pub standard: Group,
    pub reduced: Group,
    /// Tax-inclusive totals; the tax is contained in `base`.
    pub standard_inclusive: Group,
    pub reduced_inclusive: Group,
    pub total: Yen,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum InvoiceError {
    NegativeAmount,
    NoLines,
    MixedPricing,
    Overflow,
}

impl fmt::Display for InvoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InvoiceError::NegativeAmount => "amount must not be negative",
            InvoiceError::NoLines => "an invoice needs a line",
            InvoiceError::MixedPricing => "tax-inclusive and tax-exclusive lines need a conversion",
            InvoiceError::Overflow => "an amount is too large",
        })
    }
}

impl std::error::Error for InvoiceError {}

fn percent(rate: &Rate) -> i64 {
    match rate {
        Rate::Standard => 10,
        Rate::Reduced => 8,
    }
}

fn is_inclusive(pricing: &Pricing) -> bool {
    match pricing {
        Pricing::Exclusive => false,
        Pricing::Inclusive | Pricing::FixedRetail => true,
    }
}

fn add(a: i64, b: i64) -> Result<i64, InvoiceError> {
    a.checked_add(b).ok_or(InvoiceError::Overflow)
}

/// `n × m / d` rounded to an integer, for `n >= 0` and `0 < m, d <= 110`;
/// `Overflow` only when the result does not fit.
fn scale(n: i64, m: i64, d: i64, rounding: &Rounding) -> Result<i64, InvoiceError> {
    let whole = (n / d).checked_mul(m).ok_or(InvoiceError::Overflow)?;
    let rest = n % d * m;
    let part = match rounding {
        Rounding::Down => rest / d,
        Rounding::Up => (rest + d - 1) / d,
        Rounding::HalfUp => (2 * rest + d) / (2 * d),
    };
    add(whole, part)
}

/// The total of `rate`'s tax-inclusive lines, or of its tax-exclusive ones.
fn subtotal(invoice: &Invoice, rate: &Rate, inclusive: bool) -> Result<i64, InvoiceError> {
    let mut sum: i64 = 0;
    for line in invoice.lines.iter() {
        if percent(&line.rate) == percent(rate) && is_inclusive(&line.pricing) == inclusive {
            sum = add(sum, line.amount.0)?;
        }
    }
    Ok(sum)
}

/// A total and its tax, rounded once (消令70の10).
fn taxed(base: i64, rate: &Rate, inclusive: bool, rounding: &Rounding) -> Result<Group, InvoiceError> {
    let p = percent(rate);
    let d = if inclusive { 100 + p } else { 100 };
    Ok(Group { base: Yen(base), tax: Yen(scale(base, p, d, rounding)?) })
}

/// One rate's tax-exclusive group and tax-inclusive group, each taxed once.
fn groups(invoice: &Invoice, rate: Rate) -> Result<(Group, Group), InvoiceError> {
    let p = percent(&rate);
    let exclusive = subtotal(invoice, &rate, false)?;
    let inclusive = subtotal(invoice, &rate, true)?;
    match &invoice.method {
        Method::ToExclusive { conversion } => {
            let converted = scale(inclusive, 100, 100 + p, conversion)?;
            both(invoice, &rate, add(exclusive, converted)?, 0)
        }
        Method::ToInclusive { conversion } => {
            let converted = scale(exclusive, 100 + p, 100, conversion)?;
            both(invoice, &rate, 0, add(inclusive, converted)?)
        }
        Method::Separate => both(invoice, &rate, exclusive, inclusive),
    }
}

fn both(invoice: &Invoice, rate: &Rate, exclusive: i64, inclusive: i64) -> Result<(Group, Group), InvoiceError> {
    Ok((taxed(exclusive, rate, false, &invoice.rounding)?, taxed(inclusive, rate, true, &invoice.rounding)?))
}

pub fn summarize(invoice: &Invoice) -> Result<Summary, InvoiceError> {
    if invoice.lines.is_empty() {
        return Err(InvoiceError::NoLines);
    }
    let exclusive = invoice.lines.iter().any(|line| !is_inclusive(&line.pricing));
    let inclusive = invoice.lines.iter().any(|line| matches!(line.pricing, Pricing::Inclusive));
    if matches!(invoice.method, Method::Separate) && exclusive && inclusive {
        return Err(InvoiceError::MixedPricing);
    }
    let (standard, standard_inclusive) = groups(invoice, Rate::Standard)?;
    let (reduced, reduced_inclusive) = groups(invoice, Rate::Reduced)?;
    let mut total = add(standard.base.0, standard.tax.0)?;
    total = add(total, reduced.base.0)?;
    total = add(total, reduced.tax.0)?;
    total = add(total, standard_inclusive.base.0)?;
    total = add(total, reduced_inclusive.base.0)?;
    Ok(Summary { standard, reduced, standard_inclusive, reduced_inclusive, total: Yen(total) })
}
