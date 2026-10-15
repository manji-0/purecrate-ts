// Traits (design/01 §7.25): a trait as the type of a record of functions,
// an `impl` as members of the type's companion, a default body taking the
// dictionary first, and a bound `T: Tr` as one more parameter carrying the
// record for `T`.

/// Something with a price.
pub trait Priced {
    /// How many units a doubled price counts.
    const RATE: i64 = 2;

    fn price(&self) -> i64;

    /// The price times `RATE`, through the dictionary.
    fn doubled(&self) -> i64 {
        self.price() * Self::RATE
    }

    fn scaled(&self, k: i64) -> i64 {
        self.doubled() * k
    }

    fn zero() -> Self;
}

/// A priced thing with a name.
pub trait Named: Priced {
    fn name(&self) -> String;

    fn label(&self) -> String {
        let n = self.name();
        let p = self.price();
        format!("{n}@{p}")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub sku: String,
    pub price: i64,
}

impl Priced for Item {
    fn price(&self) -> i64 {
        self.price
    }

    fn zero() -> Self {
        Item { sku: String::new(), price: 0 }
    }
}

impl Named for Item {
    fn name(&self) -> String {
        self.sku.clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Coin {
    Small,
    Big(i64),
}

impl Priced for Coin {
    const RATE: i64 = 3;

    fn price(&self) -> i64 {
        match self {
            Coin::Small => 1,
            Coin::Big(n) => *n,
        }
    }

    fn doubled(&self) -> i64 {
        self.price() + 100
    }

    fn zero() -> Self {
        Coin::Small
    }
}

impl Named for Coin {
    fn name(&self) -> String {
        match self {
            Coin::Small => "petit".to_string(),
            Coin::Big(_) => "gros€".to_string(),
        }
    }

    fn label(&self) -> String {
        let n = self.name();
        format!("<{n}>")
    }
}

/// A generic implementor: its own parameter carries only `Clone`.
#[derive(Clone, Debug, PartialEq)]
pub struct Bag<T> {
    pub items: Vec<T>,
    pub fee: i64,
}

impl<T: Clone> Priced for Bag<T> {
    fn price(&self) -> i64 {
        self.fee * self.items.len() as i64
    }

    fn zero() -> Self {
        Bag { items: Vec::new(), fee: 0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shelf<T> {
    pub items: Vec<T>,
}

impl<T: Priced> Shelf<T> {
    pub fn total(&self) -> i64 {
        sum_doubled(&self.items)
    }

    pub fn cheapest(&self) -> Option<i64> {
        self.items.iter().map(|x| x.price()).min()
    }
}

pub fn sum_doubled<T: Priced>(xs: &[T]) -> i64 {
    let mut t = 0i64;
    for x in xs {
        t += x.doubled();
    }
    t
}

pub fn zero_price<T>(x: &T) -> i64
where
    T: Priced,
{
    let z = T::zero();
    z.price() + T::RATE + Priced::price(x)
}

pub fn describe<T: Named>(xs: &[T]) -> String {
    let mut out = String::new();
    for x in xs {
        out.push_str(&x.label());
    }
    let total = sum_doubled(xs);
    format!("{out}={total}")
}

pub fn pair<A: Priced, B: Priced + Named>(a: &A, b: &B) -> i64 {
    a.scaled(2) + b.scaled(3) + b.name().len() as i64
}

pub fn any_price(x: impl Priced) -> i64 {
    x.doubled()
}

/// Every call on a known type: the method, its path, the trait's path,
/// and the consts.
pub fn known(i: Item, c: Coin) -> i64 {
    i.price() + Item::price(&i) + Priced::price(&c) + i.doubled() + c.doubled() + Item::RATE + Coin::RATE
}

pub fn items(prices: Vec<i64>, name: String) -> String {
    let xs: Vec<Item> = prices.iter().map(|p| Item { sku: name.clone(), price: *p }).collect();
    let shelf = Shelf { items: xs.clone() };
    let total = shelf.total();
    let cheapest = shelf.cheapest().unwrap_or(-1);
    let d = describe(&xs);
    format!("{total} {cheapest} {d}")
}

pub fn coins(ns: Vec<i64>) -> String {
    let mut xs: Vec<Coin> = Vec::new();
    for n in ns {
        if n == 0 {
            xs.push(Coin::Small);
        } else {
            xs.push(Coin::Big(n));
        }
    }
    let d = describe(&xs);
    let z = zero_price(&Coin::Big(7));
    format!("{} {z} {d}", sum_doubled(&xs))
}

pub fn bags(fee: i64, n: usize) -> i64 {
    let items: Vec<usize> = (0..n).collect();
    let b = Bag { items, fee };
    let shelf = Shelf { items: vec![b.clone(), Bag::zero()] };
    b.doubled() + b.scaled(2) + shelf.total() + zero_price(&b) + any_price(b.clone())
}

pub fn mixed(i: Item, n: i64) -> i64 {
    pair(&Coin::Big(n), &i) + pair(&i, &Coin::Small) + any_price(i) + known(Item::zero(), Coin::zero())
}
