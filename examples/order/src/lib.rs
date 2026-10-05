// An order's lifecycle: lines are added and removed while it is a draft, it
// is placed at the total of its lines, paid that exact amount, and shipped
// with a tracking number; a draft or a placed order can be cancelled.
//
// Invariants, held by construction: `Yen`, `Sku`, `Line`, and `Order` have
// private fields, so a value of each comes only from its checked function.
// - A `Yen` is not negative, and a `Sku` is not empty.
// - A `Line` has a quantity above zero.
// - An `Order` has at most one line per SKU (`step` merges a repeated SKU
//   at the same unit price, and refuses another price), its total is the
//   sum of its lines, and a shipped order's tracking number is not empty.
// - A cancelled order's reason fits where it was cancelled: a draft is
//   cancelled only `ByCustomer`, as no payment was attempted on it, so
//   `PaymentFailed` is refused there (`ReasonMismatch`); a placed order may
//   be cancelled for either reason.
//
// Policies, chosen here rather than taken from a standard:
// - A unit price of zero is a price (a free item still makes a line), so a
//   placed order may total zero and is paid with `Pay(0)`.
// - `Pay` must be exactly the total: a partial or an over payment is
//   refused with `AmountMismatch`, and amounts compare as integers.
// - A paid or shipped order cannot be cancelled: undoing a payment is a
//   refund, which this model leaves out.
// - A SKU is any non-empty string, compared exactly (case and spacing
//   count); a tracking number is any non-empty string.
// - Quantities and totals past `u32` / `i64` are refused (`Overflow`).
//
// `step` reads the order and returns the next one, so a refused command
// leaves the caller holding the order it had.

#[derive(Clone)]
pub struct Yen(i64);

impl Yen {
    pub fn new(amount: i64) -> Result<Yen, OrderError> {
        if amount < 0 {
            Err(OrderError::NegativeAmount)
        } else {
            Ok(Yen(amount))
        }
    }

    pub fn value(&self) -> i64 {
        self.0
    }
}

#[derive(Clone)]
pub struct Sku(String);

impl Sku {
    pub fn new(code: String) -> Result<Sku, OrderError> {
        if code.is_empty() {
            Err(OrderError::EmptySku)
        } else {
            Ok(Sku(code))
        }
    }

    pub fn code(&self) -> &str {
        &self.0
    }
}

#[derive(Clone)]
pub struct Line {
    sku: Sku,
    unit_price: Yen,
    qty: u32,
}

impl Line {
    pub fn new(sku: Sku, unit_price: Yen, qty: u32) -> Result<Line, OrderError> {
        if qty == 0 {
            Err(OrderError::QtyZero)
        } else {
            Ok(Line { sku, unit_price, qty })
        }
    }

    pub fn sku(&self) -> &Sku {
        &self.sku
    }

    pub fn unit_price(&self) -> &Yen {
        &self.unit_price
    }

    pub fn qty(&self) -> u32 {
        self.qty
    }
}

#[derive(Clone)]
pub enum CancelReason {
    ByCustomer,
    PaymentFailed,
}

/// Where an order is, read through `Order::status`. Only `Order` holds one,
/// so the invariants above hold for every `Status` an `Order` returns.
pub enum Status {
    Draft { lines: Vec<Line> },
    Placed { lines: Vec<Line>, total: Yen },
    Paid { lines: Vec<Line>, total: Yen },
    Shipped { lines: Vec<Line>, total: Yen, tracking: String },
    Cancelled { reason: CancelReason },
}

pub struct Order(Status);

impl Order {
    /// An empty draft, where every order starts.
    pub fn draft() -> Order {
        Order(Status::Draft { lines: Vec::new() })
    }

    pub fn status(&self) -> &Status {
        &self.0
    }
}

pub enum Command {
    AddLine(Line),
    RemoveSku(Sku),
    Place,
    Pay(Yen),
    Ship(String),
    Cancel(CancelReason),
}

pub enum OrderError {
    QtyZero,
    UnknownSku,
    Empty,
    AmountMismatch { expected: Yen, got: Yen },
    EmptyTracking,
    InvalidTransition,
    NegativeAmount,
    EmptySku,
    /// The same SKU added again at another unit price.
    PriceMismatch,
    /// A quantity or a total past what its integer holds.
    Overflow,
    /// A cancel reason that does not fit the status: `PaymentFailed` on a
    /// draft, where no payment was attempted.
    ReasonMismatch,
}

/// Adds `line`, or adds its quantity to the line of the same SKU, which
/// must have the same unit price.
fn add_line(lines: &Vec<Line>, line: Line) -> Result<Vec<Line>, OrderError> {
    let mut out: Vec<Line> = Vec::new();
    let mut merged = false;
    for l in lines {
        if l.sku.0 != line.sku.0 {
            out.push(l.clone());
        } else if l.unit_price.0 != line.unit_price.0 {
            return Err(OrderError::PriceMismatch);
        } else {
            let qty = l.qty.checked_add(line.qty).ok_or(OrderError::Overflow)?;
            out.push(Line { qty, ..l.clone() });
            merged = true;
        }
    }
    if !merged {
        out.push(line);
    }
    Ok(out)
}

/// Removes the line of `sku`; there is at most one, as `add_line` merges.
fn remove_sku(lines: &Vec<Line>, sku: &Sku) -> Result<Vec<Line>, OrderError> {
    if !lines.iter().any(|l| l.sku.0 == sku.0) {
        return Err(OrderError::UnknownSku);
    }
    Ok(lines.iter().filter(|l| l.sku.0 != sku.0).cloned().collect())
}

/// The sum of the lines' amounts.
pub fn total(lines: &Vec<Line>) -> Result<Yen, OrderError> {
    let mut sum = 0i64;
    for line in lines {
        let amount = line.unit_price.0.checked_mul(i64::from(line.qty)).ok_or(OrderError::Overflow)?;
        sum = sum.checked_add(amount).ok_or(OrderError::Overflow)?;
    }
    Ok(Yen(sum))
}

/// The order after `cmd`, or why `cmd` does not apply; `order` is unchanged
/// either way.
pub fn step(order: &Order, cmd: Command) -> Result<Order, OrderError> {
    let next = match (&order.0, cmd) {
        (Status::Draft { lines }, Command::AddLine(line)) => Status::Draft { lines: add_line(lines, line)? },
        (Status::Draft { lines }, Command::RemoveSku(sku)) => Status::Draft { lines: remove_sku(lines, &sku)? },
        (Status::Draft { lines }, Command::Place) => {
            if lines.is_empty() {
                return Err(OrderError::Empty);
            }
            Status::Placed { lines: lines.clone(), total: total(lines)? }
        }
        (Status::Placed { total, .. }, Command::Pay(amount)) if amount.0 != total.0 => {
            return Err(OrderError::AmountMismatch { expected: total.clone(), got: amount });
        }
        (Status::Placed { lines, total }, Command::Pay(_)) => Status::Paid { lines: lines.clone(), total: total.clone() },
        (Status::Paid { .. }, Command::Ship(tracking)) if tracking.is_empty() => return Err(OrderError::EmptyTracking),
        (Status::Paid { lines, total }, Command::Ship(tracking)) => {
            Status::Shipped { lines: lines.clone(), total: total.clone(), tracking }
        }
        (Status::Draft { .. }, Command::Cancel(CancelReason::PaymentFailed)) => return Err(OrderError::ReasonMismatch),
        (Status::Draft { .. } | Status::Placed { .. }, Command::Cancel(reason)) => Status::Cancelled { reason },
        _ => return Err(OrderError::InvalidTransition),
    };
    Ok(Order(next))
}
