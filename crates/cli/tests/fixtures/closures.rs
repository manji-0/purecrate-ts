pub enum Sign {
    Neg,
    Zero,
    Pos,
}

impl Sign {
    pub fn weight(&self) -> i32 {
        match self {
            Sign::Neg => -1,
            Sign::Zero => 0,
            Sign::Pos => 1,
        }
    }
}

pub enum Fault {
    Negative,
    TooBig,
}

pub fn inc(v: i32) -> i32 {
    v + 100
}

pub fn checked(v: i32) -> Result<i32, Fault> {
    if v < 0 {
        Err(Fault::Negative)
    } else if v > 1000 {
        Err(Fault::TooBig)
    } else {
        Ok(v)
    }
}

pub fn half(v: i32) -> Option<i32> {
    if v % 2 == 0 {
        Some(v / 2)
    } else {
        None
    }
}

pub fn add_twice(x: i32, y: i32) -> i32 {
    let add = |a: i32, b: i32| a + b;
    add(add(x, y), y)
}

pub fn scaled(x: i32) -> i32 {
    let k: i32 = 3;
    let scale = |v: i32| v * k;
    scale(x) + scale(1)
}

pub fn blocky(x: i32) -> i32 {
    let step = |v: i32| {
        let doubled = v * 2;
        if doubled > 10 {
            doubled - 10
        } else {
            doubled
        }
    };
    step(step(x))
}

pub fn nested(x: i32) -> i32 {
    let outer = |a: i32| {
        let inner = |b: i32| a + b;
        inner(a) + inner(1)
    };
    outer(x)
}

pub fn shadowing(x: i32) -> i32 {
    let f = |x: i32| x + 1;
    f(x * 2) + x
}

pub fn signs(x: i32, y: i32) -> i32 {
    let classify = |v: i32| {
        if v > 0 {
            Sign::Pos
        } else if v < 0 {
            Sign::Neg
        } else {
            Sign::Zero
        }
    };
    classify(x).weight() * 10 + classify(y).weight()
}

pub fn local_wins(x: i32) -> i32 {
    let before = inc(x);
    let inc = |v: i32| v + 1;
    before + inc(x)
}

pub fn tried(x: i32) -> i32 {
    let parse = |v: i32| -> Result<i32, Fault> {
        let d = checked(v)?;
        Ok(d * 2)
    };
    match parse(x) {
        Ok(n) => n,
        Err(f) => code(f),
    }
}

pub fn code(f: Fault) -> i32 {
    match f {
        Fault::Negative => -1,
        Fault::TooBig => -2,
    }
}

pub fn quartered(x: i32) -> i32 {
    let quarter = |v: i32| -> Option<i32> { half(half(v)?) };
    match quarter(x) {
        Some(n) => n,
        None => -1,
    }
}
