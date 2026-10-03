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

pub enum Lines {
    Nil,
    Cons(Line, Box<Lines>),
}

pub enum CancelReason {
    ByCustomer,
    PaymentFailed,
}

pub enum Order {
    Draft { lines: Lines },
    Placed { lines: Lines, total: Yen },
    Paid { lines: Lines, total: Yen },
    Shipped { lines: Lines, total: Yen, tracking: String },
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
pub fn add_line(lines: Lines, line: Line) -> Result<Lines, OrderError> {
    match lines {
        Lines::Nil => Ok(Lines::Cons(line, Box::new(Lines::Nil))),
        Lines::Cons(head, rest) => {
            if head.sku.0 != line.sku.0 {
                return Ok(Lines::Cons(head, Box::new(add_line(*rest, line)?)));
            }
            if head.unit_price.0 != line.unit_price.0 {
                return Err(OrderError::PriceMismatch);
            }
            let qty = head.qty.checked_add(line.qty).ok_or(OrderError::Overflow)?;
            Ok(Lines::Cons(Line { qty, ..head }, rest))
        }
    }
}

pub fn remove_sku(lines: Lines, sku: &Sku) -> Result<Lines, OrderError> {
    match lines {
        Lines::Nil => Err(OrderError::UnknownSku),
        Lines::Cons(head, rest) => {
            if head.sku.0 == sku.0 {
                Ok(*rest)
            } else {
                let rest = remove_sku(*rest, sku)?;
                Ok(Lines::Cons(head, Box::new(rest)))
            }
        }
    }
}

pub fn total(lines: &Lines) -> Result<Yen, OrderError> {
    match lines {
        Lines::Nil => Ok(Yen(0)),
        Lines::Cons(line, rest) => {
            let amount = line.unit_price.0.checked_mul(i64::from(line.qty)).ok_or(OrderError::Overflow)?;
            let sum = amount.checked_add(total(rest)?.0).ok_or(OrderError::Overflow)?;
            Ok(Yen(sum))
        }
    }
}

pub fn step(order: Order, cmd: Command) -> Result<Order, OrderError> {
    match (order, cmd) {
        (Order::Draft { .. }, Command::AddLine(line)) if line.qty == 0 => Err(OrderError::QtyZero),
        (Order::Draft { lines }, Command::AddLine(line)) => Ok(Order::Draft { lines: add_line(lines, line)? }),
        (Order::Draft { lines }, Command::RemoveSku(sku)) => Ok(Order::Draft { lines: remove_sku(lines, &sku)? }),
        (Order::Draft { lines }, Command::Place) if matches!(lines, Lines::Nil) => Err(OrderError::Empty),
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
