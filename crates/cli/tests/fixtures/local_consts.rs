// `const` items inside function bodies.

const BASE: u32 = 100;

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum Level {
    Low = 1,
    High = 4,
}

// Used before its declaration, and referring to a crate const and to a
// discriminant.
pub fn limit(x: u32) -> u32 {
    let doubled = x * FACTOR;
    const FACTOR: u32 = BASE / 50 + Level::High as u32;
    doubled + FACTOR
}

// Unused, and a float.
pub fn scale(x: f64) -> f64 {
    const UNUSED: i32 = 7;
    const RATE: f64 = 0.25;
    x * RATE
}

// Named like a crate item, and inside a match arm's block.
pub fn classify(n: u8) -> Level {
    match n {
        0 => Level::Low,
        _ => {
            const BASE: u8 = 3;
            if n > BASE {
                Level::High
            } else {
                Level::Low
            }
        }
    }
}

// Overflows at run time with the const's value, as Rust does.
pub fn bump(x: u8) -> u8 {
    const STEP: u8 = 200;
    x + STEP
}
