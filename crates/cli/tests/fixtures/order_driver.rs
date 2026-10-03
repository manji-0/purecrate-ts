pub fn decode(code: u8) -> Command {
    if code == 0 {
        Command::AddLine(Line { sku: Sku(String::from("a")), unit_price: Yen(100), qty: 2 })
    } else if code == 1 {
        Command::AddLine(Line { sku: Sku(String::from("b")), unit_price: Yen(250), qty: 1 })
    } else if code == 2 {
        Command::AddLine(Line { sku: Sku(String::from("a")), unit_price: Yen(100), qty: 0 })
    } else if code == 3 {
        Command::RemoveSku(Sku(String::from("a")))
    } else if code == 4 {
        Command::Place
    } else if code == 5 {
        Command::Pay(Yen(200))
    } else if code == 6 {
        Command::Pay(Yen(450))
    } else if code == 7 {
        Command::Ship(String::from(""))
    } else if code == 8 {
        Command::Ship(String::from("T1"))
    } else {
        Command::Cancel(CancelReason::ByCustomer)
    }
}

pub fn summary(order: &Order) -> Result<i64, OrderError> {
    match order {
        Order::Draft { lines } => Ok(1000000 + total(lines)?.value()),
        Order::Placed { total, .. } => Ok(2000000 + total.value()),
        Order::Paid { total, .. } => Ok(3000000 + total.value()),
        Order::Shipped { total, .. } => Ok(4000000 + total.value()),
        Order::Cancelled { .. } => Ok(5000000),
    }
}

fn new_line(sku: &str, price: i64, qty: u32) -> Command {
    Command::AddLine(Line { sku: Sku(String::from(sku)), unit_price: Yen(price), qty })
}

/// Runs that reach the limits: a second price for one SKU, and a quantity,
/// a line amount, and a total past their integers.
pub fn edge(code: u8) -> Result<i64, OrderError> {
    let order = Order::Draft { lines: vec![] };
    let order = if code == 0 {
        let order = step(order, new_line("a", 100i64, 2u32))?;
        step(order, new_line("a", 120i64, 1u32))?
    } else if code == 1 {
        let order = step(order, new_line("a", 100i64, 4294967295u32))?;
        step(order, new_line("a", 100i64, 1u32))?
    } else if code == 2 {
        let order = step(order, new_line("c", 9223372036854775807i64, 2u32))?;
        step(order, Command::Place)?
    } else {
        let order = step(order, new_line("c", 4611686018427387904i64, 1u32))?;
        let order = step(order, new_line("d", 4611686018427387904i64, 1u32))?;
        step(order, Command::Place)?
    };
    summary(&order)
}

pub fn run4(a: u8, b: u8, c: u8, d: u8) -> Result<i64, OrderError> {
    let order = Order::Draft { lines: vec![] };
    let order = step(order, decode(a))?;
    let order = step(order, decode(b))?;
    let order = step(order, decode(c))?;
    let order = step(order, decode(d))?;
    summary(&order)
}

pub fn open_with(code: u8) -> Result<i64, OrderError> {
    let sku = if code == 0 { Sku::new(String::from(""))? } else { Sku::new(String::from("a"))? };
    let unit_price = if code == 1 { Yen::new(-1i64)? } else { Yen::new(100i64)? };
    let line = Line { sku, unit_price, qty: 3u32 };
    let order = step(Order::Draft { lines: vec![] }, Command::AddLine(line))?;
    summary(&order)
}

pub fn trace4(a: u8, b: u8, c: u8, d: u8) -> Result<Order, OrderError> {
    let order = Order::Draft { lines: vec![] };
    let order = step(order, decode(a))?;
    let order = step(order, decode(b))?;
    let order = step(order, decode(c))?;
    step(order, decode(d))
}
