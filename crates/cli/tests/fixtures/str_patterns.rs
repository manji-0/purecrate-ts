pub enum Method {
    Card,
    Bank,
    Wallet(u8),
}

/// The idiomatic parse of a wire tag: arms tried in order, `|`, a last `_`.
pub fn method_of(s: &str) -> Option<Method> {
    match s {
        "card" | "credit_card" => Some(Method::Card),
        "bank" => Some(Method::Bank),
        "wallet" => Some(Method::Wallet(0u8)),
        "" => None,
        _ => None,
    }
}

/// A `String` matched through `as_str`, and literals past ASCII.
pub fn owned(s: String) -> u32 {
    match s.as_str() {
        "é" => 1u32,
        "e\u{301}" => 2u32,
        "日本" => 3u32,
        "😀" => 4u32,
        "\u{ffff}" => 5u32,
        "a\"b\\c\n" => 6u32,
        _ => 0u32,
    }
}

/// `matches!` with string literals, and a string match nested in an enum arm.
pub fn score(s: &str) -> u32 {
    let n: u32 = match method_of(s) {
        Some(m) => match m {
            Method::Wallet(k) => u32::from(k) + 100u32,
            _ => match s {
                "card" => 10u32,
                _ => 20u32,
            },
        },
        None => 0u32,
    };
    if matches!(s, "wallet" | "bank") {
        n + 1u32
    } else {
        n
    }
}
