#[derive(Clone, Copy)]
pub enum Coin {
    Ten,
    Fifty,
    Hundred,
}

#[derive(Clone, Copy)]
pub enum Drink {
    Water,
    Tea,
}

pub enum Event {
    Insert(Coin),
    Select(Drink),
    Refund,
}

pub struct Machine {
    pub credit: u32,
    pub water: u32,
    pub tea: u32,
    pub sales: u32,
}

pub enum Fault {
    SoldOut,
    Short(u32),
    Full,
}

fn coin_value(coin: Coin) -> u32 {
    match coin {
        Coin::Ten => 10,
        Coin::Fifty => 50,
        Coin::Hundred => 100,
    }
}

fn price(drink: Drink) -> u32 {
    match drink {
        Drink::Water => 110,
        Drink::Tea => 150,
    }
}

pub fn step(m: Machine, event: Event) -> Result<Machine, Fault> {
    let mut credit = m.credit;
    let mut water = m.water;
    let mut tea = m.tea;
    let mut sales = m.sales;
    match event {
        Event::Insert(coin) => {
            credit += coin_value(coin);
            if credit > 200 {
                return Err(Fault::Full);
            }
        }
        Event::Select(drink) => {
            let cost = price(drink);
            if credit < cost {
                return Err(Fault::Short(cost - credit));
            }
            match drink {
                Drink::Water => {
                    if water == 0 {
                        return Err(Fault::SoldOut);
                    }
                    water -= 1;
                }
                Drink::Tea => {
                    if tea == 0 {
                        return Err(Fault::SoldOut);
                    }
                    tea -= 1;
                }
            }
            credit -= cost;
            sales += cost;
        }
        Event::Refund => {
            credit = 0;
        }
    }
    Ok(Machine {
        credit,
        water,
        tea,
        sales,
    })
}

fn decode(code: u8) -> Event {
    if code == 0 {
        Event::Insert(Coin::Ten)
    } else if code == 1 {
        Event::Insert(Coin::Fifty)
    } else if code == 2 {
        Event::Insert(Coin::Hundred)
    } else if code == 3 {
        Event::Select(Drink::Water)
    } else if code == 4 {
        Event::Select(Drink::Tea)
    } else {
        Event::Refund
    }
}

/// Four events from a machine holding one water and one tea; the final state
/// as `credit * 1_000_000 + sales * 1_000 + water * 10 + tea`.
pub fn run4(a: u8, b: u8, c: u8, d: u8) -> Result<u32, Fault> {
    let m = Machine {
        credit: 0,
        water: 1,
        tea: 1,
        sales: 0,
    };
    let m = step(m, decode(a))?;
    let m = step(m, decode(b))?;
    let m = step(m, decode(c))?;
    let m = step(m, decode(d))?;
    Ok(m.credit * 1_000_000 + m.sales * 1_000 + m.water * 10 + m.tea)
}
