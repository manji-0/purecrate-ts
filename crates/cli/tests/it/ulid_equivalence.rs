//! `examples/ulid`, the ULID specification (github.com/ulid/spec, README) on
//! two `u64`s:
//!
//! - the README's values come out as published: `01ARZ3NDEKTSV4RRFFQ69G5FAV`
//!   parses, re-encodes to itself, parses from lowercase, and carries the
//!   timestamp 1469922850259; `7ZZZZZZZZZZZZZZZZZZZZZZZZZ` is the largest
//!   ULID (time 2^48 - 1) and `80000000000000000000000000` overflows; the
//!   monotonic sequence `…VRY`, `…VRZ`, `…VS0`, `…VS1` and `…ZZX`, `…ZZY`,
//!   `…ZZZ`, then failure, in the same millisecond; the binary form is the
//!   48-bit time then the 80-bit randomness, most significant byte first;
//!   string order, byte order and `compare` are time order first; every
//!   error the model's header lists comes out where it says;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against: one `u128`, `to_be_bytes` / `from_be_bytes`, an
//!   indexed `ALPHABET`, `as` casts, a derived `Ord`) agree on: every
//!   single-byte substitution (all 256 values, non-ASCII ones as the
//!   Latin-1 character) at every position of a set of ULIDs, upper- and
//!   lowercase, and multi-byte characters standing in for two to four
//!   characters so the byte length stays 26; every prefix and extension of
//!   length 0..=30; boundary values (0, time 2^48 - 1, all-ones randomness,
//!   the top bit of `lo`, carries across bit 64 and across the 16/64 split
//!   of the randomness); seeded random ULIDs through encode, parse,
//!   lowercase parse, bytes and `new`, and pairwise through `compare`,
//!   string order and byte order; `next` chains with the same, earlier and
//!   later timestamps, near-exhausted randomness, and invalid inputs mixed
//!   in; byte inputs of length 0..=20; `Ulid::new` with randomness of length
//!   0..=12 and timestamps around 2^48;
//! - the generated package agrees with Rust on a sample of each, with
//!   values at and above 2^53, the top bit of `lo`, all-ones, error
//!   positions and `next` at exhaustion over-represented.
//!
//! The driver prints only what a crate's free function returns, and this
//! crate's only free functions are `next` (`Result<Ulid, UlidError>`, which
//! `parse`, `new` and `from_bytes` also return) and `compare`. `encode`,
//! `to_bytes`, `randomness` and `timestamp_ms` return types no free
//! function does, so `fixtures/ulid_driver.rs` adds identity functions
//! whose printers those cases borrow; the call itself is the generated
//! method's.

use crate::support::{self, Js, Rng};

purecrate_canon::fixture!(mod ulid = "../../../examples/ulid/src/lib.rs", "fixtures/ulid_driver.rs");

use ulid::{Ulid, UlidError};

/// ULID as one would write it with a `u128`, under the model's scope
/// (uppercase encoding, case-insensitive parsing, no Crockford aliases,
/// the same errors in the same order, an earlier timestamp incremented as
/// the same millisecond). Not converted; the reference only. Types carry
/// the constrained side's names, so `Debug` compares the errors.
mod idiomatic {
    use std::cmp::Ordering;

    const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    const RANDOMNESS: u128 = (1 << 80) - 1;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
    use UlidError::*;

    impl Ulid {
        pub fn new(timestamp_ms: u64, randomness: &[u8]) -> Result<Ulid, UlidError> {
            if timestamp_ms >= 1 << 48 {
                return Err(TimestampTooLarge);
            }
            if randomness.len() != 10 {
                return Err(WrongRandomnessLength);
            }
            let mut bytes = [0u8; 16];
            bytes[6..].copy_from_slice(randomness);
            Ok(Ulid(u128::from(timestamp_ms) << 80 | u128::from_be_bytes(bytes)))
        }

