// `impl From<S> for X` becomes `X.fromS`, called by `X::from(s)`,
// `From::from(s)`, `s.into()`, and by `?` where the error type is `S` and
// the function returns `Result<_, X>`: the conversion runs only on `Err`,
// after its operand, and a `?` whose types match is unchanged.

use std::num::ParseIntError;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowError {
    TooLong(usize),
    Blank,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Number,
    BadId,
    Low(LowError),
    Code(u8),
}

impl From<ParseIntError> for AppError {
    fn from(_e: ParseIntError) -> Self {
        AppError::Number
    }
}

impl From<uuid::Error> for AppError {
    fn from(_e: uuid::Error) -> AppError {
        AppError::BadId
    }
}

impl From<LowError> for AppError {
    fn from(e: LowError) -> Self {
        AppError::Low(e)
    }
}

/// Panics on overflow in the conversion, as Rust's does.
impl From<u8> for AppError {
    fn from(n: u8) -> Self {
        AppError::Code(n * 2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label(String);

impl From<String> for Label {
    fn from(s: String) -> Self {
        Label(s.to_uppercase())
    }
}

impl From<i64> for Label {
    fn from(n: i64) -> Label {
        Label(format!("#{}", n + 1))
    }
}

pub fn check_len(s: &str) -> Result<usize, LowError> {
    if s.is_empty() {
        Err(LowError::Blank)
    } else if s.chars().count() > 4 {
        Err(LowError::TooLong(s.len()))
    } else {
        Ok(s.len())
    }
}

pub fn code_of(n: u8) -> Result<u8, u8> {
    if n < 10 {
        Ok(n)
    } else {
        Err(n)
    }
}

/// `?` over three error types, each converted on its own `Err`.
pub fn read(len: &str, n: &str, id: &str) -> Result<u32, AppError> {
    let k = check_len(len)?;
    let v = n.parse::<u32>()?;
    let u = Uuid::parse_str(id)?;
    Ok(v + k as u32 + if u == Uuid::nil() { 0 } else { 1 })
}

/// The first failing `?` wins: the second operand is not evaluated.
pub fn sum(a: &str, b: &str) -> Result<i32, AppError> {
    Ok(a.parse::<i32>()? + b.parse::<i32>()?)
}

/// The conversion panics only on `Err`, after the operand ran.
pub fn coded(n: u8) -> Result<u8, AppError> {
    let c = code_of(n)?;
    Ok(c + 1)
}

/// Error types that match: unchanged.
pub fn same(n: u8) -> Result<u8, u8> {
    let c = code_of(n)?;
    Ok(c)
}

/// `map_err(..)?` next to a converting `?`.
pub fn mixed(a: &str, n: u8) -> Result<u8, AppError> {
    let x = a.parse::<u8>().map_err(|_| AppError::Number)?;
    let y = code_of(n)?;
    Ok(x.wrapping_add(y))
}

pub fn spelled(n: u8, how: u8) -> AppError {
    match how {
        0 => AppError::from(n),
        1 => From::from(n),
        _ => n.into(),
    }
}

pub fn wrap(e: LowError) -> AppError {
    e.into()
}

pub fn takes(e: AppError) -> bool {
    matches!(e, AppError::Low(_))
}

/// `into()` where a parameter gives the target.
pub fn passed(e: LowError) -> bool {
    takes(e.into())
}

pub fn labelled(s: String) -> Label {
    Label::from(s)
}

pub fn numbered(n: i64) -> Label {
    let l: Label = n.into();
    l
}
