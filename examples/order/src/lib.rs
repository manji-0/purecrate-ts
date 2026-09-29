pub struct Yen(i64);

impl Yen {
    pub fn new(amount: i64) -> Result<Yen, OrderError> {
        if amount < 0 {
            Err(OrderError::NegativeAmount)
        } else {
            Ok(Yen(amount))
        }
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
}

pub fn add_line(lines: Lines, line: Line) -> Lines {
    match lines {
        Lines::Nil => Lines::Cons(line, Box::new(Lines::Nil)),
        Lines::Cons(head, rest) => {
            if head.sku.0 == line.sku.0 {
                let merged = Line { qty: head.qty + line.qty, ..head };
                Lines::Cons(merged, rest)
            } else {
                Lines::Cons(head, Box::new(add_line(*rest, line)))
            }
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

pub fn total(lines: &Lines) -> Yen {
    match lines {
        Lines::Nil => Yen(0),
        Lines::Cons(line, rest) => Yen(line.unit_price.0 * i64::from(line.qty) + total(rest).0),
    }
}

pub fn step(order: Order, cmd: Command) -> Result<Order, OrderError> {
    match order {
        Order::Draft { lines } => draft(lines, cmd),
        Order::Placed { lines, total } => placed(lines, total, cmd),
        Order::Paid { lines, total } => paid(lines, total, cmd),
        Order::Shipped { .. } | Order::Cancelled { .. } => Err(OrderError::InvalidTransition),
    }
}

fn draft(lines: Lines, cmd: Command) -> Result<Order, OrderError> {
    match cmd {
        Command::AddLine(line) => {
            if line.qty == 0 {
                return Err(OrderError::QtyZero);
            }
            Ok(Order::Draft { lines: add_line(lines, line) })
        }
        Command::RemoveSku(sku) => Ok(Order::Draft { lines: remove_sku(lines, &sku)? }),
        Command::Place => match lines {
            Lines::Nil => Err(OrderError::Empty),
            Lines::Cons(head, rest) => {
                let lines = Lines::Cons(head, rest);
                let total = total(&lines);
                Ok(Order::Placed { lines, total })
            }
        },
        Command::Cancel(reason) => Ok(Order::Cancelled { reason }),
        _ => Err(OrderError::InvalidTransition),
    }
}

fn placed(lines: Lines, total: Yen, cmd: Command) -> Result<Order, OrderError> {
    match cmd {
        Command::Pay(amount) => {
            if amount.0 != total.0 {
                return Err(OrderError::AmountMismatch { expected: total, got: amount });
            }
            Ok(Order::Paid { lines, total })
        }
        Command::Cancel(reason) => Ok(Order::Cancelled { reason }),
        _ => Err(OrderError::InvalidTransition),
    }
}

fn paid(lines: Lines, total: Yen, cmd: Command) -> Result<Order, OrderError> {
    match cmd {
        Command::Ship(tracking) => {
            if tracking.is_empty() {
                return Err(OrderError::EmptyTracking);
            }
            Ok(Order::Shipped { lines, total, tracking })
        }
        _ => Err(OrderError::InvalidTransition),
    }
}
