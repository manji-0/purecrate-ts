//! `take`, `skip`, `take_while`, `skip_while`, and `partition` (design/01
//! §7.28): `[1, 2, 0].iter().map(|x| 6 / x).take(2).sum()` is 9 (no item
//! past the `n`th is pulled), `skip` runs the stages before it on the items
//! it drops, `take_while` pulls the item it refuses and `skip_while` calls
//! its predicate on no item after its first refusal, each with a panic that
//! comes or not as in Rust; a count of 0 and past the length, the receiver
//! evaluated before the count, chars and bytes of non-ASCII text, a `for`
//! (and one whose body does nothing), `partition` into the function's
//! result and a typed `let`, and `take` and `skip` on ranges at the ends of
//! `u8`, `u16`, `u32`, `i64`, and `u64`, before and after `rev` and
//! `step_by`.

use crate::support;

purecrate_canon::fixture!(mod adaptors_more = "fixtures/adaptors_more.rs");

#[test]
fn generated_adaptors_more_match_rust() {
    support::equivalence("adaptors_more", adaptors_more::SOURCE, |cases| {
        let i32s: [&[i32]; 7] = [&[], &[1, 2, 0], &[0], &[1, 6, 0], &[1, 2, 0, 3], &[12, 3, 4], &[-1, i32::MIN]];
        for xs in i32s {
            cases.push(case!(adaptors_more::first_two(xs)));
            cases.push(case!(adaptors_more::while_big(xs)));
            cases.push(case!(adaptors_more::after_big(xs)));
            for n in [0usize, 1, 2, 3, 4, 9] {
                cases.push(case!(adaptors_more::lazy_take(xs, n)));
                cases.push(case!(adaptors_more::skipped(xs, n)));
                cases.push(case!(adaptors_more::drained(xs, n)));
            }
        }
        let u8s: [&[u8]; 5] = [&[], &[0], &[55, 56], &[3, 200, 1], &[255, 255, 255, 1]];
        for xs in u8s {
            cases.push(case!(adaptors_more::first_checked(xs)));
            for n in [0usize, 1, 2, 5] {
                cases.push(case!(adaptors_more::walked(xs, n)));
                for from in [0usize, 1, 4] {
                    cases.push(case!(adaptors_more::sliced_first(xs, from, n)));
                }
            }
        }
        let u32s: [&[u32]; 5] = [&[], &[7], &[2, 3, 4, 5], &[1, u32::MAX, 3], &[u32::MAX / 2 + 1, 9, 2]];
        for xs in u32s {
            for a in [0usize, 1, 3, 10] {
                cases.push(case!(adaptors_more::odd_after(xs, a)));
                for b in [0usize, 1, 2, 10] {
                    cases.push(case!(adaptors_more::middle(xs, a, b)));
                }
            }
            for k in [0u32, 5, u32::MAX] {
                cases.push(case!(adaptors_more::split_doubled(xs, k)));
            }
        }
        for s in ["", "a", "12ab;cd", "٣4é😀;x", "😀😀é", "99", ";1"] {
            cases.push(case!(adaptors_more::no_digits(s)));
            cases.push(case!(adaptors_more::leading_bytes(s)));
            cases.push(case!(adaptors_more::digits_apart(s)));
            for a in [0usize, 1, 3] {
                for b in [0usize, 2, 7] {
                    cases.push(case!(adaptors_more::chars_cut(s, a, b)));
                }
            }
        }
        let words: Vec<Vec<String>> = vec![
            vec![],
            vec!["".into(), "naïve".into()],
            vec!["ab".into(), "😀😀é".into(), "abc".into(), "".into(), "xyz".into(), "long".into()],
            vec!["é".into(), "ab".into()],
        ];
        for xs in words {
            cases.push(case!(adaptors_more::walked_while(xs.clone())));
            cases.push(case!(adaptors_more::long_words(xs)));
        }
        for xs in [vec![], vec![0i32], vec![-1, 2, -3, 4, 0, i32::MAX], vec![i32::MIN, 5]] {
            cases.push(case!(adaptors_more::signs(xs)));
        }
        // Long ranges cut short, and short ones cut past their end.
        grid!(cases, adaptors_more::range_window; n in [0u64, 3, 10], a in [0usize, 2, 1 << 40], b in [0usize, 3, 1 << 52]);
        grid!(cases, adaptors_more::range_window; n in [u64::MAX], a in [0usize, 2, 1 << 40], b in [0usize, 3]);
        grid!(cases, adaptors_more::range_back; n in [i32::MIN, 0, 1, 9, i32::MAX], k in [0usize, 1, 4, 7]);
        grid!(cases, adaptors_more::range_back; n in [i32::MIN, 0, 1, 9], k in [1 << 40]);
        let u8_ends = [0u8, 1, 250, 255];
        for a in u8_ends {
            for b in u8_ends {
                grid!(cases, adaptors_more::range_cut_stepped; a in [a], b in [b], k in [0usize, 1, 3, 300], n in [0usize, 1, 2, 300]);
            }
        }
        let i64s = [(i64::MIN, i64::MIN + 9), (-3, 4), (4, -3), (i64::MAX - 7, i64::MAX), (i64::MIN, i64::MAX)];
        for (a, b) in i64s {
            grid!(cases, adaptors_more::range_down; a in [a], b in [b], n in [0usize, 2, 6, 1 << 52]);
        }
        grid!(cases, adaptors_more::range_far; n in [0usize, 7, u32::MAX as usize - 1, u32::MAX as usize, 1 << 40]);
        grid!(cases, adaptors_more::range_skip_rev; a in [0u16, 5, u16::MAX - 2], b in [0u16, 9, u16::MAX], n in [0usize, 1, 3, 70_000]);
        grid!(cases, adaptors_more::range_for; n in [0u32, 2, 6, u32::MAX], k in [0usize, 1, 5, 1 << 40]);
    });
}
