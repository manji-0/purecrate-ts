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
// - A `Ulid` is a closed type: one `u128`, the time (48 bits) followed by
//   the randomness (80 bits). Values come only from `Ulid::new`,
//   `Ulid::parse`, `Ulid::from_bytes`, and `next`, so every value is one of
//   the 2^128 ULIDs.
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
// - The subset has no `to_be_bytes` / `from_be_bytes`, so the bytes are
//   shifted in and out one at a time.

use std::cmp::Ordering;

const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const MAX_TIMESTAMP: u64 = (1 << 48) - 1;
const RANDOMNESS: u128 = (1 << 80) - 1;
const ENCODED_LEN: usize = 26;
const RANDOMNESS_LEN: usize = 10;
const BINARY_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ulid(u128);

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
        let mut value = u128::from(timestamp_ms);
        for b in randomness {
            value = (value << 8) | u128::from(*b);
        }
        Ok(Ulid(value))
    }

    pub fn parse(text: &str) -> Result<Ulid, UlidError> {
        if text.len() != ENCODED_LEN {
            return Err(UlidError::WrongLength);
        }
        let mut value: u128 = 0;
        let mut first: usize = 0;
        let mut position: usize = 0;
        for c in text.chars() {
            let upper = c.to_ascii_uppercase();
            let v = match ALPHABET.iter().position(|&a| char::from(a) == upper) {
                Some(v) => v,
                None => return Err(UlidError::InvalidCharacter { position }),
            };
            if position == 0 {
                first = v;
            }
            value = (value << 5) | v as u128;
            position += 1;
        }
        if first > 7 {
            return Err(UlidError::Overflow);
        }
        Ok(Ulid(value))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Ulid, UlidError> {
        if bytes.len() != BINARY_LEN {
            return Err(UlidError::WrongByteLength);
        }
        let mut value: u128 = 0;
        for b in bytes {
            value = (value << 8) | u128::from(*b);
        }
        Ok(Ulid(value))
    }

    pub fn encode(&self) -> String {
        let mut out = String::new();
        for i in 0..26u32 {
            out.push(char::from(ALPHABET[((self.0 >> (125 - 5 * i)) & 31) as usize]));
        }
        out
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for i in 0..16u32 {
            out.push((self.0 >> (120 - 8 * i)) as u8);
        }
        out
    }

    pub fn timestamp_ms(&self) -> u64 {
        (self.0 >> 80) as u64
    }

    pub fn randomness(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for i in 0..10u32 {
            out.push((self.0 >> (72 - 8 * i)) as u8);
        }
        out
    }

    fn increment(&self) -> Result<Ulid, UlidError> {
        if self.0 & RANDOMNESS == RANDOMNESS {
            return Err(UlidError::RandomnessExhausted);
        }
        Ok(Ulid(self.0 + 1))
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
    a.0.cmp(&b.0)
}
