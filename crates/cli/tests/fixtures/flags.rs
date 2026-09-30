// Permission bits in the shape of Stoat's channel permissions: a `u64`
// enum of flags, sets built from them in consts, and overrides applied with
// bit operators.

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    ViewChannel = 1 << 0,
    ReadMessageHistory = 1 << 1,
    SendMessage = 1 << 2,
    ManageMessages = 1 << 3,
    ManageChannel = 1 << 4,
    Connect = 1 << 20,
    Speak = 1 << 21,
    Masquerade = 1 << 28,
    GrantAll = 1 << 63,
}

pub const DEFAULT_PERMISSIONS: u64 = Permission::ViewChannel as u64
    | Permission::ReadMessageHistory as u64
    | Permission::SendMessage as u64
    | Permission::Connect as u64
    | Permission::Speak as u64;

/// What a member in timeout keeps.
pub const ALLOW_IN_TIMEOUT: u64 = Permission::ViewChannel as u64 | Permission::ReadMessageHistory as u64;

pub const ALL_BITS: u64 = !0;
pub const MAX_OVERRIDES: usize = 16;
pub const HALF: usize = MAX_OVERRIDES / 2 - 1;
pub const ROLE_PREFIX: &str = "role:";
pub const SEPARATOR: char = ':';
pub const STRICT: bool = !false && true;
pub const RATIO: f64 = -0.5;
pub const OFFSET: i32 = -(1 << 4) + 3;
pub const WRAPPED: u8 = 0x81 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Override {
    pub allow: u64,
    pub deny: u64,
}

/// Implicit discriminants follow the explicit ones: 0, 5, 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Low,
    Mid = 5,
    High,
}

#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    Negative = -1,
    Zero,
    Positive,
}

pub fn bit(p: Permission) -> u64 {
    p as u64
}

pub fn has(perms: u64, p: Permission) -> bool {
    perms & (p as u64) == p as u64
}

pub fn apply(perms: u64, o: Override) -> u64 {
    (perms | o.allow) & !o.deny
}

pub fn in_timeout(perms: u64) -> u64 {
    perms & ALLOW_IN_TIMEOUT
}

pub fn defaults() -> u64 {
    DEFAULT_PERMISSIONS
}

pub fn everything() -> u64 {
    ALL_BITS
}

pub fn room(used: usize) -> bool {
    used < MAX_OVERRIDES && used != HALF
}

pub fn is_role(name: &str) -> bool {
    name.starts_with(ROLE_PREFIX)
}

pub fn is_separator(c: char) -> bool {
    c == SEPARATOR
}

pub fn strict() -> bool {
    STRICT
}

pub fn scaled(x: f64) -> f64 {
    x * RATIO
}

pub fn shifted(x: i32) -> i32 {
    x + OFFSET
}

pub fn wrapped() -> u8 {
    WRAPPED
}

pub fn level_value(l: Level) -> i32 {
    l as i32
}

/// Every discriminant fits `u8`, so the cast is accepted.
pub fn level_byte(l: Level) -> u8 {
    l as u8
}

pub fn sign_value(s: Sign) -> i64 {
    s as i64
}

/// A const is a value like any other: compared, and read in a `match` arm.
pub fn classify(perms: u64) -> u8 {
    match perms {
        0 => 0u8,
        _ => {
            if perms == DEFAULT_PERMISSIONS {
                1u8
            } else if perms & Permission::GrantAll as u64 != 0 {
                2u8
            } else {
                3u8
            }
        }
    }
}
