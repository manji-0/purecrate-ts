// An IBAN in the electronic format of ISO 13616-1: two capital letters for
// the country, two check digits, then capital letters and digits. ISO
// 13616-1 caps the whole at 34 characters (a BBAN of at most 30); it sets no
// minimum. The minimum of 15 is the shortest national IBAN in the SWIFT IBAN
// Registry (Norway's), so 15 to 34 characters in all. Length counts
// characters, as both bounds are stated: non-ASCII input is not an IBAN, but
// it gets the error of the rule it breaks first, `Length` when there are
// fewer than 15 or more than 34 characters, otherwise the error for the
// position of its first non-ASCII character (`Country`, `CheckDigits`, or
// `Bban`).
//
// It is valid when, with the first four characters moved to the end and each
// letter read as 10 to 35, the number is 1 mod 97 (ISO 7064 MOD 97-10). The
// check digits are 02 to 98: 00, 01, and 99 leave the same remainder as 97,
// 98, and 02, and are never issued. The country-by-country lengths and BBAN
// formats of the registry, and whether the country is in it, are not
// checked: a well-formed IBAN with the right checksum for an unknown
// country, or of the wrong length for its country, is accepted.
//
// `Iban` is a closed type: the value comes only from `parse`.

pub struct Iban(String);

pub enum IbanError {
    Length,
    Country,
    CheckDigits,
    Bban,
    Checksum,
}

fn is_digit(b: u8) -> bool {
    matches!(b, b'0'..=b'9')
}

fn is_upper(b: u8) -> bool {
    matches!(b, b'A'..=b'Z')
}

/// One character into the running remainder: a digit is one decimal digit,
/// a letter two (A = 10 … Z = 35).
fn push(acc: u32, c: u8) -> u32 {
    match c {
        b'0'..=b'9' => (acc * 10 + u32::from(c - b'0')) % 97,
        _ => (acc * 100 + u32::from(c - b'A') + 10) % 97,
    }
}

impl Iban {
    pub fn parse(raw: String) -> Result<Iban, IbanError> {
        let n = raw.chars().count();
        if n < 15 || n > 34 {
            return Err(IbanError::Length);
        }
        // At least 15 characters, so at least 15 bytes. A byte below 0x80
        // is a whole character; one at or above it is part of a non-ASCII
        // one and fails the test of the position it stands in.
        let b = raw.as_bytes();
        if !is_upper(b[0]) || !is_upper(b[1]) {
            return Err(IbanError::Country);
        }
        if !is_digit(b[2]) || !is_digit(b[3]) {
            return Err(IbanError::CheckDigits);
        }
        let check = (b[2] - b'0') * 10 + (b[3] - b'0');
        if check < 2 || check > 98 {
            return Err(IbanError::CheckDigits);
        }
        // The first four characters count last.
        let mut acc = 0u32;
        for i in 4..b.len() {
            if !is_upper(b[i]) && !is_digit(b[i]) {
                return Err(IbanError::Bban);
            }
            acc = push(acc, b[i]);
        }
        for i in 0..4usize {
            acc = push(acc, b[i]);
        }
        if acc != 1 {
            return Err(IbanError::Checksum);
        }
        Ok(Iban(raw))
    }
}