        pub fn parse(text: &str) -> Result<Ulid, UlidError> {
            if text.len() != 26 {
                return Err(WrongLength);
            }
            let mut value = 0u128;
            for (position, c) in text.chars().enumerate() {
                let digit = u8::try_from(c.to_ascii_uppercase())
                    .ok()
                    .and_then(|b| ALPHABET.iter().position(|&a| a == b))
                    .ok_or(InvalidCharacter { position })?;
                value = value << 5 | digit as u128;
            }
            // Every character is now in the alphabet: above '7' is a value
            // of 8 or more in the first, 130 bits.
            if text.as_bytes()[0] > b'7' {
                return Err(Overflow);
            }
            Ok(Ulid(value))
        }

        pub fn from_bytes(bytes: &[u8]) -> Result<Ulid, UlidError> {
            let bytes: [u8; 16] = bytes.try_into().map_err(|_| WrongByteLength)?;
            Ok(Ulid(u128::from_be_bytes(bytes)))
        }

        pub fn encode(&self) -> String {
            (0..26).map(|i| ALPHABET[(self.0 >> (125 - 5 * i) & 31) as usize] as char).collect()
        }

        pub fn to_bytes(self) -> Vec<u8> {
            self.0.to_be_bytes().to_vec()
        }

        pub fn timestamp_ms(&self) -> u64 {
            (self.0 >> 80) as u64
        }

        pub fn randomness(&self) -> Vec<u8> {
            self.0.to_be_bytes()[6..].to_vec()
        }
    }

    pub fn next(previous: Option<Ulid>, timestamp_ms: u64, fresh_randomness: &[u8]) -> Result<Ulid, UlidError> {
        let fresh = Ulid::new(timestamp_ms, fresh_randomness)?;
        match previous {
            Some(p) if timestamp_ms <= p.timestamp_ms() => {
                (p.0 & RANDOMNESS).checked_add(1).filter(|&r| r <= RANDOMNESS).ok_or(RandomnessExhausted)?;
                Ok(Ulid(p.0 + 1))
            }
            _ => Ok(fresh),
        }
    }

    pub fn compare(a: &Ulid, b: &Ulid) -> Ordering {
        a.cmp(b)
    }
}

const README: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const README_TIME: u64 = 1_469_922_850_259;
/// The ULID in the README's binary layout section, spaces removed.
const LAYOUT: &str = "01AN4Z07BY79KA1307SR9X4MV3";
const MAX: &str = "7ZZZZZZZZZZZZZZZZZZZZZZZZZ";
const MAX_TIME: u64 = (1 << 48) - 1;
const MONOTONIC_TIME: u64 = 1_508_808_576_371;

