// ULID: Universally Unique Lexicographically Sortable Identifier.
//
// Written from the specification at https://github.com/ulid/spec (README):
// 128 bits, a 48-bit UNIX time in milliseconds followed by 80 bits of
// randomness; canonically 26 characters of Crockford's base32
// (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`), 10 for the time and 16 for the
// randomness; binary form 16 octets, most significant byte first.
//
// What the model checks and leaves out
//
// - A `Ulid` is a closed type: two `u64`s, `hi` = time (48 bits) followed by
//   the top 16 bits of the randomness, `lo` = the low 64 bits of the
//   randomness. Values come only from `Ulid::new`, `Ulid::parse`,
//   `Ulid::from_bytes`, and `next`, so every value is one of the 2^128 ULIDs.
// - There is no random source in the subset. The caller supplies the 80 bits
//   of randomness as 10 bytes (from a CSPRNG); any other length is refused.
// - `Ulid::new` refuses a timestamp >= 2^48 (the spec's maximum is
//   2^48 - 1 = 281474976710655 ms).
// - Encoding is uppercase. Parsing is case-insensitive, as the spec says
//   ("Case insensitive"). The spec names no aliases, only that I, L, O and U
//   are excluded from the alphabet, so they are refused rather than read as
//   1, 1, 0 (Crockford's own decoding table) or as anything else.
// - Parsing checks, in this order: length (26 bytes), each character in turn
//   (the first invalid one is reported with its character position), then
//   overflow: a first character above '7' is more than 128 bits and is
//   refused, as the spec asks.
// - Monotonicity: `next(previous, timestamp_ms, fresh_randomness)` is the
//   spec's monotonic factory as a pure function. When the timestamp is the
//   same as the previous ULID's, or earlier (a clock that went backwards),
//   the result is the previous ULID with its 80-bit randomness incremented by
//   1 in the least significant bit, with carry, keeping the previous
//   timestamp so order is preserved. When all 80 bits are already 1 that is
//   `RandomnessExhausted` (the spec: "the generation will fail"). A later
//   timestamp starts over from the fresh randomness. The fresh randomness and
//   the timestamp are validated on every call, even when not used.
// - Order: `compare` is numeric order of the 128 bits, which is the order of
//   the binary form (big-endian bytes) and of the canonical strings (the
//   alphabet is in ASCII order and every string has 26 characters).
// - Not modelled: the clock, the RNG, UUID conversion, serde. A plain serde
//   derive would read a `Ulid` by shape, so wire it as the canonical string
//   through `Ulid::parse` instead.
// - The subset has no narrowing conversion between integer types, so bytes
//   are built bit by bit (`low_byte`) and base32 digits are read and written
//   through `match` tables instead of an indexed alphabet.

use std::cmp::Ordering;

