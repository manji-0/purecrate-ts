//! Iterator adaptors and ranges: `filter_map`, `flat_map`, and `flatten`
//! running lazily (an overflow after the item `find` takes does not
//! happen), `windows` and `chunks` with a size of 0 (literal or not), empty
//! input, and sizes past the length, `dedup` on floats (NaN, `-0.0`),
//! strings, and tuples, `step_by` with `rev` before, after, and around it
//! at the edges of `u8`, `u16`, `i32`, `u64`, and `i64` and with a step of
//! 0, ranges before a stage or a consumer, and accumulators whose
//! unsuffixed start their first use types.

use crate::support;

purecrate_canon::fixture!(mod adaptors = "fixtures/adaptors.rs");

#[test]
fn generated_adaptors_match_rust() {
    support::equivalence("adaptors", adaptors::SOURCE, |cases| {
        let i32s: [&[i32]; 5] = [&[], &[0], &[3, -1, 4, -1, 5], &[i32::MIN, i32::MAX], &[-7, -8]];
        for xs in i32s {
            cases.push(case!(adaptors::positives(xs)));
            cases.push(case!(adaptors::widest(xs)));
            for n in [0usize, 1, 2, 3, 6] {
                cases.push(case!(adaptors::rises(xs, n)));
            }
        }
        for s in ["", "a1b2", "٣4é5😀6", "12,x,,30", "4294967295,1", "4294967295,0", "-1,2"] {
            cases.push(case!(adaptors::digits(s)));
            cases.push(case!(adaptors::parsed_total(s)));
        }
        let u8s: [&[u8]; 7] =
            [&[], &[0], &[101, 151, 200], &[200, 160], &[1, 1, 0, 0, 0, 2], &[155, 156, 155, 155], &[255, 0, 255]];
        for xs in u8s {
            cases.push(case!(adaptors::first_big(xs)));
            cases.push(case!(adaptors::no_window(xs)));
            cases.push(case!(adaptors::heads_back(xs)));
            cases.push(case!(adaptors::odd_slots(xs)));
            cases.push(case!(adaptors::zeros(xs)));
            cases.push(case!(adaptors::deduped_by_overflow(xs.to_vec())));
            for n in [0usize, 1, 2, 4, 9] {
                cases.push(case!(adaptors::chunk_sums(xs, n)));
            }
        }
        for xs in [vec![], vec![None], vec![Some(1u32), None, Some(u32::MAX)], vec![Some(2), Some(3)]] {
            cases.push(case!(adaptors::some_total(xs)));
        }
        let grids: Vec<Vec<Vec<u32>>> =
            vec![vec![], vec![vec![]], vec![vec![1, 2], vec![], vec![3]], vec![vec![u32::MAX], vec![1]]];
        for xs in grids {
            cases.push(case!(adaptors::joined(xs.clone())));
            cases.push(case!(adaptors::grid_total(xs)));
        }
        let i64s: [&[Option<i64>]; 4] = [&[], &[None, None], &[Some(-3), None, Some(9)], &[Some(i64::MAX), Some(1)]];
        for xs in i64s {
            cases.push(case!(adaptors::some_sum(xs)));
        }
        let lines: Vec<Vec<String>> = vec![
            vec![],
            vec!["".into(), "naïve".into()],
            vec!["😀😀".into(), "abc".into(), "".into()],
            vec!["a".into(), "a".into(), "bb".into(), "cc".into(), "a".into()],
        ];
        cases.push(case!(adaptors::words(lines.clone())));
        cases.push(case!(adaptors::words(Vec::<Vec<String>>::new())));
        cases.push(case!(adaptors::longest(&lines)));
        for line in lines {
            cases.push(case!(adaptors::spread(line.clone())));
            cases.push(case!(adaptors::pairs(line.clone())));
            cases.push(case!(adaptors::deduped_by_len(line.clone())));
        }
        let u32s: [&[u32]; 6] = [&[], &[5], &[1, 2, 2, 3], &[3, 2], &[4, u32::MAX, 6], &[7, 7, 7, 8, 7]];
        for xs in u32s {
            cases.push(case!(adaptors::doubled(xs)));
            cases.push(case!(adaptors::halves(xs)));
            cases.push(case!(adaptors::sorted(xs)));
            cases.push(case!(adaptors::deduped(xs.to_vec())));
        }
        let u16s: [&[u16]; 4] = [&[], &[9], &[u16::MAX, u16::MAX, 1], &[1, 2, 3]];
        for xs in u16s {
            cases.push(case!(adaptors::best_pair(xs)));
        }
        let floats = [vec![], vec![1.0, f64::NAN, f64::NAN, -0.0, 0.0, 0.0], vec![0.5, 0.5, -1.5, 0.5]];
        for xs in floats {
            cases.push(case!(adaptors::deduped_floats(xs)));
        }
        let pairs = vec![
            ("é".to_string(), Some(1u8)),
            ("é".to_string(), Some(1)),
            ("é".to_string(), None),
            ("e".to_string(), None),
            ("e".to_string(), None),
        ];
        cases.push(case!(adaptors::deduped_pairs(pairs)));
        cases.push(case!(adaptors::deduped_pairs(Vec::<(String, Option<u8>)>::new())));
        grid!(cases, adaptors::evens; n in [0u32, 1, 2, 9, 10, 92_000]);
        let u8_ends = [0u8, 1, 5, 250, 254, 255];
        for a in u8_ends {
            for b in u8_ends {
                for k in [0usize, 1, 2, 3, 300] {
                    cases.push(case!(adaptors::stepped(a, b, k)));
                    cases.push(case!(adaptors::stepped_through(a, b, k)));
                    cases.push(case!(adaptors::down_by(a, b, k)));
                    cases.push(case!(adaptors::by_then_down(a, b, k)));
                }
            }
        }
        // Each range short, or stepped by enough to be short, near the ends.
        let i32s = [(i32::MIN, i32::MIN + 9), (-5, 7), (0, 0), (7, -5), (i32::MAX - 9, i32::MAX), (i32::MIN, i32::MAX)];
        for (a, b) in i32s {
            for k in [0usize, 1, 4, 7, 1 << 31] {
                if a != i32::MIN || b != i32::MAX || k == 1 << 31 {
                    cases.push(case!(adaptors::signed_by_then_down(a, b, k)));
                }
            }
        }
        let u16s = [(0u16, 0u16), (3, 10), (10, 3), (u16::MAX - 5, u16::MAX), (0, u16::MAX)];
        for (a, b) in u16s {
            grid!(cases, adaptors::down_by_back; a in [a], b in [b], k in [0usize, 3, 4, 4000]);
        }
        let u64s = [(0u64, 5u64), (u64::MAX - 10, u64::MAX), (u64::MAX, u64::MAX), (u64::MAX, 5), (0, u64::MAX)];
        for (a, b) in u64s {
            for k in [0usize, 3, 4, 1 << 52] {
                if b - a.min(b) < 100 || k == 1 << 52 {
                    cases.push(case!(adaptors::wide_steps(a, b, k)));
                }
            }
        }
        let i64s = [(i64::MIN, i64::MIN + 9), (-3, 4), (4, -3), (i64::MAX - 7, i64::MAX)];
        for (a, b) in i64s {
            grid!(cases, adaptors::wide_down; a in [a], b in [b], k in [0usize, 1, 5]);
        }
        grid!(cases, adaptors::stride_sum; n in [0u32, 1, 10, 1000], k in [0usize, 1, 3, 5000]);
        grid!(cases, adaptors::first_square_over; n in [0u32, 10, 70_000], limit in [0u32, 99, 4_000_000_000, u32::MAX]);
        grid!(cases, adaptors::triangle; n in [0u16, 1, 100, 361, 362]);
        grid!(cases, adaptors::between; a in [i8::MIN, -1, 0, i8::MAX], b in [i8::MIN, 0, 3, i8::MAX]);
        grid!(cases, adaptors::multiples; n in [0u32, 1, 20], d in [1u32, 3, 0]);
        grid!(cases, adaptors::last_below; n in [-1i64, 0, 50], limit in [i64::MIN, 0, 10, i64::MAX]);
        grid!(cases, adaptors::upto; n in [0u32, 1, 100, 92_681, 92_682]);
        let i16s: [&[i16]; 4] = [&[], &[5, 3], &[-1, i16::MIN, 7], &[0]];
        for xs in i16s {
            cases.push(case!(adaptors::lowest(xs)));
        }
        grid!(cases, adaptors::halved; k in [false, true]);
    });
}
