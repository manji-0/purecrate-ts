// Consumption tax on a qualified invoice (適格請求書), as the National Tax
// Agency states it (インボイスQ&A 問57 and 問59, revised 2024-04; read
// 2026-09-29):
//
// - The tax is computed per rate, 10% (standard) and 8% (reduced), from the
//   total of that rate's amounts, and rounded to a yen once per rate per
//   invoice (消令70の10, 基通1-8-15). Rounding up, down, or half up is the
//   seller's choice. Rounding each line and adding the results is not
//   allowed; this type has no way to do it.
// - Amounts are tax-exclusive (tax = total × rate) or tax-inclusive (tax =
//   total × rate / (100 + rate)).
// - When one receipt mixes tax-inclusive lines (tobacco, at a fixed retail
//   price) with tax-exclusive ones (問59), either every inclusive line is
//   first converted to an exclusive amount (method 1; that conversion's
//   rounding is also the seller's choice and is not the tax rounding), or
//   the inclusive lines are totalled and taxed apart (method 2).
//
// Amounts are yen, an integer; there is no decimal.
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
}

#[derive(Serialize, Deserialize)]
pub enum Rounding {
    Down,
    Up,
    HalfUp,
}

/// 問59: what to do with tax-inclusive lines among tax-exclusive ones.
#[derive(Serialize, Deserialize)]
pub enum Method {
    /// Method 2: total and tax the inclusive lines apart.
    Separate,
    /// Method 1: convert each inclusive line to an exclusive amount first.
    ToExclusive { conversion: Rounding },
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

/// One rate and pricing: the total of its amounts and the tax on it.
#[derive(Serialize, Deserialize)]
pub struct Group {
    pub base: Yen,
    pub tax: Yen,
}

#[derive(Serialize, Deserialize)]
pub struct Summary {
    pub standard: Group,
    pub reduced: Group,
    /// Inclusive lines under method 2; the tax is contained in `base`.
    pub standard_inclusive: Group,
    pub reduced_inclusive: Group,
    pub total: Yen,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum InvoiceError {
    NegativeAmount,
    NoLines,
}

impl fmt::Display for InvoiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InvoiceError::NegativeAmount => "amount must not be negative",
            InvoiceError::NoLines => "an invoice needs a line",
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

/// `n / d` rounded to an integer, for `n >= 0` and `d > 0`.
fn divide(n: i64, d: i64, rounding: &Rounding) -> i64 {
    match rounding {
        Rounding::Down => n / d,
        Rounding::Up => (n + d - 1) / d,
        Rounding::HalfUp => (2 * n + d) / (2 * d),
    }
}

/// What `line` adds to the total of `rate`'s group; `apart` is the group of
/// inclusive lines under method 2.
fn share(line: &Line, rate: &Rate, apart: bool, method: &Method) -> i64 {
    match (&line.pricing, method) {
        _ if percent(&line.rate) != percent(rate) => 0,
        (Pricing::Exclusive, _) if !apart => line.amount.0,
        (Pricing::Inclusive, Method::Separate) if apart => line.amount.0,
        (Pricing::Inclusive, Method::ToExclusive { conversion }) if !apart => {
            divide(line.amount.0 * 100, 100 + percent(rate), conversion)
        }
        _ => 0,
    }
}

/// One group's total and its tax, rounded once (消令70の10).
fn taxed(invoice: &Invoice, rate: Rate, apart: bool) -> Group {
    let base: i64 = invoice.lines.iter().map(|line| share(line, &rate, apart, &invoice.method)).sum();
    let p = percent(&rate);
    let d = if apart { 100 + p } else { 100 };
    Group { base: Yen(base), tax: Yen(divide(base * p, d, &invoice.rounding)) }
}

pub fn summarize(invoice: &Invoice) -> Result<Summary, InvoiceError> {
    if invoice.lines.is_empty() {
        return Err(InvoiceError::NoLines);
    }
    let standard = taxed(invoice, Rate::Standard, false);
    let reduced = taxed(invoice, Rate::Reduced, false);
    let standard_inclusive = taxed(invoice, Rate::Standard, true);
    let reduced_inclusive = taxed(invoice, Rate::Reduced, true);
    let total = standard.base.0
        + standard.tax.0
        + reduced.base.0
        + reduced.tax.0
        + standard_inclusive.base.0
        + reduced_inclusive.base.0;
    Ok(Summary { standard, reduced, standard_inclusive, reduced_inclusive, total: Yen(total) })
}
