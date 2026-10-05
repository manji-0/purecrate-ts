//! `examples/punycode`, RFC 3492 Punycode and a simplified IDNA layer:
//!
//! - RFC 3492 §7.1's samples (A) to (S) encode to the published text (digits
//!   compared without case, as (I)'s annotation upper-cases one) and decode
//!   back, digits upper-cased too; the IDNA examples convert both ways;
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against: `String`, `Vec::insert`, `format!`, `join`) agree
//!   on those, on every string of one to three code points from a set of
//!   edge code points, on every label of up to three base-36 digits, and on
//!   names around each length limit;
//! - the generated package agrees with Rust on a sample of each.

use crate::support;

purecrate_canon::fixture!(mod punycode = "../../../examples/punycode/src/lib.rs");

/// RFC 3492 without the subset's constraints. Not converted; the reference
/// only. Error types carry the constrained side's names, so `Debug`
/// compares them.
mod idiomatic {
    const BASE: u32 = 36;
    const TMIN: u32 = 1;
    const TMAX: u32 = 26;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PunycodeError {
        NonBasic,
        BadDigit,
        Overflow,
        InvalidCodePoint,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DomainError {
        EmptyLabel,
        LabelTooLong,
        DomainTooLong,
        Punycode(PunycodeError),
        NotRoundTrip,
        AsciiOnly,
    }

    fn adapt(delta: u32, points: u32, first: bool) -> u32 {
        let mut delta = if first { delta / 700 } else { delta / 2 };
        delta += delta / points;
        let mut k = 0;
        while delta > ((BASE - TMIN) * TMAX) / 2 {
            delta /= BASE - TMIN;
            k += BASE;
        }
        k + (BASE - TMIN + 1) * delta / (delta + 38)
    }

    fn threshold(k: u32, bias: u32) -> u32 {
        k.saturating_sub(bias).clamp(TMIN, TMAX)
    }

    /// `a`..=`z` are 0..=25, `0`..=`9` are 26..=35 (§5).
    fn digit(d: u32) -> char {
        let b = u8::try_from(d).expect("a digit");
        char::from(if d < 26 { b'a' + b } else { b'0' + b - 26 })
    }

    fn value(c: char) -> Option<u32> {
        match c {
            'a'..='z' | 'A'..='Z' => Some(u32::from(c.to_ascii_lowercase()) - u32::from('a')),
            '0'..='9' => Some(u32::from(c) - u32::from('0') + 26),
            _ => None,
        }
    }

    pub fn encode(input: &str) -> Result<String, PunycodeError> {
        use PunycodeError::Overflow;
        let cps: Vec<u32> = input.chars().map(u32::from).collect();
        let mut out: String = input.chars().filter(char::is_ascii).collect();
        let basic = u32::try_from(out.len()).map_err(|_| Overflow)?;
        if basic > 0 {
            out.push('-');
        }
        let (mut n, mut delta, mut bias, mut h) = (128u32, 0u32, 72, basic);
        while (h as usize) < cps.len() {
            let m = *cps.iter().filter(|&&c| c >= n).min().expect("a code point left");
            delta = delta.checked_add((m - n).checked_mul(h + 1).ok_or(Overflow)?).ok_or(Overflow)?;
            n = m;
            for &c in &cps {
                if c < n {
                    delta = delta.checked_add(1).ok_or(Overflow)?;
                }
                if c == n {
                    let mut q = delta;
                    let mut k = BASE;
                    loop {
                        let t = threshold(k, bias);
                        if q < t {
                            break;
                        }
                        out.push(digit(t + (q - t) % (BASE - t)));
                        q = (q - t) / (BASE - t);
                        k += BASE;
                    }
                    out.push(digit(q));
                    bias = adapt(delta, h + 1, h == basic);
                    delta = 0;
                    h += 1;
                }
            }
            delta = delta.checked_add(1).ok_or(Overflow)?;
            n = n.checked_add(1).ok_or(Overflow)?;
        }
        Ok(out)
    }

    pub fn decode(input: &str) -> Result<String, PunycodeError> {
        use PunycodeError::*;
        if !input.is_ascii() {
            return Err(NonBasic);
        }
        let (basic, digits) = match input.rfind('-') {
            Some(at) if at > 0 => (&input[..at], &input[at + 1..]),
            _ => ("", input),
        };
        let mut out: Vec<char> = basic.chars().collect();
        let (mut n, mut i, mut bias) = (128u32, 0u32, 72);
        let mut digits = digits.chars().peekable();
        while digits.peek().is_some() {
            let old_i = i;
            let mut w = 1u32;
            let mut k = BASE;
            loop {
                let d = digits.next().and_then(value).ok_or(BadDigit)?;
                i = i.checked_add(d.checked_mul(w).ok_or(Overflow)?).ok_or(Overflow)?;
                let t = threshold(k, bias);
                if d < t {
                    break;
                }
                w = w.checked_mul(BASE - t).ok_or(Overflow)?;
                k += BASE;
            }
            let len = u32::try_from(out.len() + 1).map_err(|_| Overflow)?;
            bias = adapt(i - old_i, len, old_i == 0);
            n = n.checked_add(i / len).ok_or(Overflow)?;
            i %= len;
            out.insert(i as usize, char::from_u32(n).ok_or(InvalidCodePoint)?);
            i += 1;
        }
        Ok(out.into_iter().collect())
    }

    fn lower(label: &str) -> String {
        label.to_ascii_lowercase()
    }

    fn u_label_of(label: &str) -> Result<String, DomainError> {
        let payload = &label[4..];
        let u = decode(payload).map_err(DomainError::Punycode)?;
        if u.is_ascii() {
            return Err(DomainError::AsciiOnly);
        }
        if encode(&u).map_err(DomainError::Punycode)? != payload {
            return Err(DomainError::NotRoundTrip);
        }
        Ok(u)
    }

    fn a_label(label: &str) -> Result<String, DomainError> {
        if !label.is_ascii() {
            return Ok(format!("xn--{}", encode(label).map_err(DomainError::Punycode)?));
        }
        if label.starts_with("xn--") {
            u_label_of(label)?;
        }
        Ok(label.to_string())
    }

    /// The labels lower-cased, the root dot, and their ASCII forms checked
    /// for length.
    fn labels(domain: &str) -> Result<(Vec<String>, bool, Vec<String>), DomainError> {
        let (body, rooted) = match domain.strip_suffix('.') {
            Some(rest) => (rest, true),
            None => (domain, false),
        };
        let labels: Vec<String> = body.split('.').map(lower).collect();
        if labels.iter().any(String::is_empty) {
            return Err(DomainError::EmptyLabel);
        }
        let ascii = labels.iter().map(|l| a_label(l)).collect::<Result<Vec<_>, _>>()?;
        if ascii.iter().any(|l| l.len() > 63) {
            return Err(DomainError::LabelTooLong);
        }
        if ascii.join(".").len() > 253 {
            return Err(DomainError::DomainTooLong);
        }
        Ok((labels, rooted, ascii))
    }

    fn joined(labels: Vec<String>, rooted: bool) -> String {
        labels.join(".") + if rooted { "." } else { "" }
    }

    pub fn to_ascii(domain: &str) -> Result<String, DomainError> {
        labels(domain).map(|(_, rooted, ascii)| joined(ascii, rooted))
    }

    pub fn to_unicode(domain: &str) -> Result<String, DomainError> {
        let (labels, rooted, _) = labels(domain)?;
        let unicode = labels
            .into_iter()
            .map(|l| if l.starts_with("xn--") { u_label_of(&l) } else { Ok(l) })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(joined(unicode, rooted))
    }
}

fn text(s: String) -> String {
    s
}

/// RFC 3492 §7.1, (A) to (S): the code points and the published Punycode.
fn samples() -> Vec<(&'static str, Vec<u32>, &'static str)> {
    vec![
        (
            "A",
            vec![
                0x644, 0x64A, 0x647, 0x645, 0x627, 0x628, 0x62A, 0x643, 0x644, 0x645, 0x648, 0x634, 0x639, 0x631,
                0x628, 0x64A, 0x61F,
            ],
            "egbpdaj6bu4bxfgehfvwxn",
        ),
        ("B", vec![0x4ED6, 0x4EEC, 0x4E3A, 0x4EC0, 0x4E48, 0x4E0D, 0x8BF4, 0x4E2D, 0x6587], "ihqwcrb4cv8a8dqg056pqjye"),
        (
            "C",
            vec![0x4ED6, 0x5011, 0x7232, 0x4EC0, 0x9EBD, 0x4E0D, 0x8AAA, 0x4E2D, 0x6587],
            "ihqwctvzc91f659drss3x8bo0yb",
        ),
        (
            "D",
            vec![
                0x50, 0x72, 0x6F, 0x10D, 0x70, 0x72, 0x6F, 0x73, 0x74, 0x11B, 0x6E, 0x65, 0x6D, 0x6C, 0x75, 0x76, 0xED,
                0x10D, 0x65, 0x73, 0x6B, 0x79,
            ],
            "Proprostnemluvesky-uyb24dma41a",
        ),
        (
            "E",
            vec![
                0x5DC, 0x5DE, 0x5D4, 0x5D4, 0x5DD, 0x5E4, 0x5E9, 0x5D5, 0x5D8, 0x5DC, 0x5D0, 0x5DE, 0x5D3, 0x5D1,
                0x5E8, 0x5D9, 0x5DD, 0x5E2, 0x5D1, 0x5E8, 0x5D9, 0x5EA,
            ],
            "4dbcagdahymbxekheh6e0a7fei0b",
        ),
        (
            "F",
            vec![
                0x92F, 0x939, 0x932, 0x94B, 0x917, 0x939, 0x93F, 0x928, 0x94D, 0x926, 0x940, 0x915, 0x94D, 0x92F,
                0x94B, 0x902, 0x928, 0x939, 0x940, 0x902, 0x92C, 0x94B, 0x932, 0x938, 0x915, 0x924, 0x947, 0x939,
                0x948, 0x902,
            ],
            "i1baa7eci9glrd9b2ae1bj0hfcgg6iyaf8o0a1dig0cd",
        ),
        (
            "G",
            vec![
                0x306A, 0x305C, 0x307F, 0x3093, 0x306A, 0x65E5, 0x672C, 0x8A9E, 0x3092, 0x8A71, 0x3057, 0x3066, 0x304F,
                0x308C, 0x306A, 0x3044, 0x306E, 0x304B,
            ],
            "n8jok5ay5dzabd5bym9f0cm5685rrjetr6pdxa",
        ),
        (
            "H",
            vec![
                0xC138, 0xACC4, 0xC758, 0xBAA8, 0xB4E0, 0xC0AC, 0xB78C, 0xB4E4, 0xC774, 0xD55C, 0xAD6D, 0xC5B4, 0xB97C,
                0xC774, 0xD574, 0xD55C, 0xB2E4, 0xBA74, 0xC5BC, 0xB9C8, 0xB098, 0xC88B, 0xC744, 0xAE4C,
            ],
            "989aomsvi5e83db1d2a355cv1e0vak1dwrv93d5xbh15a0dt30a5jpsd879ccm6fea98c",
        ),
        (
            "I",
            vec![
                0x43F, 0x43E, 0x447, 0x435, 0x43C, 0x443, 0x436, 0x435, 0x43E, 0x43D, 0x438, 0x43D, 0x435, 0x433,
                0x43E, 0x432, 0x43E, 0x440, 0x44F, 0x442, 0x43F, 0x43E, 0x440, 0x443, 0x441, 0x441, 0x43A, 0x438,
            ],
            "b1abfaaepdrnnbgefbaDotcwatmq2g4l",
        ),
        (
            "J",
            vec![
                0x50, 0x6F, 0x72, 0x71, 0x75, 0xE9, 0x6E, 0x6F, 0x70, 0x75, 0x65, 0x64, 0x65, 0x6E, 0x73, 0x69, 0x6D,
                0x70, 0x6C, 0x65, 0x6D, 0x65, 0x6E, 0x74, 0x65, 0x68, 0x61, 0x62, 0x6C, 0x61, 0x72, 0x65, 0x6E, 0x45,
                0x73, 0x70, 0x61, 0xF1, 0x6F, 0x6C,
            ],
            "PorqunopuedensimplementehablarenEspaol-fmd56a",
        ),
        (
            "K",
            vec![
                0x54, 0x1EA1, 0x69, 0x73, 0x61, 0x6F, 0x68, 0x1ECD, 0x6B, 0x68, 0xF4, 0x6E, 0x67, 0x74, 0x68, 0x1EC3,
                0x63, 0x68, 0x1EC9, 0x6E, 0xF3, 0x69, 0x74, 0x69, 0x1EBF, 0x6E, 0x67, 0x56, 0x69, 0x1EC7, 0x74,
            ],
            "TisaohkhngthchnitingVit-kjcr8268qyxafd2f1b9g",
        ),
        ("L", vec![0x33, 0x5E74, 0x42, 0x7D44, 0x91D1, 0x516B, 0x5148, 0x751F], "3B-ww4c5e180e575a65lsy2b"),
        (
            "M",
            vec![
                0x5B89, 0x5BA4, 0x5948, 0x7F8E, 0x6075, 0x2D, 0x77, 0x69, 0x74, 0x68, 0x2D, 0x53, 0x55, 0x50, 0x45,
                0x52, 0x2D, 0x4D, 0x4F, 0x4E, 0x4B, 0x45, 0x59, 0x53,
            ],
            "-with-SUPER-MONKEYS-pc58ag80a8qai00g7n9n",
        ),
        (
            "N",
            vec![
                0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x2D, 0x41, 0x6E, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x2D, 0x57, 0x61, 0x79,
                0x2D, 0x305D, 0x308C, 0x305E, 0x308C, 0x306E, 0x5834, 0x6240,
            ],
            "Hello-Another-Way--fc4qua05auwb3674vfr0b",
        ),
        ("O", vec![0x3072, 0x3068, 0x3064, 0x5C4B, 0x6839, 0x306E, 0x4E0B, 0x32], "2-u9tlzr9756bt3uc0v"),
        (
            "P",
            vec![0x4D, 0x61, 0x6A, 0x69, 0x3067, 0x4B, 0x6F, 0x69, 0x3059, 0x308B, 0x35, 0x79D2, 0x524D],
            "MajiKoi5-783gue6qz075azm5e",
        ),
        ("Q", vec![0x30D1, 0x30D5, 0x30A3, 0x30FC, 0x64, 0x65, 0x30EB, 0x30F3, 0x30D0], "de-jg4avhby1noc0d"),
        ("R", vec![0x305D, 0x306E, 0x30B9, 0x30D4, 0x30FC, 0x30C9, 0x3067], "d9juau41awczczp"),
        ("S", vec![0x2D, 0x3E, 0x20, 0x24, 0x31, 0x2E, 0x30, 0x30, 0x20, 0x3C, 0x2D], "-> $1.00 <--"),
    ]
}

fn from_code_points(cps: &[u32]) -> String {
    cps.iter().map(|c| char::from_u32(*c).expect("a scalar value")).collect()
}

/// The digits after the last delimiter in upper case.
fn upper_digits(p: &str) -> String {
    match p.rfind('-') {
        Some(at) => format!("{}{}", &p[..=at], p[at + 1..].to_ascii_uppercase()),
        None => p.to_ascii_uppercase(),
    }
}

const IDNA: [(&str, &str); 3] = [
    ("bücher.example", "xn--bcher-kva.example"),
    ("例え.テスト", "xn--r8jz45g.xn--zckzah"),
    ("bücher.example.", "xn--bcher-kva.example."),
];

#[test]
fn the_published_samples_come_out_as_printed() {
    for (name, cps, published) in samples() {
        let input = from_code_points(&cps);
        let encoded = text(punycode::encode(&input).expect(name));
        assert!(encoded.eq_ignore_ascii_case(published), "({name}) encodes to {encoded}");
        assert_eq!(text(punycode::decode(published).expect(name)), input, "({name}) decodes");
        assert_eq!(text(punycode::decode(&upper_digits(published)).expect(name)), input, "({name}) upper-cased");
    }
    for (unicode, ascii) in IDNA {
        assert_eq!(punycode::to_ascii(unicode).map(text), Ok(ascii.to_string()));
        assert_eq!(punycode::to_unicode(ascii).map(text), Ok(unicode.to_string()));
    }
    assert_eq!(punycode::to_unicode("XN--BCHER-KVA.Example").map(text), Ok("bücher.example".to_string()));
    use punycode::{DomainError as D, PunycodeError as P};
    assert_eq!(punycode::decode("abcé"), Err(P::NonBasic));
    assert_eq!(punycode::decode("ab-c!d"), Err(P::BadDigit));
    assert_eq!(punycode::decode("99999999999"), Err(P::Overflow));
    assert_eq!(punycode::decode("-xrpdc"), Err(P::BadDigit), "a leading '-' is no delimiter (§6.2)");
    assert_eq!(punycode::to_ascii("a..b"), Err(D::EmptyLabel));
    assert_eq!(punycode::to_ascii(&"a".repeat(64)), Err(D::LabelTooLong));
    assert_eq!(punycode::to_ascii("xn--abc-"), Err(D::AsciiOnly));
    let at_limit = [&"a".repeat(63)[..], &"a".repeat(63), &"a".repeat(63), &"a".repeat(61)].join(".");
    assert_eq!(at_limit.len(), 253);
    assert!(punycode::to_ascii(&at_limit).is_ok());
    assert_eq!(punycode::to_ascii(&format!("{at_limit}a")), Err(D::DomainTooLong));
}

/// Code points around the edges the algorithm meets: basic and not, the
/// initial n, each UTF-8 and UTF-16 length, the surrogate gap.
const EDGES: [char; 12] =
    ['a', 'Z', '-', '0', '\u{7f}', '\u{80}', 'ü', '\u{7ff}', '日', '\u{d7ff}', '\u{e000}', '\u{10ffff}'];

fn strings() -> Vec<String> {
    let mut out = vec![String::new()];
    for a in EDGES {
        out.push(a.to_string());
        for b in EDGES {
            out.push(format!("{a}{b}"));
            for c in EDGES {
                out.push(format!("{a}{b}{c}"));
            }
        }
    }
    out
}

/// Every label of up to three base-36 digits and '-', with and without a
/// basic prefix.
fn payloads() -> Vec<String> {
    let alphabet: Vec<char> = "a9z-".chars().chain("bcdefghk012".chars()).collect();
    let mut out = vec![String::new()];
    for a in &alphabet {
        for b in &alphabet {
            for c in &alphabet {
                out.push(format!("{a}{b}{c}"));
                out.push(format!("x-{a}{b}{c}"));
            }
        }
    }
    out.extend(
        ["zzzzzzzzzzzzzzzzz", "99999999999", "a-99999999999", "ihqwcrb4cv8a8dqg056pqjye", "bb0c", "bb0c9"]
            .map(String::from),
    );
    out
}

fn names() -> Vec<String> {
    let mut out: Vec<String> = IDNA.iter().flat_map(|(u, a)| [u.to_string(), a.to_string()]).collect();
    out.extend(
        [
            "",
            ".",
            "a..b",
            "a.b..",
            "Bücher.EXAMPLE",
            "xn--abc-",
            "xn--bcher-kvaa",
            "xn--BCHER-KVA",
            "xn--zz",
            "xn---xrpdc.x",
        ]
        .map(String::from),
    );
    for n in [61, 62, 63, 64] {
        out.push("a".repeat(n));
        out.push(format!("{}ü", "a".repeat(n - 8)));
    }
    let label = "a".repeat(63);
    for last in [59, 60, 61, 62] {
        out.push([&label[..], &label, &label, &"b".repeat(last)].join("."));
        out.push([&label[..], &label, &label, &"b".repeat(last)].join(".") + ".");
    }
    out
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let dbg = |x: &dyn std::fmt::Debug| format!("{x:?}");
    for s in strings() {
        let c = punycode::encode(&s).map(text);
        assert_eq!(dbg(&c), dbg(&idiomatic::encode(&s)), "encode {s:?}");
        if let Ok(p) = c {
            assert_eq!(punycode::decode(&p).map(text), Ok(s.clone()), "round trip {s:?}");
        }
    }
    for p in payloads() {
        assert_eq!(dbg(&punycode::decode(&p).map(text)), dbg(&idiomatic::decode(&p)), "decode {p:?}");
        let label = format!("xn--{p}");
        assert_eq!(dbg(&punycode::to_unicode(&label).map(text)), dbg(&idiomatic::to_unicode(&label)), "{label}");
    }
    for name in names() {
        assert_eq!(dbg(&punycode::to_ascii(&name).map(text)), dbg(&idiomatic::to_ascii(&name)), "to_ascii {name:?}");
        assert_eq!(dbg(&punycode::to_unicode(&name).map(text)), dbg(&idiomatic::to_unicode(&name)), "{name:?}");
    }
}

#[test]
fn punycode_matches_rust() {
    support::equivalence("punycode", punycode::SOURCE, |cases| {
        for (_, cps, published) in samples() {
            let input = from_code_points(&cps);
            cases.push(case!(punycode::encode(input.as_str())));
            cases.push(case!(punycode::decode(published)));
            let upper = upper_digits(published);
            cases.push(case!(punycode::decode(upper.as_str())));
        }
        for s in strings().iter().step_by(7) {
            cases.push(case!(punycode::encode(s.as_str())));
        }
        for p in payloads().iter().step_by(5) {
            cases.push(case!(punycode::decode(p.as_str())));
        }
        for name in names() {
            cases.push(case!(punycode::to_ascii(name.as_str())));
            cases.push(case!(punycode::to_unicode(name.as_str())));
        }
    });
}
