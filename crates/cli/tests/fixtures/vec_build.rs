#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strength {
    Password,
    PasswordAndOtp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub subject: String,
    pub amr: Vec<String>,
}

/// A list the caller reads as an array, one length per arm.
pub fn amr(strength: Strength) -> Vec<String> {
    match strength {
        Strength::Password => vec![String::from("pwd")],
        Strength::PasswordAndOtp => vec![String::from("pwd"), String::from("otp"), String::from("mfa")],
    }
}

/// Built in a struct literal; the element type comes from the field.
pub fn issue(subject: String, strength: Strength) -> Grant {
    Grant { subject, amr: amr(strength) }
}

/// Untyped literals take the element type from the return type.
pub fn wide() -> Vec<u64> {
    vec![0, 1, 18_446_744_073_709_551_615]
}

/// `vec![]` next to a non-empty branch, and a trailing comma.
pub fn upto(n: u8) -> Vec<u8> {
    if n == 0u8 {
        vec![]
    } else if n == 1u8 {
        vec![1u8,]
    } else {
        vec![1u8, n]
    }
}

pub fn nested(a: i32, b: i32) -> Vec<Vec<i32>> {
    vec![vec![a], vec![], vec![a, b]]
}

/// Elements are evaluated left to right: when both overflow, the first
/// panic is the one reported.
pub fn ordered(a: i32, b: i32) -> Vec<i32> {
    vec![a + b, a * b, a - b]
}

/// A built `Vec` read back with `len`, indexing and `is_empty`.
pub fn reread(a: char, b: char) -> usize {
    let xs = vec![a, b, a];
    let empty: Vec<char> = vec![];
    if xs[1] == b && empty.is_empty() {
        xs.len()
    } else {
        0
    }
}

/// Moved values go in whole; the `Vec` owns them.
pub fn optional(x: Option<String>, y: Option<String>) -> Vec<Option<String>> {
    vec![x, None, y]
}
