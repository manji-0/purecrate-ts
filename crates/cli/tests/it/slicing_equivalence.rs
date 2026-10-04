//! Slicing: `&s[a..b]` and its open forms at UTF-8 byte positions, on
//! `Vec`s, slices, and `as_bytes()`, with Rust's panics for positions past
//! the end, reversed ranges, and positions inside a character; and
//! `strip_prefix` / `strip_suffix`.

use crate::support;

purecrate_canon::fixture!(mod slicing = "fixtures/slicing.rs");

const SOURCE: &str = slicing::SOURCE;

// Every UTF-8 width, and a position inside each multi-byte character. The
// last string's characters are ones `Debug` escapes in the panic message: a
// combining mark, and Zs, Cc, Cf, Co, and Cn code points.
const STRINGS: [&str; 7] = [
    "",
    "abc",
    "héllo",
    "a😀b",
    "日本語",
    "x\u{7ff}\u{800}\u{ffff}\u{10000}",
    "e\u{301}\u{a0}\u{85}\u{200b}\u{e000}\u{378}\u{10ffff}",
];

#[test]
fn generated_slicing_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in STRINGS {
            let n = s.len();
            for a in 0..=n + 1 {
                cases.push(case!(slicing::from(String::from(s), a)));
                cases.push(case!(slicing::to(s, a)));
                cases.push(case!(slicing::digits_after(s, a)));
                for b in 0..=n + 1 {
                    cases.push(case!(slicing::mid(s, a, b)));
                    cases.push(case!(slicing::inner(s, a, b)));
                    cases.push(case!(slicing::bytes_of(s, a, b)));
                }
            }
            cases.push(case!(slicing::whole(s)));
        }
        let xs: &[u8] = &[1, 2, 3, 4];
        for a in 0..=5 {
            cases.push(case!(slicing::items_to(xs, a)));
            cases.push(case!(slicing::items_from(vec![1, 2, u32::MAX - 3, 1], a)));
            for b in 0..=5 {
                cases.push(case!(slicing::items(xs, a, b)));
            }
        }
        for s in ["", "pm_", "pm_1", "pm", "PM_1", "pm_é", "é_pm_"] {
            cases.push(case!(slicing::strip(s)));
            for (p, q) in [("", ""), ("pm", "1"), ("é", "_"), ("pm_", "é"), ("x", "")] {
                cases.push(case!(slicing::strip_both(s, p, q)));
            }
        }
        cases
    });
    support::assert_equivalent("slicing", SOURCE, &cases);
}

/// Random strings over every UTF-8 width and the escaped categories, sliced
/// at random positions up to two past the end.
#[test]
fn random_slicing_matches_rust() {
    const ALPHABET: [char; 12] =
        ['a', 'Z', '0', ' ', 'é', '\u{7ff}', '日', '\u{ffff}', '😀', '\u{10ffff}', '\u{301}', '\u{200b}'];
    let mut rng = support::Rng::new(0x0005_11ce);
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for _ in 0..600 {
            let len = rng.below(7);
            let s: String = (0..len).map(|_| rng.pick(&ALPHABET)).collect();
            let n = s.len() as u64 + 3;
            let (a, b) = (rng.below(n) as usize, rng.below(n) as usize);
            cases.push(case!(slicing::mid(&s, a, b)));
            cases.push(case!(slicing::from(s.clone(), a)));
            cases.push(case!(slicing::inner(&s, a, b)));
            let p: String = s.chars().take(rng.below(3) as usize).collect();
            cases.push(case!(slicing::strip_both(&s, &p, "")));
        }
        cases
    });
    support::assert_equivalent("slicing_random", SOURCE, &cases);
}