const MAX_TIMESTAMP: u64 = (1 << 48) - 1;
const ALL_ONES: u64 = 0xFFFF_FFFF_FFFF_FFFF;
const LOW_16: u64 = 0xFFFF;
const ENCODED_LEN: usize = 26;
const RANDOMNESS_LEN: usize = 10;
const BINARY_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ulid {
    hi: u64,
    lo: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UlidError {
    TimestampTooLarge,
    WrongRandomnessLength,
    WrongLength,
    InvalidCharacter { position: usize },
    Overflow,
    WrongByteLength,
    RandomnessExhausted,
}

impl Ulid {
    pub fn new(timestamp_ms: u64, randomness: &[u8]) -> Result<Ulid, UlidError> {
        if timestamp_ms > MAX_TIMESTAMP {
            return Err(UlidError::TimestampTooLarge);
        }
        if randomness.len() != RANDOMNESS_LEN {
            return Err(UlidError::WrongRandomnessLength);
        }
        let hi = (timestamp_ms << 16) | (u64::from(randomness[0]) << 8) | u64::from(randomness[1]);
        let mut lo: u64 = 0;
        for i in 2..RANDOMNESS_LEN {
            lo = (lo << 8) | u64::from(randomness[i]);
        }
        Ok(Ulid { hi, lo })
    }

    pub fn parse(text: &str) -> Result<Ulid, UlidError> {
        if text.len() != ENCODED_LEN {
            return Err(UlidError::WrongLength);
        }
        let mut hi: u64 = 0;
        let mut lo: u64 = 0;
        let mut first: u64 = 0;
        let mut position: usize = 0;
        for c in text.chars() {
            let v = match symbol_value(c) {
                Some(v) => v,
                None => return Err(UlidError::InvalidCharacter { position }),
            };
            if position == 0 {
                first = v;
            }
            hi = (hi << 5) | (lo >> 59);
            lo = (lo << 5) | v;
            position += 1;
        }
        if first > 7 {
            return Err(UlidError::Overflow);
        }
        Ok(Ulid { hi, lo })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Ulid, UlidError> {
        if bytes.len() != BINARY_LEN {
            return Err(UlidError::WrongByteLength);
        }
        let mut hi: u64 = 0;
        let mut lo: u64 = 0;
        for i in 0..8usize {
            hi = (hi << 8) | u64::from(bytes[i]);
            lo = (lo << 8) | u64::from(bytes[i + 8]);
        }
        Ok(Ulid { hi, lo })
    }

    pub fn encode(&self) -> String {
        let mut out = String::new();
        for i in 0..26u64 {
            let shift = 125 - 5 * i;
            out.push(value_symbol(group(self.hi, self.lo, shift)));
        }
        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for i in 0..8u64 {
            out.push(low_byte(self.hi >> (56 - 8 * i)));
        }
        for i in 0..8u64 {
            out.push(low_byte(self.lo >> (56 - 8 * i)));
        }
        out
    }

    pub fn timestamp_ms(&self) -> u64 {
        self.hi >> 16
    }

    pub fn randomness(&self) -> Vec<u8> {
        let bytes = self.to_bytes();
        let mut out: Vec<u8> = Vec::new();
        for i in 6..BINARY_LEN {
            out.push(bytes[i]);
        }
        out
    }

    fn increment(&self) -> Result<Ulid, UlidError> {
        if self.lo != ALL_ONES {
            return Ok(Ulid { hi: self.hi, lo: self.lo + 1 });
        }
        if self.hi & LOW_16 == LOW_16 {
            return Err(UlidError::RandomnessExhausted);
        }
        Ok(Ulid { hi: self.hi + 1, lo: 0 })
    }
}

pub fn next(previous: Option<Ulid>, timestamp_ms: u64, fresh_randomness: &[u8]) -> Result<Ulid, UlidError> {
    let fresh = Ulid::new(timestamp_ms, fresh_randomness)?;
    match previous {
        Some(p) => {
            if timestamp_ms <= p.timestamp_ms() {
                p.increment()
            } else {
                Ok(fresh)
            }
        }
        None => Ok(fresh),
    }
}

pub fn compare(a: &Ulid, b: &Ulid) -> Ordering {
    a.hi.cmp(&b.hi).then(a.lo.cmp(&b.lo))
}

// The 5 bits of the 130-bit number (two leading zero bits, then hi, then lo)
// whose lowest bit is `shift` bits from the bottom.
fn group(hi: u64, lo: u64, shift: u64) -> u64 {
    if shift >= 64 {
        (hi >> (shift - 64)) & 31
    } else if shift + 5 <= 64 {
        (lo >> shift) & 31
    } else {
        ((lo >> shift) | (hi << (64 - shift))) & 31
    }
}

// The low 8 bits of `v` as a `u8`, built bit by bit.
fn low_byte(v: u64) -> u8 {
    let mut b: u8 = 0;
    for k in 0..8u64 {
        if (v >> (7 - k)) & 1 == 1 {
            b = b * 2 + 1;
        } else {
            b = b * 2;
        }
    }
    b
}

fn value_symbol(v: u64) -> char {
    match v {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        9 => '9',
        10 => 'A',
        11 => 'B',
        12 => 'C',
        13 => 'D',
        14 => 'E',
        15 => 'F',
        16 => 'G',
        17 => 'H',
        18 => 'J',
        19 => 'K',
        20 => 'M',
        21 => 'N',
        22 => 'P',
        23 => 'Q',
        24 => 'R',
        25 => 'S',
        26 => 'T',
        27 => 'V',
        28 => 'W',
        29 => 'X',
        30 => 'Y',
        31 => 'Z',
        _ => '?',
    }
}

fn symbol_value(c: char) -> Option<u64> {
    match c.to_ascii_uppercase() {
        '0' => Some(0),
        '1' => Some(1),
        '2' => Some(2),
        '3' => Some(3),
        '4' => Some(4),
        '5' => Some(5),
        '6' => Some(6),
        '7' => Some(7),
        '8' => Some(8),
        '9' => Some(9),
        'A' => Some(10),
        'B' => Some(11),
        'C' => Some(12),
        'D' => Some(13),
        'E' => Some(14),
        'F' => Some(15),
        'G' => Some(16),
        'H' => Some(17),
        'J' => Some(18),
        'K' => Some(19),
        'M' => Some(20),
        'N' => Some(21),
        'P' => Some(22),
        'Q' => Some(23),
        'R' => Some(24),
        'S' => Some(25),
        'T' => Some(26),
        'V' => Some(27),
        'W' => Some(28),
        'X' => Some(29),
        'Y' => Some(30),
        'Z' => Some(31),
        _ => None,
    }
}
