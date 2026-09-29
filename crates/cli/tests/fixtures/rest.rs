pub enum Shape {
    Dot,
    Circle(i32),
    Rect(i32, i32),
    Named { label: String, sides: i32 },
}

pub enum Light {
    Red,
    Amber,
    Green,
    Off,
}

pub fn shape_of(code: i32) -> Shape {
    if code == 0 {
        Shape::Dot
    } else if code == 1 {
        Shape::Circle(3)
    } else if code == 2 {
        Shape::Rect(2, 5)
    } else {
        Shape::Named { label: String::from("tri"), sides: 3 }
    }
}

pub fn light_of(code: i32) -> Light {
    if code == 0 {
        Light::Red
    } else if code == 1 {
        Light::Amber
    } else if code == 2 {
        Light::Green
    } else {
        Light::Off
    }
}

/// `_` after one arm, as a statement and as a value.
pub fn area(code: i32) -> i32 {
    let s = shape_of(code);
    let base = match s {
        Shape::Rect(w, h) => w * h,
        _ => 0,
    };
    base + 1
}

/// `|` over tuple, struct and unit variants, with `_` taking the rest.
pub fn corners(code: i32) -> i32 {
    match shape_of(code) {
        Shape::Dot | Shape::Circle(_) => 0,
        Shape::Named { sides, .. } => sides,
        _ => 4,
    }
}

/// `|` alone covering every variant in two arms.
pub fn stops(code: i32) -> bool {
    match light_of(code) {
        Light::Red | Light::Amber => true,
        Light::Green | Light::Off => false,
    }
}

/// `_` with an early `return` and a `?` in the named arm.
pub fn next(code: i32) -> Result<i32, i32> {
    let l = light_of(code);
    let n: Result<i32, i32> = match l {
        Light::Off => return Err(code),
        _ => Ok(code + 1),
    };
    let v = n?;
    Ok(v * 10)
}

/// `_` on `Option` and `Result`, before and after the named case.
pub fn options(x: i32) -> i32 {
    let o: Option<i32> = if x > 0 { Some(x) } else { None };
    let r: Result<i32, i32> = if x % 2 == 0 { Ok(x) } else { Err(x) };
    let a = match o {
        Some(v) => v,
        _ => -1,
    };
    let b: i32 = match o {
        None => 100,
        _ => 200,
    };
    let c = match r {
        Err(e) => e * 3,
        _ => 7,
    };
    let d = match r {
        Ok(v) => v,
        _ => 11,
    };
    a + b + c + d
}

/// Nested matches, each with its own `_`.
pub fn nested(a: i32, b: i32) -> i32 {
    match light_of(a) {
        Light::Green => match shape_of(b) {
            Shape::Circle(r) => r,
            _ => 10,
        },
        Light::Red | Light::Off => 20,
        _ => match light_of(b) {
            Light::Amber => 30,
            _ => 40,
        },
    }
}

/// `matches!` on enums, `Option`, and a pattern covering every variant.
pub fn matched(a: i32, b: i32) -> i32 {
    let o: Option<i32> = if a > 1 { Some(a) } else { None };
    let mut n = 0i32;
    if matches!(light_of(a), Light::Red | Light::Amber) {
        n += 1;
    }
    if matches!(shape_of(b), Shape::Named { .. }) {
        n += 10;
    }
    if matches!(o, Some(_)) {
        n += 100;
    }
    if matches!(light_of(b), Light::Red | Light::Amber | Light::Green | Light::Off) {
        n += 1000;
    }
    n
}
