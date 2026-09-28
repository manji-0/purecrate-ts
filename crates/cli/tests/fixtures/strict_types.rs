pub enum E {
    A,
    B(i32),
}

pub fn try_on_if(c: bool) -> Result<i32, i32> {
    let r: Result<i32, i32> = if c { Ok(1i32) } else { Err(2i32) };
    let v: i32 = r?;
    Ok(v + 1)
}

pub fn match_on_ok(a: i32) -> i32 {
    let r: Result<i32, i32> = Ok(a);
    match r {
        Ok(v) => v,
        Err(e) => e,
    }
}

pub fn pair(a: i32) -> (i32, i32) {
    let t = (a, a);
    t
}

fn takes(_t: (i32, i32)) -> i32 {
    7
}

pub fn first(a: i32) -> i32 {
    let t = (a, a);
    takes(t) + takes(pair(a))
}

pub fn variant_if(c: bool, n: i32) -> i32 {
    let e = if c { E::A } else { E::B(n) };
    match e {
        E::A => 0,
        E::B(m) => m,
    }
}

pub fn option_if(c: bool, n: i32) -> i32 {
    match if c { Some(n) } else { None } {
        Some(m) => m,
        None => 0,
    }
}
