// Punycode and a simplified IDNA domain-name layer.
//
// Specifications:
// - RFC 3492 Punycode: §5 parameters, §6.1 bias adaptation, §6.2 decoding,
//   §6.3 encoding, §6.4 overflow handling. Digits decode case-insensitively;
//   the basic code points are those before the last delimiter.
// - RFC 5890/5891 (IDNA2008), simplified: a domain is split on '.', an ASCII
//   label is kept, a non-ASCII label becomes "xn--" + Punycode, and the
//   reverse turns an "xn--" label (prefix case-insensitive) back.
//
// Everything impure is outside: the caller reads the input, displays the
// result, and resolves the name.
//
// Policy choices, not the specifications':
// - Overflow (§6.4): every RFC quantity (n, i, delta, bias, w, and the
//   lengths h, b, and the output length) is a `u32`, and every addition and
//   multiplication that the RFC's sample code guards is checked, so the
//   limit is exactly the sample code's maxint = 2^32-1.
// - Decoding rejects a code point that is a surrogate or above U+10FFFF
//   (`InvalidCodePoint`). The braced check of §6.2 ("if n is a basic code
//   point then fail") is not written: n starts at 128 and never decreases.
// - Decoding rejects any non-ASCII input up front (`NonBasic`); an ASCII
//   character that is not a digit after the last delimiter, or an input
//   ending inside a variable-length integer, is `BadDigit`.
// - The encoder writes digits in lower case and has no mixed-case
//   annotation (§A); basic code points are copied as they are.
// - Domains: every ASCII letter of every label is lower-cased before
//   conversion, in both directions; nothing else is mapped or normalized.
// - An "xn--" label must decode, must re-encode to the same text, and must
//   decode to at least one non-ASCII code point (RFC 5891 §5.4: an A-label
//   is the ASCII form of a U-label). `to_ascii` checks this too, and keeps
//   the label as it is.
// - Lengths are counted on the ASCII form in both directions: each label
//   1..=63 octets, the whole name at most 253 octets without the trailing
//   root dot. A single trailing dot is kept; any other empty label
//   (including the names "" and ".") is rejected.
//
// Left out on purpose:
// - Unicode normalization, IDNA mapping tables, the code point validity
//   rules of RFC 5892, the bidi rule (RFC 5893), hyphen placement rules
//   (RFC 5891 §4.2.3.1), and the mixed-case annotation of RFC 3492 §A.

// ---------------------------------------------------------------------------
// Punycode (RFC 3492)
// ---------------------------------------------------------------------------

pub const BASE: u32 = 36;
pub const TMIN: u32 = 1;
pub const TMAX: u32 = 26;
pub const SKEW: u32 = 38;
pub const DAMP: u32 = 700;
pub const INITIAL_BIAS: u32 = 72;
pub const INITIAL_N: u32 = 128;
pub const DELIMITER: char = '-';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunycodeError {
    /// The input to `decode` contains a non-ASCII character.
    NonBasic,
    /// A character after the last delimiter is not a base-36 digit, or the
    /// input ends inside a variable-length integer.
    BadDigit,
    /// A value exceeds 2^32-1 (§6.4).
    Overflow,
    /// A decoded code point is a surrogate or above U+10FFFF.
    InvalidCodePoint,
}

/// §6.1.
fn adapt(delta: u32, num_points: u32, first_time: bool) -> u32 {
    let mut d = if first_time { delta / DAMP } else { delta / 2 };
    d += d / num_points;
    let mut k: u32 = 0;
    while d > ((BASE - TMIN) * TMAX) / 2 {
        d /= BASE - TMIN;
        k += BASE;
    }
    k + ((BASE - TMIN + 1) * d) / (d + SKEW)
}

/// The threshold t for position k (§6.2, §6.3).
fn threshold(k: u32, bias: u32) -> u32 {
    if k <= bias {
        TMIN
    } else if k >= bias + TMAX {
        TMAX
    } else {
        k - bias
    }
}

/// The value of a digit, case-insensitively: A-Z and a-z are 0..=25,
/// 0-9 are 26..=35.
fn digit_value(c: char) -> Option<u32> {
    match c.to_digit(36) {
        Some(d) => {
            if d < 10 {
                Some(d + 26)
            } else {
                Some(d - 10)
            }
        }
        None => None,
    }
}

/// The lower-case character of a digit value in 0..36.
fn digit_char(d: u32) -> char {
    let cp = if d < 26 { 97 + d } else { 22 + d };
    match char::from_u32(cp) {
        Some(c) => c,
        None => '?',
    }
}

fn is_basic(cp: u32) -> bool {
    cp < 0x80
}

/// `out` with `cp` inserted before index `at` (`at == len` appends).
fn insert_at(out: &Vec<u32>, at: u32, cp: u32) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    let mut j: u32 = 0;
    for c in out {
        if j == at {
            v.push(cp);
        }
        v.push(*c);
        j += 1;
    }
    if j == at {
        v.push(cp);
    }
    v
}

