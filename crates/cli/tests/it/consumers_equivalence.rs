//! Iterator consumers: `all`, `any`, `position`, `count`, and `sum` over
//! `chars()`, `bytes()`, and `iter()`, with a closure or a function name,
//! stopping where std stops; `sum` overflowing; and `for` over
//! `.enumerate()` with `continue`.

use crate::support;

purecrate_canon::fixture!(mod consumers = "fixtures/consumers.rs");

const SOURCE: &str = consumers::SOURCE;

#[test]
fn generated_consumers_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in ["", "0", "0123", "12a3", "é1", "Hi there", "a@b@c", "x😀@", " a  b "] {
            cases.push(case!(consumers::all_digits(s)));
            cases.push(case!(consumers::any_upper(String::from(s))));
            cases.push(case!(consumers::at_sign(s)));
            cases.push(case!(consumers::chars(s)));
            cases.push(case!(consumers::byte_sum(s)));
            for c in ['@', 'a', '😀', 'z'] {
                cases.push(case!(consumers::char_at(s, c)));
            }
        }
        let u32s: [&[u32]; 5] = [&[], &[0], &[1, 2, 3], &[5, 0, 12], &[9, 9, 10]];
        for xs in u32s {
            cases.push(case!(consumers::has_zero(xs)));
            cases.push(case!(consumers::all_small(xs.to_vec())));
            for t in [0, 9, 12] {
                cases.push(case!(consumers::last_index(xs, t)));
            }
        }
        let u8s: [&[u8]; 6] = [&[], &[1, 2], &[100, 100, 55], &[100, 100, 56], &[60, 1], &[1, 60, 255]];
        for xs in u8s {
            cases.push(case!(consumers::total(xs)));
            cases.push(case!(consumers::stops_early(xs)));
        }
        for xs in [vec![], vec![1, -2, 3], vec![i64::MAX, 1], vec![i64::MIN, -1], vec![i64::MAX, -1, 1]] {
            cases.push(case!(consumers::total_wide(xs.clone())));
            cases.push(case!(consumers::items(xs.clone())));
        }
        cases
    });
    support::assert_equivalent("consumers", SOURCE, &cases);
}
