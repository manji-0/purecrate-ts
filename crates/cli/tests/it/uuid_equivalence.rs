//! `uuid::Uuid` (design/01 §6): `Uuid::parse_str` and `try_parse` accept in
//! TS exactly what the `uuid` crate accepts, in each of its four forms and
//! any case, and give the same value; `==`, `<`, and `nil` agree; values in
//! structs, enums, and `?` pass through. Inputs are UUIDs the crate prints,
//! and each one-character change, cut, extension, case change, and wrapping
//! of them, including non-ASCII characters whose UTF-8 length reaches a
//! form's length when their UTF-16 length does not.

use crate::support;

use uuid::Uuid;

purecrate_canon::fixture!(mod uuids = "fixtures/uuids.rs");

/// xorshift64*, so the corpus is the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn ids(rng: &mut Rng, n: usize) -> Vec<Uuid> {
    let mut out = vec![
        Uuid::nil(),
        Uuid::max(),
        Uuid::from_u128(1),
        Uuid::from_u128(u128::MAX - 1),
        Uuid::from_u128(0x0f0f_0f0f_0f0f_0f0f_0f0f_0f0f_0f0f_0f0f),
    ];
    for _ in 0..n {
        out.push(Uuid::from_u64_pair(rng.next(), rng.next()));
    }
    out
}

fn texts(rng: &mut Rng) -> Vec<String> {
    let mut out: Vec<String> = [
        "",
        "-",
        "urn:uuid:",
        "{}",
        "00000000-0000-0000-0000-00000000000",
        "00000000-0000-0000-0000-0000000000000",
        "0000000000000000000000000000000",
        "000000000000000000000000000000000",
        "{00000000000000000000000000000000}",
        "urn:uuid:00000000000000000000000000000000",
        "URN:UUID:00000000-0000-0000-0000-000000000000",
        "Urn:uuid:00000000-0000-0000-0000-000000000000",
        "{00000000-0000-0000-0000-000000000000]",
        "(00000000-0000-0000-0000-000000000000)",
        "000000000-000-0000-0000-000000000000",
        "00000000-0000-0000-0000-0000-00000000",
        "0000000000000000000000000000000000-00",
        "00000000-0000-0000-0000-000000000000\n",
        " 00000000-0000-0000-0000-000000000000",
        "é0000000-0000-0000-0000-000000000000",
        "é000000-0000-0000-0000-000000000000",
        "é000000000000000000000000000000",
        "𝄞0000000000000000000000000000",
        "０0000000-0000-0000-0000-000000000000",
        "0000000g-0000-0000-0000-000000000000",
        "+0000000-0000-0000-0000-000000000000",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for id in ids(rng, 60) {
        let forms =
            [id.hyphenated().to_string(), id.simple().to_string(), id.braced().to_string(), id.urn().to_string()];
        for form in forms {
            out.push(form.to_uppercase());
            let mixed: String =
                form.chars().map(|c| if rng.below(2) == 0 { c.to_ascii_uppercase() } else { c }).collect();
            out.push(mixed);
            let b = form.as_bytes();
            let i = rng.below(b.len());
            for with in ["g", "-", "é", "{", "0", ""] {
                let mut edited = form.clone();
                edited.replace_range(i..=i, with);
                out.push(edited);
            }
            out.push(form[1..].to_string());
            out.push(format!("{form}0"));
            out.push(format!("{{{form}}}"));
            out.push(format!("urn:uuid:{form}"));
            out.push(form);
        }
    }
    out
}

#[test]
fn uuids_parse_and_compare_as_in_rust() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let texts = texts(&mut rng);
    let ids = ids(&mut rng, 40);
    let cases = support::cases(|cases| {
        for s in &texts {
            cases.push(case!(uuids::parse(s)));
            cases.push(case!(uuids::try_parse(s.clone())));
            cases.push(case!(uuids::read_segment(s.clone())));
        }
        for a in &ids {
            cases.push(case!(uuids::is_nil(*a)));
            for b in &ids {
                cases.push(case!(uuids::compare(*a, *b)));
            }
        }
        for (i, s) in texts.iter().enumerate().step_by(7) {
            let d = uuids::Disk { id: ids[i % ids.len()], parent: None };
            cases.push(case!(uuids::reparent(d, s)));
        }
    });
    // The forms the crate prints all parse, so the corpus reaches `Ok`.
    let accepted = texts.iter().filter(|s| Uuid::parse_str(s).is_ok()).count();
    assert!(accepted > 60 * 8, "only {accepted} accepted inputs");
    support::assert_equivalent("uuids", uuids::SOURCE, &cases);
}
