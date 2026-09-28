// An IBAN in the electronic format of ISO 13616-1: two capital letters for
// the country, two check digits, then 11 to 30 capital letters and digits
// (15 to 34 in all). It is valid when, with the first four characters moved
// to the end and each letter read as 10 to 35, the number is 1 mod 97
// (ISO 7064 MOD 97-10). The country-by-country lengths are not checked.
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
    b >= 48u8 && b <= 57u8
}

fn is_upper(b: u8) -> bool {
    b >= 65u8 && b <= 90u8
}

fn all_upper(b: &[u8], i: usize, end: usize) -> bool {
    i == end || (is_upper(b[i]) && all_upper(b, i + 1usize, end))
}

fn all_digit(b: &[u8], i: usize, end: usize) -> bool {
    i == end || (is_digit(b[i]) && all_digit(b, i + 1usize, end))
}

fn all_alnum(b: &[u8], i: usize) -> bool {
    i == b.len() || ((is_upper(b[i]) || is_digit(b[i])) && all_alnum(b, i + 1usize))
}

/// One character into the running remainder: a digit is one decimal digit,
/// a letter two (A = 10 … Z = 35).
fn push(acc: u32, c: u8) -> u32 {
    if is_digit(c) {
        (acc * 10u32 + u32::from(c - 48u8)) % 97u32
    } else {
        (acc * 100u32 + u32::from(c - 55u8)) % 97u32
    }
}

/// The remainder of the rearranged string: from `i` to the end, then the first four.
fn rem(b: &[u8], i: usize, acc: u32) -> u32 {
    if i == b.len() {
        rem_head(b, 0usize, acc)
    } else {
        rem(b, i + 1usize, push(acc, b[i]))
    }
}

fn rem_head(b: &[u8], i: usize, acc: u32) -> u32 {
    if i == 4usize {
        acc
    } else {
        rem_head(b, i + 1usize, push(acc, b[i]))
    }
}

impl Iban {
    pub fn parse(raw: String) -> Result<Iban, IbanError> {
        let b = raw.as_bytes();
        if b.len() < 15usize || b.len() > 34usize {
            return Err(IbanError::Length);
        }
        if !all_upper(b, 0usize, 2usize) {
            return Err(IbanError::Country);
        }
        if !all_digit(b, 2usize, 4usize) {
            return Err(IbanError::CheckDigits);
        }
        if !all_alnum(b, 4usize) {
            return Err(IbanError::Bban);
        }
        if rem(b, 4usize, 0u32) != 1u32 {
            return Err(IbanError::Checksum);
        }
        Ok(Iban(raw))
    }
}
