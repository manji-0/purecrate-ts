pub enum E {
    A,
    B,
}

pub fn f(a: i32, e: E) -> i32 {
    let x = a + 1;
    let y = x * 2;
    let z = match e {
        E::A => y,
        E::B => undefined_name,
    };
    z + unknown_fn(y)
}
