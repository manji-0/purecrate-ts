//! `examples/iban` agrees with the generated package, and with the same
//! rules written in idiomatic Rust (`idiomatic`, the line count design/07
//! §2 compares against), on valid IBANs, each one-character change of
//! them, and malformed input.

use crate::support;

purecrate_canon::fixture!(mod iban = "../../../examples/iban/src/lib.rs", "fixtures/iban_driver.rs");

/// ISO 13616-1 electronic format with MOD 97-10, as one would write it
/// without the subset's constraints. Not converted; the reference only.
mod idiomatic {
    #[derive(Debug, PartialEq, Clone, Copy)]
    pub enum IbanError {
        Length,
        Country,
        CheckDigits,
        Bban,
        Checksum,
    }

    pub fn check(s: &str) -> Result<(), IbanError> {
        if !(15..=34).contains(&s.chars().count()) {
            return Err(IbanError::Length);
        }
        let b = s.as_bytes();
        if !b[..2].iter().all(u8::is_ascii_uppercase) {
            return Err(IbanError::Country);
        }
        if !b[2..4].iter().all(u8::is_ascii_digit) || matches!(&s[2..4], "00" | "01" | "99") {
            return Err(IbanError::CheckDigits);
        }
        if !b[4..].iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
            return Err(IbanError::Bban);
        }
        let rem = b[4..].iter().chain(&b[..4]).fold(0u32, |acc, &c| match c {
            b'0'..=b'9' => (acc * 10 + u32::from(c - b'0')) % 97,
            _ => (acc * 100 + u32::from(c - b'A') + 10) % 97,
        });
        if rem == 1 {
            Ok(())
        } else {
            Err(IbanError::Checksum)
        }
    }
}

/// Published examples (ECBS / SWIFT IBAN registry).
const VALID: [&str; 6] = [
    "GB82WEST12345698765432",
    "DE89370400440532013000",
    "FR1420041010050500013M02606",
    "NO9386011117947",
    "MT84MALT011000012345MTLCAST001S",
    "LC55HEMM000100010012001200023015",
];

fn inputs() -> Vec<String> {
    let mut out: Vec<String> = VALID.iter().map(|s| s.to_string()).collect();
    for v in VALID {
        // Every single-character change to a neighbouring digit or letter.
        for (i, c) in v.char_indices() {
            let next = match c {
                '9' => '0',
                'Z' => 'A',
                c => char::from(c as u8 + 1),
            };
            out.push(format!("{}{next}{}", &v[..i], &v[i + 1..]));
        }
        out.push(v.to_lowercase());
        out.push(format!("{v}0"));
        out.push(v[..v.len() - 1].to_string());
        out.push(format!("{} {}", &v[..4], &v[4..]));
    }
    out.extend(
        // A MOD 97 remainder of 1 with check digits never issued.
        [
            "DE01100000000000000010",
            "DE00100000000000000028",
            "DE99100000000000000089",
            "",
            "GB82",
            "GB82WEST1234569876543",
            "1B82WEST12345698765432",
            "GBX2WEST12345698765432",
            "GB82WEST1234569876543é",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    out.push(format!("GB82{}", "A".repeat(31)));
    out.extend(non_ascii().into_iter().map(|(s, _)| s));
    out
}

/// Input whose length in characters and in UTF-8 bytes fall on different
/// sides of a bound, and non-ASCII in each field, with the error ISO
/// 13616's character counts give.
fn non_ascii() -> Vec<(String, idiomatic::IbanError)> {
    use idiomatic::IbanError as I;
    vec![
        // 34 characters, 35 bytes: within the length, then not a BBAN character.
        (format!("GB82{}é", "A".repeat(29)), I::Bban),
        // 14 characters, 15 bytes: too short.
        (format!("NO93{}é", "1".repeat(9)), I::Length),
        // 15 characters, 16 bytes; 34 characters, 36 bytes (a 3-byte character).
        (format!("NO93{}é", "1".repeat(10)), I::Bban),
        (format!("GB82{}€", "A".repeat(29)), I::Bban),
        // 35 characters: too long, whatever they are.
        (format!("GB82{}é", "A".repeat(30)), I::Length),
        // 34 characters, 37 bytes: a 4-byte character in the country, an
        // accented one in the check digits.
        (format!("😀B82{}", "A".repeat(30)), I::Country),
        (format!("GBé2{}", "A".repeat(30)), I::CheckDigits),
    ]
}

#[test]
fn length_counts_characters() {
    for (s, want) in non_ascii() {
        assert_eq!(idiomatic::check(&s), Err(want), "{s:?}");
        assert!(same_verdict(&iban::Iban::parse(s.clone()), &Err(want)), "{s:?}: want {want:?}");
    }
}

fn same_verdict(a: &Result<iban::Iban, iban::IbanError>, b: &Result<(), idiomatic::IbanError>) -> bool {
    use iban::IbanError as E;
    use idiomatic::IbanError as I;
    matches!(
        (a, b),
        (Ok(_), Ok(()))
            | (Err(E::Length), Err(I::Length))
            | (Err(E::Country), Err(I::Country))
            | (Err(E::CheckDigits), Err(I::CheckDigits))
            | (Err(E::Bban), Err(I::Bban))
            | (Err(E::Checksum), Err(I::Checksum))
    )
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let inputs = inputs();
    for v in VALID {
        assert_eq!(idiomatic::check(v), Ok(()), "{v} is a published valid IBAN");
    }
    let differ: Vec<String> = inputs
        .iter()
        .filter(|s| !same_verdict(&iban::Iban::parse(s.to_string()), &idiomatic::check(s)))
        .map(|s| format!("{s:?}: idiomatic {:?}", idiomatic::check(s)))
        .collect();
    assert!(differ.is_empty(), "the constrained Rust differs:\n{}", differ.join("\n"));
}

#[test]
fn iban_matches_rust() {
    let cases =
        support::quietly(|| inputs().into_iter().map(|s| case!(iban::parse_iban(s.clone()))).collect::<Vec<_>>());
    support::assert_equivalent("iban", iban::SOURCE, &cases);
}
