//! `loop` with `break` and a value (from `match` arms, nested, dropped, or
//! the function's value), `loop` left only by `return`, `while let` on a
//! method and on a variant, and `for` over `a..=b` and `(a..b).rev()` at the
//! ends of every width, with the bounds' panics in Rust's order.

use crate::support;

purecrate_canon::fixture!(mod loop_value = "fixtures/loop_value.rs");

#[test]
fn generated_loops_with_values_and_ranges_match_rust() {
    support::equivalence("loop_value", loop_value::SOURCE, |cases| {
        use loop_value::Step::{self, Add, Skip, Stop};
        grid!(cases, loop_value::first_square; n in [0u32, 1, 2, 15, 16, 17, 1_000_000, 4_294_836_225, 4_294_836_226, u32::MAX]);
        grid!(cases, loop_value::collatz; n in [0u64, 1, 2, 3, 27, 97, 6_148_914_691_236_517_205, u64::MAX]);
        let seqs: Vec<Vec<Step>> = vec![
            vec![],
            vec![Add(1), Add(2)],
            vec![Add(1), Skip, Add(200), Stop, Add(9)],
            vec![Stop, Add(1)],
            vec![Skip, Skip],
            vec![Add(255); 9],
            vec![Add(0), Add(3), Add(0)],
        ];
        for steps in seqs {
            cases.push(case!(loop_value::run_steps(steps.clone())));
            cases.push(case!(loop_value::take_adds(steps.clone())));
        }
        grid!(cases, loop_value::grid; w in [0u8, 1, 3, 255], h in [0u8, 1, 2, 4, 255]);
        grid!(cases, loop_value::count_down; n in [0u8, 1, 7, 255]);
        grid!(cases, loop_value::strip_all; s in ["", "ab", "ababx", "abab", "aab", "é", "ééé"], p in ["", "ab", "a", "é"]);
        grid!(cases, loop_value::sum_numbers; s in ["", "1", "1,2,3", "1,,3", "4294967295,4294967295", "4294967296,1", "7,x,9", "12,"]);
        let u8s = [0u8, 1, 2, 127, 128, 254, 255];
        grid!(cases, loop_value::sum_to; a in u8s, b in u8s);
        let i8s = [i8::MIN, -127, -1, 0, 1, 126, i8::MAX];
        grid!(cases, loop_value::count_to_i8; a in i8s, b in i8s);
        let u64s = [0u64, 1, 9_007_199_254_740_991, 9_007_199_254_740_993, u64::MAX - 2, u64::MAX];
        grid!(cases, loop_value::last_of_u64; a in u64s, b in u64s);
        grid!(cases, loop_value::digits_down; n in u8s);
        grid!(cases, loop_value::down_incl; a in i8s, b in i8s);
        grid!(cases, loop_value::down_u64; a in u64s, b in u64s);
        grid!(cases, loop_value::bounds_order; a in [0u8, 1, 3, 127, 128, 255], b in [0u8, 1, 4, 127, 128, 255]);
    });
}
