// `f32` / `f64` methods, constants, and conversions: `round` half away
// from zero, `floor`, `ceil`, `trunc`, `abs`, the NaN and infinity tests;
// a float to an integer toward zero and saturating, NaN to 0; an integer to
// a float rounded once to the nearest, ties to even; `f64` to `f32`.

pub fn rounded(x: f64) -> (f64, f64, f64, f64, f64) {
    (x.round(), x.floor(), x.ceil(), x.trunc(), x.abs())
}

pub fn rounded32(x: f32) -> (f32, f32, f32, f32, f32) {
    (x.round(), x.floor(), x.ceil(), x.trunc(), x.abs())
}

pub fn tests(x: f64) -> (bool, bool, bool) {
    (x.is_nan(), x.is_finite(), x.is_infinite())
}

pub fn tests32(x: f32) -> (bool, bool, bool) {
    (x.is_nan(), x.is_finite(), x.is_infinite())
}

pub fn consts() -> (f64, f64, f64, f32) {
    (f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f32::NEG_INFINITY)
}

pub fn narrowed(x: f64) -> f32 {
    x as f32
}

pub fn widened(x: f32, n: i32, b: u16) -> (f64, f64, f32) {
    (f64::from(x), f64::from(n), f32::from(b))
}

pub fn f64_as_i8(x: f64) -> i8 {
    x as i8
}

pub fn f32_as_i8(x: f32) -> i8 {
    x as i8
}

pub fn f64_as_i16(x: f64) -> i16 {
    x as i16
}

pub fn f32_as_i16(x: f32) -> i16 {
    x as i16
}

pub fn f64_as_i32(x: f64) -> i32 {
    x as i32
}

pub fn f32_as_i32(x: f32) -> i32 {
    x as i32
}

pub fn i32_as_f32(n: i32) -> f32 {
    n as f32
}

pub fn f64_as_i64(x: f64) -> i64 {
    x as i64
}

pub fn f32_as_i64(x: f32) -> i64 {
    x as i64
}

pub fn i64_as_f64(n: i64) -> f64 {
    n as f64
}

pub fn i64_as_f32(n: i64) -> f32 {
    n as f32
}

pub fn f64_as_i128(x: f64) -> i128 {
    x as i128
}

pub fn f32_as_i128(x: f32) -> i128 {
    x as i128
}

pub fn i128_as_f64(n: i128) -> f64 {
    n as f64
}

pub fn i128_as_f32(n: i128) -> f32 {
    n as f32
}

pub fn f64_as_u8(x: f64) -> u8 {
    x as u8
}

pub fn f32_as_u8(x: f32) -> u8 {
    x as u8
}

pub fn f64_as_u16(x: f64) -> u16 {
    x as u16
}

pub fn f32_as_u16(x: f32) -> u16 {
    x as u16
}

pub fn f64_as_u32(x: f64) -> u32 {
    x as u32
}

pub fn f32_as_u32(x: f32) -> u32 {
    x as u32
}

pub fn u32_as_f32(n: u32) -> f32 {
    n as f32
}

pub fn f64_as_u64(x: f64) -> u64 {
    x as u64
}

pub fn f32_as_u64(x: f32) -> u64 {
    x as u64
}

pub fn u64_as_f64(n: u64) -> f64 {
    n as f64
}

pub fn u64_as_f32(n: u64) -> f32 {
    n as f32
}

pub fn f64_as_u128(x: f64) -> u128 {
    x as u128
}

pub fn f32_as_u128(x: f32) -> u128 {
    x as u128
}

pub fn u128_as_f64(n: u128) -> f64 {
    n as f64
}

pub fn u128_as_f32(n: u128) -> f32 {
    n as f32
}

pub fn f64_as_usize(x: f64) -> usize {
    x as usize
}

pub fn f32_as_usize(x: f32) -> usize {
    x as usize
}

pub fn usize_as_f64(n: usize) -> f64 {
    n as f64
}

pub fn usize_as_f32(n: usize) -> f32 {
    n as f32
}

/// Encoded Polyline's step: a coordinate scaled and rounded to an integer.
pub fn scaled(x: f64, places: i32) -> i64 {
    let mut f: f64 = 1.0;
    for _ in 0..places {
        f = f * 10.0;
    }
    (x * f).round() as i64
}

/// `!` before a test printed as a comparison would take only its left side.
pub fn not_infinite(x: f64) -> (bool, bool) {
    (!x.is_infinite(), !(x * 2.0).is_nan())
}

/// An unused `let` of an `if` keeps its test's effects, the test negated.
pub fn kept_test(x: f64, k: i32) -> i32 {
    let _v: i32 = if x.is_infinite() { 0 } else { 1 / k };
    1
}

/// Shapes lint once refused: a branded float into `round` and `castFloat`,
/// a negated `f32` literal, a literal more precise than a double, and an
/// unused conversion.
pub fn lint_shapes(z: f32, x: f64, b: bool, m: u32) -> (f32, f64, i64, f32, bool) {
    let _w: f64 = f64::from(m);
    (
        (z * z).round(),
        (if b { x * x } else { 1e10 }).round(),
        (if b { x.abs() } else { x }) as i64,
        z * -0.0,
        x >= 9223372036854775807.0,
    )
}