fn ulid(text: &str) -> Ulid {
    Ulid::parse(text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}

fn of_bytes(bytes: &[u8; 16]) -> Ulid {
    Ulid::from_bytes(bytes).expect("16 bytes")
}

/// The 16 bytes of `v`, which is how the test builds any `Ulid`: its
/// fields are private.
fn of_u128(v: u128) -> Ulid {
    of_bytes(&v.to_be_bytes())
}

fn mirror(u: &Ulid) -> idiomatic::Ulid {
    idiomatic::Ulid::from_bytes(&u.to_bytes()).expect("16 bytes")
}

/// Everything a `Ulid` shows outside: its text, bytes, time and randomness.
type Views = (String, Vec<u8>, u64, Vec<u8>);

fn views(u: &Ulid) -> Views {
    (u.encode(), u.to_bytes(), u.timestamp_ms(), u.randomness())
}

fn reference_views(u: &idiomatic::Ulid) -> Views {
    (u.encode(), u.to_bytes(), u.timestamp_ms(), u.randomness())
}

/// Both results as text: the views of an `Ok`, the `Debug` of an `Err`.
fn agree(model: Result<Ulid, UlidError>, reference: Result<idiomatic::Ulid, idiomatic::UlidError>, what: &str) {
    let m = model.map(|u| views(&u)).map_err(|e| format!("{e:?}"));
    let r = reference.map(|u| reference_views(&u)).map_err(|e| format!("{e:?}"));
    assert_eq!(m, r, "{what}");
}

#[test]
fn the_published_examples_come_out_as_published() {
    use UlidError::*;
    let readme = ulid(README);
    assert_eq!(readme.encode(), README);
    assert_eq!(Ulid::parse(&README.to_lowercase()), Ok(readme));
    assert_eq!(Ulid::parse("01arZ3ndEKtsv4RRffq69G5FAV"), Ok(readme), "any mix of case");
    assert_eq!(readme.timestamp_ms(), README_TIME);
    let layout = ulid(LAYOUT);
    assert_eq!(layout.encode(), LAYOUT);
    assert_eq!(layout.timestamp_ms(), 1_465_824_320_894);

    // Binary layout: the time's 48 bits big-endian, then the 80 random bits.
    let bytes = readme.to_bytes();
    assert_eq!(bytes, [0x01, 0x56, 0x3e, 0x3a, 0xb5, 0xd3, 0xd6, 0x76, 0x4c, 0x61, 0xef, 0xb9, 0x93, 0x02, 0xbd, 0x5b]);
    assert_eq!(bytes[..6], README_TIME.to_be_bytes()[2..]);
    assert_eq!(readme.randomness(), bytes[6..]);
    assert_eq!(Ulid::from_bytes(&bytes), Ok(readme));
    assert_eq!(Ulid::new(README_TIME, &bytes[6..]), Ok(readme));

    // The largest ULID, and one past it.
    let max = ulid(MAX);
    assert_eq!(max.timestamp_ms(), MAX_TIME);
    assert_eq!(max.encode(), MAX);
    assert_eq!(max.to_bytes(), [0xFF; 16]);
    assert_eq!(max.randomness(), [0xFF; 10]);
    assert_eq!(Ulid::new(MAX_TIME, &[0xFF; 10]), Ok(max));
    assert_eq!(Ulid::new(MAX_TIME + 1, &[0; 10]), Err(TimestampTooLarge));
    assert_eq!(Ulid::new(u64::MAX, &[0; 3]), Err(TimestampTooLarge), "the time before the length");
    assert_eq!(Ulid::parse("80000000000000000000000000"), Err(Overflow));
    assert_eq!(Ulid::parse("zzzzzzzzzzzzzzzzzzzzzzzzzz"), Err(Overflow));
    assert_eq!(Ulid::parse("7zzzzzzzzzzzzzzzzzzzzzzzzz"), Ok(max));
    assert_eq!(Ulid::parse("8000000000000000000000000U"), Err(InvalidCharacter { position: 25 }), "characters first");
    let zero = ulid("00000000000000000000000000");
    assert_eq!(zero.to_bytes(), [0; 16]);
    assert_eq!(Ulid::new(0, &[0; 10]), Ok(zero));

    // Parse errors in the header's order.
    assert_eq!(Ulid::parse(""), Err(WrongLength));
    assert_eq!(Ulid::parse(&README[..25]), Err(WrongLength));
    assert_eq!(Ulid::parse(&format!("{README}0")), Err(WrongLength));
    assert_eq!(Ulid::parse("01ARZ3NDEKTSV4RRFFQ69G5FA\u{e9}"), Err(WrongLength), "27 bytes");
    assert_eq!(Ulid::parse("01ARZ3NDEKTSV4RRFFQ69G5F\u{e9}"), Err(InvalidCharacter { position: 24 }), "26 bytes");
    for (k, bad) in ["I", "L", "O", "U", "i", "l", "o", "u", "-", " ", "\u{0}"].iter().enumerate() {
        let at = k * 2 + 1;
        let text = format!("{}{bad}{}", &README[..at], &README[at + 1..]);
        assert_eq!(Ulid::parse(&text), Err(InvalidCharacter { position: at }), "{text:?}");
    }
    assert_eq!(Ulid::parse("0IARZ3NDEKTSV4RRFFQ69G5FAU"), Err(InvalidCharacter { position: 1 }), "the first one");
    assert_eq!(Ulid::from_bytes(&[0; 15]), Err(WrongByteLength));
    assert_eq!(Ulid::from_bytes(&[0; 17]), Err(WrongByteLength));
    assert_eq!(Ulid::new(0, &[0; 9]), Err(WrongRandomnessLength));
    assert_eq!(Ulid::new(0, &[0; 11]), Err(WrongRandomnessLength));

    // Monotonicity, the README's two sequences in one millisecond.
    let any = [0x5A; 10];
    let mut previous = ulid("01BX5ZZKBKACTAV9WEVGEMMVRY");
    assert_eq!(previous.timestamp_ms(), MONOTONIC_TIME);
    for expected in ["01BX5ZZKBKACTAV9WEVGEMMVRZ", "01BX5ZZKBKACTAV9WEVGEMMVS0", "01BX5ZZKBKACTAV9WEVGEMMVS1"] {
        previous = ulid::next(Some(previous), MONOTONIC_TIME, &any).expect("same ms");
        assert_eq!(previous.encode(), expected);
    }
    let mut previous = ulid("01BX5ZZKBKZZZZZZZZZZZZZZZX");
    for expected in ["01BX5ZZKBKZZZZZZZZZZZZZZZY", "01BX5ZZKBKZZZZZZZZZZZZZZZZ"] {
        previous = ulid::next(Some(previous), MONOTONIC_TIME, &any).expect("same ms");
        assert_eq!(previous.encode(), expected);
    }
    assert_eq!(ulid::next(Some(previous), MONOTONIC_TIME, &any), Err(RandomnessExhausted));
    assert_eq!(ulid::next(Some(previous), MONOTONIC_TIME - 1, &any), Err(RandomnessExhausted), "a clock behind");
    let later = ulid::next(Some(previous), MONOTONIC_TIME + 1, &any).expect("a later ms starts over");
    assert_eq!(later, Ulid::new(MONOTONIC_TIME + 1, &any).expect("valid"));
    // A clock that went back keeps the previous time.
    let back = ulid::next(Some(ulid("01BX5ZZKBKACTAV9WEVGEMMVRZ")), 5, &any).expect("increments");
    assert_eq!(back.encode(), "01BX5ZZKBKACTAV9WEVGEMMVS0");
    // The carry crosses the 64-bit word: lo all ones, hi's random bits not.
    let carry = of_u128((u128::from(MONOTONIC_TIME) << 80) | (0x1234 << 64) | u128::from(u64::MAX));
    let carried = ulid::next(Some(carry), MONOTONIC_TIME, &any).expect("carries");
    assert_eq!(carried, of_u128((u128::from(MONOTONIC_TIME) << 80) | (0x1235 << 64)));
    // The fresh inputs are checked even when not used.
    assert_eq!(ulid::next(Some(readme), README_TIME, &[0; 9]), Err(WrongRandomnessLength));
    assert_eq!(ulid::next(Some(readme), 1 << 48, &any), Err(TimestampTooLarge));
    assert_eq!(ulid::next(None, README_TIME, &any), Ulid::new(README_TIME, &any));

    // Lexicographic order is time order, then randomness.
    let ordered = [
        "00000000000000000000000000",
        "00000000000000000000000001",
        "0000000001ZZZZZZZZZZZZZZZZ",
        "00000000020000000000000000",
        LAYOUT,
        README,
        "01BX5ZZKBKACTAV9WEVGEMMVRZ",
        "01BX5ZZKBKZZZZZZZZZZZZZZZZ",
        "7ZZZZZZZZZ0000000000000000",
        MAX,
    ];
    let ordered: Vec<Ulid> = ordered.iter().map(|t| ulid(t)).collect();
    for (i, a) in ordered.iter().enumerate() {
        for (j, b) in ordered.iter().enumerate() {
            assert_eq!(ulid::compare(a, b), i.cmp(&j), "{} {}", a.encode(), b.encode());
            assert_eq!(a.encode().cmp(&b.encode()), i.cmp(&j));
            assert_eq!(a.to_bytes().cmp(&b.to_bytes()), i.cmp(&j));
            if a.timestamp_ms() != b.timestamp_ms() {
                assert_eq!(a.timestamp_ms().cmp(&b.timestamp_ms()), i.cmp(&j));
            }
        }
    }
}

/// ULIDs whose single-character changes are tried.
fn bases() -> Vec<String> {
    let mut out: Vec<String> = [
        README,
        LAYOUT,
        MAX,
        "00000000000000000000000000",
        "01BX5ZZKBKZZZZZZZZZZZZZZZZ",
        "7ZZZZZZZZZ0000000000000000",
        "0123456789ABCDEFGHJKMNPQRS",
        "7TVWXYZ0000000000000000000",
    ]
    .map(String::from)
    .into();
    out.extend(out.clone().iter().map(|s| s.to_lowercase()));
    out
}

/// Every byte value at every position: ASCII ones in place, the others as
/// the Latin-1 character (two bytes, so a length error), plus multi-byte
/// characters replacing two to four characters (26 bytes, a character
/// error at their position).
fn mutations() -> Vec<String> {
    let wide = ['\u{e9}', '\u{131}', '\u{17f}', '\u{212a}', '\u{ff10}', '\u{3007}', '\u{1d7ce}'];
    let mut out = Vec::new();
    for base in bases() {
        let chars: Vec<char> = base.chars().collect();
        for at in 0..chars.len() {
            for b in 0..=255u8 {
                let mut cs = chars.clone();
                cs[at] = char::from(b);
                out.push(cs.into_iter().collect());
            }
            for w in wide {
                let take = w.len_utf8();
                if at + take <= chars.len() {
                    let text: String = chars[..at].iter().chain([w].iter()).chain(chars[at + take..].iter()).collect();
                    assert_eq!(text.len(), 26);
                    out.push(text);
                }
            }
        }
    }
    out
}

/// Every prefix and extension of the bases, lengths 0..=30.
fn lengths() -> Vec<String> {
    let mut out = Vec::new();
    for base in bases() {
        for n in 0..=26 {
            out.push(base[..n].to_string());
        }
        for extra in ["0", "Z", "00", "0Z", "ZZZZ", "\u{e9}"] {
            out.push(format!("{base}{extra}"));
            out.push(format!("{extra}{base}"));
        }
    }
    out
}

/// Boundary values as `u128`s: zero, the maximum, time edges, all-ones
/// randomness, the top bit of `lo`, values around 2^53 in either word, and
/// both sides of the carries across bit 64 and across bit 80.
fn boundaries() -> Vec<u128> {
    let words =
        [0u64, 1, 0x7FFF, 0x8000, 0xFFFF, (1 << 53) - 1, 1 << 53, (1 << 53) + 1, 1 << 63, u64::MAX - 1, u64::MAX];
    let mut out = vec![0, 1, u128::MAX, u128::MAX - 1, u128::MAX >> 1, 1 << 127, 1 << 80, (1 << 80) - 1];
    for time in [0u128, 1, 0x7FFF_FFFF, 1 << 47, MAX_TIME as u128 - 1, MAX_TIME as u128] {
        for high in [0u128, 1, 0x7FFF, 0x8000, 0xFFFE, 0xFFFF] {
            for &lo in &words {
                out.push(time << 80 | high << 64 | u128::from(lo));
            }
        }
    }
    for &hi in &words {
        for &lo in &words {
            out.push(u128::from(hi) << 64 | u128::from(lo));
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn random_u128(rng: &mut Rng) -> u128 {
    let v = u128::from(rng.next()) << 64 | u128::from(rng.next());
    match rng.below(6) {
        // Near all-ones randomness.
        0 => v | ((1 << 80) - 1) >> rng.below(12),
        // A small time.
        1 => v & ((1 << 100) - 1),
        _ => v,
    }
}

fn randoms() -> Vec<u128> {
    let mut rng = Rng::new(0x0071_d000);
    (0..5000).map(|_| random_u128(&mut rng)).collect()
}

/// A step of a `next` chain: previous, timestamp, fresh randomness.
type Step = (Option<u128>, u64, Vec<u8>);

/// Chains of `next` from both sides, recording each step's inputs. The
/// timestamp moves by the same, earlier or later; the randomness is
/// sometimes near all ones; inputs are sometimes invalid. An error leaves
/// the previous ULID as it was.
fn chains() -> Vec<(Step, Result<Ulid, UlidError>)> {
    let mut rng = Rng::new(0x0071_dc4a);
    let mut out = Vec::new();
    for chain in 0..6 {
        let mut time: u64 = match chain {
            0 => 0,
            1 => MAX_TIME - 4000,
            2 => 1 << 53 >> 16,
            _ => rng.below(MAX_TIME),
        };
        let mut previous: Option<Ulid> = if chain % 2 == 0 { None } else { Some(of_u128(u128::from(time) << 80)) };
        for _ in 0..3000 {
            // The clock drifts forward; a step now and then reads a
            // timestamp out of range or at the maximum without moving it.
            time = match rng.below(10) {
                0..=4 => time,
                5 => time.saturating_sub(1 + rng.below(3)),
                _ => (time + 1 + rng.below(3)).min(MAX_TIME),
            };
            let at = match rng.below(40) {
                0 => rng.pick(&[MAX_TIME + 1, 1 << 63, u64::MAX]),
                _ => time,
            };
            let mut fresh: Vec<u8> = (0..10).map(|_| rng.below(256) as u8).collect();
            match rng.below(12) {
                0 => fresh.iter_mut().take(9).for_each(|b| *b = 0xFF),
                1 => fresh = vec![0xFF; 10],
                2 => fresh.truncate(rng.below(10) as usize),
                3 => fresh.push(0),
                _ => {}
            }
            // Jump the previous one to the edge of exhaustion now and then.
            if rng.below(60) == 0 {
                if let Some(p) = previous {
                    let edge = (u128::from(p.timestamp_ms()) << 80) | ((1 << 80) - 1 - u128::from(rng.below(3)));
                    previous = Some(of_u128(edge));
                }
            }
            let step = (previous.map(|p| u128::from_be_bytes(p.to_bytes().try_into().expect("16"))), at, fresh);
            let result = ulid::next(previous, step.1, &step.2);
            if let Ok(u) = result {
                previous = Some(u);
            }
            out.push((step, result));
        }
    }
    out
}

fn timestamps() -> Vec<u64> {
    vec![0, 1, (1 << 32) - 1, MAX_TIME - 1, MAX_TIME, MAX_TIME + 1, MAX_TIME + 2, 1 << 53, 1 << 63, u64::MAX]
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    // Single-character changes, case included.
    let mutations = mutations();
    let mut kinds = [0usize; 4];
    for text in &mutations {
        let model = Ulid::parse(text);
        kinds[match model {
            Ok(_) => 0,
            Err(UlidError::InvalidCharacter { .. }) => 1,
            Err(UlidError::WrongLength) => 2,
            _ => 3,
        }] += 1;
        agree(model, idiomatic::Ulid::parse(text), &format!("parse({text:?})"));
    }
    assert!(mutations.len() > 100_000 && kinds.iter().all(|&k| k > 500), "{} mutations: {kinds:?}", mutations.len());
    for text in lengths() {
        agree(Ulid::parse(&text), idiomatic::Ulid::parse(&text), &format!("parse({text:?})"));
    }

    // Boundaries and random values, every way in and out.
    let values: Vec<u128> = boundaries().into_iter().chain(randoms()).collect();
    for &v in &values {
        let bytes = v.to_be_bytes();
        let u = of_u128(v);
        let r = idiomatic::Ulid::from_bytes(&bytes).expect("16 bytes");
        assert_eq!(views(&u), reference_views(&r), "{v:#x}");
        let text = u.encode();
        assert_eq!(Ulid::parse(&text), Ok(u), "{text}");
        assert_eq!(Ulid::parse(&text.to_lowercase()), Ok(u), "{text}");
        assert_eq!(Ulid::from_bytes(&u.to_bytes()), Ok(u));
        assert_eq!(Ulid::new(u.timestamp_ms(), &u.randomness()), Ok(u));
        assert_eq!(u.to_bytes(), bytes);
        agree(Ulid::parse(&text), idiomatic::Ulid::parse(&text), &text);
        agree(
            Ulid::new(u.timestamp_ms(), &u.randomness()),
            idiomatic::Ulid::new(r.timestamp_ms(), &r.randomness()),
            &text,
        );
        // Whatever the time, a `next` at the same or an earlier one.
        for time in [u.timestamp_ms(), u.timestamp_ms().saturating_sub(1), u.timestamp_ms() + 1, 0] {
            agree(
                ulid::next(Some(u), time, &[7; 10]),
                idiomatic::next(Some(r), time, &[7; 10]),
                &format!("next({text}, {time})"),
            );
        }
    }

    // Order: model, reference, string and bytes, on neighbours of a
    // sorted mix and on every pair of a subset.
    let mut sorted = values.clone();
    sorted.sort_unstable();
    let pairs = sorted.windows(2).map(|w| (w[0], w[1])).chain(values.windows(2).map(|w| (w[0], w[1])));
    let small: Vec<u128> = values.iter().copied().step_by(37).collect();
    let all = small.iter().flat_map(|&a| small.iter().map(move |&b| (a, b)));
    let mut compared = 0;
    for (a, b) in pairs.chain(all) {
        let (x, y) = (of_u128(a), of_u128(b));
        let order = ulid::compare(&x, &y);
        assert_eq!(order, a.cmp(&b), "{a:#x} {b:#x}");
        assert_eq!(order, idiomatic::compare(&mirror(&x), &mirror(&y)));
        assert_eq!(order, x.encode().cmp(&y.encode()));
        assert_eq!(order, x.to_bytes().cmp(&y.to_bytes()));
        compared += 1;
    }
    assert!(compared > 30_000, "{compared} pairs");

    // `next` chains.
    let chains = chains();
    let (mut exhausted, mut increments) = (0, 0);
    for ((previous, time, fresh), model) in &chains {
        let p = previous.map(|v| mirror(&of_u128(v)));
        exhausted += usize::from(*model == Err(UlidError::RandomnessExhausted));
        increments += usize::from(previous.is_some_and(|v| *time as u128 <= v >> 80) && model.is_ok());
        agree(*model, idiomatic::next(p, *time, fresh), &format!("next({previous:?}, {time}, {fresh:?})"));
    }
    assert!(exhausted > 20 && increments > 5000, "{exhausted} exhausted, {increments} increments");

    // Byte inputs and `new` around the limits.
    let mut rng = Rng::new(0x0071_db00);
    for len in 0..=20 {
        for _ in 0..20 {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    let any = rng.below(256) as u8;
                    rng.pick(&[0, 1, 0x7F, 0x80, 0xFE, 0xFF, any])
                })
                .collect();
            agree(Ulid::from_bytes(&bytes), idiomatic::Ulid::from_bytes(&bytes), &format!("from_bytes({bytes:?})"));
        }
    }
    for len in 0..=12 {
        for fill in [0u8, 0x80, 0xFF] {
            let randomness = vec![fill; len];
            for time in timestamps() {
                agree(
                    Ulid::new(time, &randomness),
                    idiomatic::Ulid::new(time, &randomness),
                    &format!("new({time}, {randomness:?})"),
                );
                agree(
                    ulid::next(Some(ulid(MAX)), time, &randomness),
                    idiomatic::next(Some(mirror(&ulid(MAX))), time, &randomness),
                    &format!("next(MAX, {time}, {randomness:?})"),
                );
            }
        }
    }
}

/// `Ulid::parse` returns what `next` does: the case borrows its printer.
fn parse_case(text: String) -> support::Case {
    support::run("next", format!("pkg.Ulid.parse({})", text.js()), move || Ulid::parse(&text))
}

fn new_case(time: u64, randomness: Vec<u8>) -> support::Case {
    let call = format!("pkg.Ulid.new({}, {})", time.js(), randomness.js());
    support::run("next", call, move || Ulid::new(time, &randomness))
}

fn from_bytes_case(bytes: Vec<u8>) -> support::Case {
    support::run("next", format!("pkg.Ulid.fromBytes({})", bytes.js()), move || Ulid::from_bytes(&bytes))
}

/// `encode`, `to_bytes`, `randomness`, `timestamp_ms` and the round trips
/// back through `parse` and `from_bytes`, each the generated method,
/// printed through the driver's identity functions.
fn view_cases(cases: &mut Vec<support::Case>, u: Ulid) {
    let js = u.js();
    cases.push(support::run("show_text", format!("showText(pkg.Ulid.encode({js}))"), move || u.encode()));
    cases.push(support::run("show_bytes", format!("showBytes(pkg.Ulid.toBytes({js}))"), move || u.to_bytes()));
    cases.push(support::run("show_bytes", format!("showBytes(pkg.Ulid.randomness({js}))"), move || u.randomness()));
    cases.push(support::run("show_u64", format!("showU64(pkg.Ulid.timestampMs({js}))"), move || u.timestamp_ms()));
    let back = format!("pkg.Ulid.parse(pkg.Ulid.encode({js}).toLowerCase())");
    cases.push(support::run("next", back, move || Ulid::parse(&u.encode().to_lowercase())));
    let bytes = format!("pkg.Ulid.fromBytes(pkg.Ulid.toBytes({js}))");
    cases.push(support::run("next", bytes, move || Ulid::from_bytes(&u.to_bytes())));
}

#[test]
fn ulid_matches_rust() {
    let cases = support::cases(|cases| {
        for text in [README, LAYOUT, MAX, "80000000000000000000000000", "01arz3ndektsv4rrffq69g5fav"] {
            cases.push(parse_case(text.to_string()));
        }
        // Values at and above 2^53, the top bit of `lo`, all ones.
        let boundaries = boundaries();
        for (k, v) in boundaries.iter().enumerate() {
            let u = of_u128(*v);
            if k % 9 == 0 || *v >= u128::MAX - 1 || (*v & (1 << 63) != 0 && k % 7 == 0) {
                view_cases(cases, u);
            }
            if k % 5 == 0 {
                cases.push(case!(ulid::next(Some(u), u.timestamp_ms(), [9u8; 10].as_slice())));
            }
        }
        for v in randoms().into_iter().step_by(250) {
            view_cases(cases, of_u128(v));
        }
        // Errors with their positions, non-ASCII and lowercase among them.
        for text in mutations().into_iter().step_by(457) {
            cases.push(parse_case(text));
        }
        for text in lengths().into_iter().step_by(5) {
            cases.push(parse_case(text));
        }
        cases.push(parse_case("01ARZ3NDEKTSV4RRFFQ69G5F\u{e9}".to_string()));
        cases.push(parse_case("01ARZ3NDEKTSV4RRFFQ69G5\u{212a}".to_string()));
        cases.push(parse_case("01ARZ3NDEKTSV4RRFFQ69G\u{1d7ce}".to_string()));
        cases.push(parse_case("8000000000000000000000000U".to_string()));
        for len in [0, 1, 15, 16, 17, 20] {
            cases.push(from_bytes_case(vec![0xFF; len]));
        }
        for len in [0, 9, 10, 11, 12] {
            for time in timestamps() {
                cases.push(new_case(time, vec![0xA5; len]));
            }
        }
        // `next` along the chains, exhaustion and invalid inputs kept.
        for (k, ((previous, time, fresh), model)) in chains().into_iter().enumerate() {
            let interesting = model.is_err() || previous.is_some_and(|v| v as u64 >= u64::MAX - 2);
            if k % 60 == 0 || interesting && k % 16 == 0 {
                let previous = previous.map(of_u128);
                cases.push(case!(ulid::next(previous, time, fresh.as_slice())));
            }
        }
        let edge = of_u128((u128::from(MAX_TIME) << 80) | ((1 << 80) - 1));
        for time in [MAX_TIME - 1, MAX_TIME, MAX_TIME + 1] {
            cases.push(case!(ulid::next(Some(edge), time, [0u8; 10].as_slice())));
            cases.push(case!(ulid::next(None::<Ulid>, time, [0xFFu8; 10].as_slice())));
        }
        // Order across the words.
        let small: Vec<Ulid> = boundaries.iter().step_by(23).map(|&v| of_u128(v)).collect();
        for a in &small {
            for b in &small {
                cases.push(case!(ulid::compare(a, b)));
            }
        }
    });
    assert!((500..2500).contains(&cases.len()), "{} cases", cases.len());
    support::assert_equivalent("ulid", ulid::SOURCE, &cases);
}