/// §6.3, on code points. The result is ASCII.
fn encode_code_points(input: &Vec<u32>) -> Result<String, PunycodeError> {
    let mut out = String::new();
    let mut len: u32 = 0;
    let mut b: u32 = 0;
    for cp in input {
        len += 1;
        if is_basic(*cp) {
            b += 1;
            match char::from_u32(*cp) {
                Some(c) => out.push(c),
                None => {}
            }
        }
    }
    if b > 0 {
        out.push(DELIMITER);
    }
    let mut n = INITIAL_N;
    let mut delta: u32 = 0;
    let mut bias = INITIAL_BIAS;
    let mut h = b;
    while h < len {
        // The smallest code point >= n in the input; one exists since h < len.
        let mut m: u32 = 0x10FFFF;
        for cp in input {
            if *cp >= n && *cp < m {
                m = *cp;
            }
        }
        let step = (m - n).checked_mul(h + 1).ok_or(PunycodeError::Overflow)?;
        delta = delta.checked_add(step).ok_or(PunycodeError::Overflow)?;
        n = m;
        for cp in input {
            if *cp < n {
                delta = delta.checked_add(1).ok_or(PunycodeError::Overflow)?;
            }
            if *cp == n {
                let mut q = delta;
                let mut k = BASE;
                while q >= threshold(k, bias) {
                    let t = threshold(k, bias);
                    out.push(digit_char(t + (q - t) % (BASE - t)));
                    q = (q - t) / (BASE - t);
                    k += BASE;
                }
                out.push(digit_char(q));
                bias = adapt(delta, h + 1, h == b);
                delta = 0;
                h += 1;
            }
        }
        delta = delta.checked_add(1).ok_or(PunycodeError::Overflow)?;
        n = n.checked_add(1).ok_or(PunycodeError::Overflow)?;
    }
    Ok(out)
}

/// §6.2, on characters already known to be ASCII.
fn decode_chars(input: &Vec<char>) -> Result<Vec<u32>, PunycodeError> {
    // The basic code points are those before the last delimiter.
    let mut last_delim: Option<usize> = None;
    for (j, c) in input.iter().enumerate() {
        if *c == DELIMITER {
            last_delim = Some(j);
        }
    }
    let mut out: Vec<u32> = Vec::new();
    let mut out_len: u32 = 0;
    let mut pos: usize = 0;
    match last_delim {
        Some(d) => {
            for j in 0..d {
                out.push(u32::from(input[j]));
                out_len += 1;
            }
            // "if more than zero code points were consumed then consume one
            // more (which will be the last delimiter)": a delimiter at 0 is
            // read as a digit, and fails.
            if d > 0 {
                pos = d + 1;
            }
        }
        None => {}
    }
    let mut n = INITIAL_N;
    let mut i: u32 = 0;
    let mut bias = INITIAL_BIAS;
    while pos < input.len() {
        let old_i = i;
        let mut w: u32 = 1;
        let mut k = BASE;
        // The RFC's `for (k = base;; k += base)`: `w` overflows (it grows by
        // at least base - tmax = 10 per digit) long before `k` would.
        while k > 0 {
            if pos >= input.len() {
                return Err(PunycodeError::BadDigit);
            }
            let digit = digit_value(input[pos]).ok_or(PunycodeError::BadDigit)?;
            pos += 1;
            let dw = digit.checked_mul(w).ok_or(PunycodeError::Overflow)?;
            i = i.checked_add(dw).ok_or(PunycodeError::Overflow)?;
            let t = threshold(k, bias);
            if digit < t {
                break;
            }
            w = w.checked_mul(BASE - t).ok_or(PunycodeError::Overflow)?;
            k += BASE;
        }
        bias = adapt(i - old_i, out_len + 1, old_i == 0);
        n = n.checked_add(i / (out_len + 1)).ok_or(PunycodeError::Overflow)?;
        i %= out_len + 1;
        if matches!(char::from_u32(n), None) {
            return Err(PunycodeError::InvalidCodePoint);
        }
        out = insert_at(&out, i, n);
        out_len += 1;
        i += 1;
    }
    Ok(out)
}

/// Punycode of a string of Unicode scalar values (RFC 3492 §6.3); ASCII.
pub fn encode(input: &str) -> Result<String, PunycodeError> {
    let cps: Vec<u32> = input.chars().map(|c| u32::from(c)).collect();
    encode_code_points(&cps)
}

