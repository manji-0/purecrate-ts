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

pub fn summary(order: &Order) -> i64 {
    match order {
        Order::Draft { lines } => 1000000 + total(lines).0,
        Order::Placed { total, .. } => 2000000 + total.0,
        Order::Paid { total, .. } => 3000000 + total.0,
        Order::Shipped { total, .. } => 4000000 + total.0,
        Order::Cancelled { .. } => 5000000,
    }
}

pub fn run4(a: u8, b: u8, c: u8, d: u8) -> Result<i64, OrderError> {
    let order = Order::Draft { lines: Lines::Nil };
    let order = step(order, decode(a))?;
    let order = step(order, decode(b))?;
    let order = step(order, decode(c))?;
    let order = step(order, decode(d))?;
    Ok(summary(&order))
}
