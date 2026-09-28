pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

pub fn pack(p: Pos) -> i32 {
    p.x + p.y * 100 + p.z * 10000
}

pub fn shift_x(x: i32, y: i32, z: i32, d: i32) -> i32 {
    let p = Pos { x, y, z };
    pack(Pos { x: p.x + d, ..p })
}

pub fn copy(x: i32, y: i32, z: i32) -> i32 {
    let p = Pos { x, y, z };
    pack(Pos { ..p })
}

pub fn gate(p: Pos, flag: Result<i32, i32>) -> Result<Pos, i32> {
    let _n = flag?;
    Ok(p)
}

/// `dx?` is evaluated before `..gate(...)`. A non-zero `dx_err` is `Err` and
/// leaves before `gate`, whose non-zero `flag_err` would be a different `Err`.
pub fn over(x: i32, y: i32, z: i32, dx_err: i32, flag_err: i32) -> Result<i32, i32> {
    let dx: Result<i32, i32> = if dx_err == 0 { Ok(5i32) } else { Err(dx_err) };
    let flag: Result<i32, i32> = if flag_err == 0 { Ok(0i32) } else { Err(flag_err) };
    let p = Pos { x, y, z };
    let q = Pos {
        x: p.x + dx?,
        ..gate(p, flag)?
    };
    Ok(pack(q))
}