/// The text a Punycode text stands for (RFC 3492 §6.2).
pub fn decode(input: &str) -> Result<String, PunycodeError> {
    if !input.bytes().all(|b| b < 0x80) {
        return Err(PunycodeError::NonBasic);
    }
    let cs: Vec<char> = input.chars().collect();
    let cps = decode_chars(&cs)?;
    let mut out = String::new();
    for cp in &cps {
        match char::from_u32(*cp) {
            Some(c) => out.push(c),
            None => return Err(PunycodeError::InvalidCodePoint),
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Domain names (RFC 5891, simplified)
// ---------------------------------------------------------------------------

pub const MAX_LABEL_LEN: usize = 63;
pub const MAX_DOMAIN_LEN: usize = 253;
pub const ACE_PREFIX: &str = "xn--";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainError {
    /// The name is empty, or has an empty label other than a single
    /// trailing root dot.
    EmptyLabel,
    /// A label is longer than 63 octets in its ASCII form.
    LabelTooLong,
    /// The name is longer than 253 octets in its ASCII form.
    DomainTooLong,
    /// An "xn--" label does not decode.
    Punycode(PunycodeError),
    /// An "xn--" label decodes, but does not re-encode to the same text.
    NotRoundTrip,
    /// An "xn--" label decodes to ASCII only.
    AsciiOnly,
}

fn is_ascii_label(label: &str) -> bool {
    label.bytes().all(|b| b < 0x80)
}

/// The labels of a name, ASCII letters lower-cased, and whether it ends in
/// the root dot.
fn split_labels(domain: &str) -> Result<(Vec<String>, bool), DomainError> {
    let (body, rooted) = match domain.strip_suffix(".") {
        Some(rest) => (rest, true),
        None => (domain, false),
    };
    if body.is_empty() {
        return Err(DomainError::EmptyLabel);
    }
    let mut labels: Vec<String> = Vec::new();
    for label in body.split('.') {
        if label.is_empty() {
            return Err(DomainError::EmptyLabel);
        }
        labels.push(label.chars().map(|c| c.to_ascii_lowercase()).collect());
    }
    Ok((labels, rooted))
}

/// The U-label an "xn--" label stands for, after the round-trip check.
fn checked_u_label(label: &str) -> Result<String, DomainError> {
    // "xn--" is ASCII, so byte 4 is a character boundary.
    let payload = &label[4..];
    let u = decode(payload).map_err(|e| DomainError::Punycode(e))?;
    if is_ascii_label(&u) {
        return Err(DomainError::AsciiOnly);
    }
    let again = encode(&u).map_err(|e| DomainError::Punycode(e))?;
    if again != payload {
        return Err(DomainError::NotRoundTrip);
    }
    Ok(u)
}

/// The A-label (ASCII form) of one label.
fn a_label(label: &str) -> Result<String, DomainError> {
    if is_ascii_label(label) {
        if label.starts_with(ACE_PREFIX) {
            checked_u_label(label)?;
        }
        return Ok(String::from(label));
    }
    let p = encode(label).map_err(|e| DomainError::Punycode(e))?;
    let mut out = String::from(ACE_PREFIX);
    out.push_str(&p);
    Ok(out)
}

/// The U-label (Unicode form) of one label.
fn u_label(label: &str) -> Result<String, DomainError> {
    if label.starts_with(ACE_PREFIX) {
        return checked_u_label(label);
    }
    Ok(String::from(label))
}

/// The labels joined with '.', with the root dot when `rooted`.
fn join_labels(labels: &Vec<String>, rooted: bool) -> String {
    let mut out = String::new();
    for (j, label) in labels.iter().enumerate() {
        if j > 0 {
            out.push('.');
        }
        out.push_str(label);
    }
    if rooted {
        out.push('.');
    }
    out
}

/// Checks the lengths of the ASCII form of a name (ASCII, so one octet per
/// character).
fn check_lengths(a_labels: &Vec<String>) -> Result<(), DomainError> {
    let mut total: usize = 0;
    for (j, label) in a_labels.iter().enumerate() {
        if label.len() > MAX_LABEL_LEN {
            return Err(DomainError::LabelTooLong);
        }
        if j > 0 {
            total += 1;
        }
        total += label.len();
    }
    if total > MAX_DOMAIN_LEN {
        return Err(DomainError::DomainTooLong);
    }
    Ok(())
}

/// The ASCII form of a domain name.
pub fn to_ascii(domain: &str) -> Result<String, DomainError> {
    let (labels, rooted) = split_labels(domain)?;
    let mut out: Vec<String> = Vec::new();
    for label in &labels {
        out.push(a_label(label)?);
    }
    check_lengths(&out)?;
    Ok(join_labels(&out, rooted))
}

/// The Unicode form of a domain name.
pub fn to_unicode(domain: &str) -> Result<String, DomainError> {
    let (labels, rooted) = split_labels(domain)?;
    let mut ascii: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for label in &labels {
        ascii.push(a_label(label)?);
        out.push(u_label(label)?);
    }
    check_lengths(&ascii)?;
    Ok(join_labels(&out, rooted))
}
