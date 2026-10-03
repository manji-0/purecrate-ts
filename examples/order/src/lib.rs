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

pub struct Sku(String);

impl Sku {
    pub fn new(code: String) -> Result<Sku, OrderError> {
        if code.is_empty() {
            Err(OrderError::EmptySku)
        } else {
            Ok(Sku(code))
        }
    }
}

pub struct Line {
    pub sku: Sku,
    pub unit_price: Yen,
    pub qty: u32,
}

pub enum CancelReason {
    ByCustomer,
    PaymentFailed,
}

pub enum Order {
    Draft { lines: Vec<Line> },
    Placed { lines: Vec<Line>, total: Yen },
    Paid { lines: Vec<Line>, total: Yen },
    Shipped { lines: Vec<Line>, total: Yen, tracking: String },
    Cancelled { reason: CancelReason },
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
}

/// Adds `line`, or adds its quantity to the line of the same SKU, which
/// must have the same unit price.
pub fn add_line(lines: Vec<Line>, line: Line) -> Result<Vec<Line>, OrderError> {
    let mut out: Vec<Line> = Vec::new();
    let mut merged = false;
    for l in lines {
        if l.sku.0 != line.sku.0 {
            out.push(l);
        } else if l.unit_price.0 != line.unit_price.0 {
            return Err(OrderError::PriceMismatch);
        } else {
            let qty = l.qty.checked_add(line.qty).ok_or(OrderError::Overflow)?;
            out.push(Line { qty, ..l });
            merged = true;
        }
    }
    if !merged {
        out.push(line);
    }
    Ok(out)
}

/// Removes the line of `sku`; there is at most one, as `add_line` merges.
pub fn remove_sku(lines: Vec<Line>, sku: &Sku) -> Result<Vec<Line>, OrderError> {
    if !lines.iter().any(|l| l.sku.0 == sku.0) {
        return Err(OrderError::UnknownSku);
    }
    Ok(lines.into_iter().filter(|l| l.sku.0 != sku.0).collect())
}

pub fn total(lines: &Vec<Line>) -> Result<Yen, OrderError> {
    let mut sum = 0i64;
    for line in lines {
        let amount = line.unit_price.0.checked_mul(i64::from(line.qty)).ok_or(OrderError::Overflow)?;
        sum = sum.checked_add(amount).ok_or(OrderError::Overflow)?;
    }
    Ok(Yen(sum))
}

pub fn step(order: Order, cmd: Command) -> Result<Order, OrderError> {
    match (order, cmd) {
        (Order::Draft { .. }, Command::AddLine(line)) if line.qty == 0 => Err(OrderError::QtyZero),
        (Order::Draft { lines }, Command::AddLine(line)) => Ok(Order::Draft { lines: add_line(lines, line)? }),
        (Order::Draft { lines }, Command::RemoveSku(sku)) => Ok(Order::Draft { lines: remove_sku(lines, &sku)? }),
        (Order::Draft { lines }, Command::Place) if lines.is_empty() => Err(OrderError::Empty),
        (Order::Draft { lines }, Command::Place) => {
            let total = total(&lines)?;
            Ok(Order::Placed { lines, total })
        }
        (Order::Placed { total, .. }, Command::Pay(amount)) if amount.0 != total.0 => {
            Err(OrderError::AmountMismatch { expected: total, got: amount })
        }
        (Order::Placed { lines, total }, Command::Pay(_)) => Ok(Order::Paid { lines, total }),
        (Order::Paid { .. }, Command::Ship(tracking)) if tracking.is_empty() => Err(OrderError::EmptyTracking),
        (Order::Paid { lines, total }, Command::Ship(tracking)) => Ok(Order::Shipped { lines, total, tracking }),
        (Order::Draft { .. } | Order::Placed { .. }, Command::Cancel(reason)) => Ok(Order::Cancelled { reason }),
        _ => Err(OrderError::InvalidTransition),
    }
}
