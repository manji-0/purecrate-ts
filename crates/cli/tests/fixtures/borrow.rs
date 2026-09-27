pub struct Meters(i32);

impl Meters {
    pub fn new(n: &i32) -> Self {
        Self(*n)
    }

    pub fn value(&self) -> i32 {
        self.0
    }

    pub fn plus(&self, other: &Meters) -> Self {
        Self(self.0 + other.0)
    }
}

pub struct Leg {
    pub from: i32,
    pub to: i32,
}

impl Leg {
    pub fn length(&self) -> Meters {
        Meters::new(&(self.to - self.from))
    }

    pub fn reversed(&self) -> Self {
        Self {
            from: self.to,
            to: self.from,
        }
    }
}

pub enum Mode {
    Walk,
    Ride(i32),
}

impl Mode {
    pub fn speed(&self) -> i32 {
        match self {
            Self::Walk => 5,
            Self::Ride(n) => *n,
        }
    }
}

pub fn trip(a: i32, b: i32, c: i32) -> i32 {
    let first = Leg { from: a, to: b };
    let second = Leg { from: b, to: c };
    let total = Meters::plus(&Leg::length(&first), &Leg::length(&second));
    Meters::value(&total)
}

pub fn back(a: i32, b: i32) -> i32 {
    let leg = Leg::reversed(&Leg { from: a, to: b });
    Meters::value(&Leg::length(&leg))
}

pub fn hours(distance: &i32, ride: bool, speed: i32) -> Option<i32> {
    let mode = if ride { Mode::Ride(speed) } else { Mode::Walk };
    let s = Mode::speed(&mode);
    if s == 0 {
        None
    } else {
        Some(*distance / s)
    }
}

pub fn label<'a>(name: &'a str, fallback: &'a str, use_name: bool) -> &'a str {
    let chosen: &str = if use_name { name } else { fallback };
    chosen
}
