pub fn add_i32(a: i32, b: i32) -> i32 {
    a + b
}

pub fn sub_i32(a: i32, b: i32) -> i32 {
    a - b
}

pub fn mul_i32(a: i32, b: i32) -> i32 {
    a * b
}

pub fn div_i32(a: i32, b: i32) -> i32 {
    a / b
}

pub fn rem_i32(a: i32, b: i32) -> i32 {
    a % b
}

pub fn neg_i32(a: i32) -> i32 {
    -a
}

pub fn half_i32(a: i32) -> i32 {
    a / 2
}

pub fn min_i32() -> i32 {
    -2147483648
}

pub fn add_u8(a: u8, b: u8) -> u8 {
    a + b
}

pub fn sub_u8(a: u8, b: u8) -> u8 {
    a - b
}

pub fn div_i8(a: i8, b: i8) -> i8 {
    a / b
}

pub fn rem_i8(a: i8, b: i8) -> i8 {
    a % b
}

pub fn mul_u32(a: u32, b: u32) -> u32 {
    a * b
}

pub fn add_i64(a: i64, b: i64) -> i64 {
    a + b
}

pub fn mul_i64(a: i64, b: i64) -> i64 {
    a * b
}

pub fn div_i64(a: i64, b: i64) -> i64 {
    a / b
}

pub fn rem_i64(a: i64, b: i64) -> i64 {
    a % b
}

pub fn neg_i64(a: i64) -> i64 {
    -a
}

pub fn sub_u64(a: u64, b: u64) -> u64 {
    a - b
}

pub fn affine_i64(a: i64) -> i64 {
    a * 3 - 7
}

pub fn small_i64(a: i64) -> bool {
    a < 10
}

pub fn scaled(a: i32) -> i64 {
    let k: i64 = 5;
    k * 2 + if a > 0 { 1 } else { -1 }
}

pub fn grouped_i32(a: i32, b: i32, c: i32) -> i32 {
    (a + b) * c
}

pub fn avg_f32(a: f32, b: f32) -> f32 {
    (a + b) / 2.0
}

pub fn third_f32(a: f32) -> f32 {
    a / 3.0
}

pub fn poly_f64(x: f64) -> f64 {
    (x + 1.0) * (x - 1.0)
}

pub fn div_f64(a: f64, b: f64) -> f64 {
    a / b
}
