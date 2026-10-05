fn new_line(sku: &str, price: i64, qty: u32) -> Result<Command, OrderError> {
    Ok(Command::AddLine(Line::new(Sku::new(String::from(sku))?, Yen::new(price)?, qty)?))
}

pub fn decode(code: u8) -> Result<Command, OrderError> {
    if code == 0 {
        new_line("a", 100i64, 2u32)
    } else if code == 1 {
        new_line("b", 250i64, 1u32)
    } else if code == 2 {
        new_line("a", 100i64, 0u32)
    } else if code == 3 {
        Ok(Command::RemoveSku(Sku::new(String::from("a"))?))
    } else if code == 4 {
        Ok(Command::Place)
    } else if code == 5 {
        Ok(Command::Pay(Yen::new(200i64)?))
    } else if code == 6 {
        Ok(Command::Pay(Yen::new(450i64)?))
    } else if code == 7 {
        Ok(Command::Ship(String::from("")))
    } else if code == 8 {
        Ok(Command::Ship(String::from("T1")))
    } else {
        Ok(Command::Cancel(CancelReason::ByCustomer))
    }
}

pub fn summary(order: &Order) -> Result<i64, OrderError> {
    match order.status() {
        Status::Draft { lines } => Ok(1000000 + total(lines)?.value()),
        Status::Placed { total, .. } => Ok(2000000 + total.value()),
        Status::Paid { total, .. } => Ok(3000000 + total.value()),
        Status::Shipped { total, .. } => Ok(4000000 + total.value()),
        Status::Cancelled { .. } => Ok(5000000),
    }
}

/// Runs that reach the limits: a second price for one SKU, and a quantity,
/// a line amount, and a total past their integers.
pub fn edge(code: u8) -> Result<i64, OrderError> {
    let order = Order::draft();
    let order = if code == 0 {
        let order = step(&order, new_line("a", 100i64, 2u32)?)?;
        step(&order, new_line("a", 120i64, 1u32)?)?
    } else if code == 1 {
        let order = step(&order, new_line("a", 100i64, 4294967295u32)?)?;
        step(&order, new_line("a", 100i64, 1u32)?)?
    } else if code == 2 {
        let order = step(&order, new_line("c", 9223372036854775807i64, 2u32)?)?;
        step(&order, Command::Place)?
    } else {
        let order = step(&order, new_line("c", 4611686018427387904i64, 1u32)?)?;
        let order = step(&order, new_line("d", 4611686018427387904i64, 1u32)?)?;
        step(&order, Command::Place)?
    };
    summary(&order)
}

pub fn run4(a: u8, b: u8, c: u8, d: u8) -> Result<i64, OrderError> {
    let order = Order::draft();
    let order = step(&order, decode(a)?)?;
    let order = step(&order, decode(b)?)?;
    let order = step(&order, decode(c)?)?;
    let order = step(&order, decode(d)?)?;
    summary(&order)
}

/// Each argument of `Line::new` refused in turn: an empty SKU, a negative
/// price, a zero quantity; else a line of 3 at 100.
pub fn open_with(code: u8) -> Result<i64, OrderError> {
    let sku = if code == 0 { Sku::new(String::from(""))? } else { Sku::new(String::from("a"))? };
    let unit_price = if code == 1 { Yen::new(-1i64)? } else { Yen::new(100i64)? };
    let qty = if code == 2 { 0u32 } else { 3u32 };
    let line = Line::new(sku, unit_price, qty)?;
    let order = step(&Order::draft(), Command::AddLine(line))?;
    summary(&order)
}

/// The lines of the order after four commands, one per SKU, as
/// `(code, unit price, quantity)`.
pub fn lines4(a: u8, b: u8, c: u8, d: u8) -> Result<Vec<(String, i64, u32)>, OrderError> {
    let order = Order::draft();
    let order = step(&order, decode(a)?)?;
    let order = step(&order, decode(b)?)?;
    let order = step(&order, decode(c)?)?;
    let order = step(&order, decode(d)?)?;
    let lines = match order.status() {
        Status::Draft { lines } => lines.clone(),
        Status::Placed { lines, .. } => lines.clone(),
        Status::Paid { lines, .. } => lines.clone(),
        Status::Shipped { lines, .. } => lines.clone(),
        Status::Cancelled { .. } => Vec::new(),
    };
    let mut out: Vec<(String, i64, u32)> = Vec::new();
    for l in &lines {
        out.push((String::from(l.sku().code()), l.unit_price().value(), l.qty()));
    }
    Ok(out)
}

fn attempt(order: &Order, code: u8) -> Result<Order, OrderError> {
    step(order, decode(code)?)
}

/// Every command in turn on one order, each refused one dropped: the order
/// a refusal leaves is the one the next command reads. Starts from two lines
/// of `a` and one of `b` (450), then tries the commands `a` to `d` in order.
pub fn survive(a: u8, b: u8, c: u8, d: u8) -> Result<(Order, Vec<OrderError>), OrderError> {
    let order = step(&Order::draft(), decode(0u8)?)?;
    let mut order = step(&order, decode(1u8)?)?;
    let mut refused: Vec<OrderError> = Vec::new();
    for code in vec![a, b, c, d] {
        match attempt(&order, code) {
            Ok(next) => order = next,
            Err(e) => refused.push(e),
        }
    }
    Ok((order, refused))
}

pub fn trace4(a: u8, b: u8, c: u8, d: u8) -> Result<Order, OrderError> {
    let order = Order::draft();
    let order = step(&order, decode(a)?)?;
    let order = step(&order, decode(b)?)?;
    let order = step(&order, decode(c)?)?;
    step(&order, decode(d)?)
}

/// A cancel from each status, for each reason: `at` is an empty draft (0),
/// a draft of one line (1), placed (2), paid (3), shipped (4), or cancelled
/// (5); `reason` is `ByCustomer` (0) or `PaymentFailed` (1).
pub fn cancel_at(at: u8, reason: u8) -> Result<Order, OrderError> {
    let order = Order::draft();
    let order = if at == 0 { order } else { step(&order, decode(0u8)?)? };
    let order = if at >= 2 && at != 5 { step(&order, Command::Place)? } else { order };
    let order = if at >= 3 && at != 5 { step(&order, Command::Pay(Yen::new(200i64)?))? } else { order };
    let order = if at == 4 { step(&order, decode(8u8)?)? } else { order };
    let order = if at == 5 { step(&order, decode(9u8)?)? } else { order };
    let reason = if reason == 0 { CancelReason::ByCustomer } else { CancelReason::PaymentFailed };
    step(&order, Command::Cancel(reason))
}

/// A line of `qty` at a unit price of zero, placed, then paid `paid`.
pub fn free_line(qty: u32, paid: i64) -> Result<Order, OrderError> {
    let line = Line::new(Sku::new(String::from("gift"))?, Yen::new(0i64)?, qty)?;
    let order = step(&Order::draft(), Command::AddLine(line))?;
    let order = step(&order, Command::Place)?;
    step(&order, Command::Pay(Yen::new(paid)?))
}
