//! What an independent review of 0.13.1 found, against Rust.

use crate::support;

use std::cmp::Ordering;

purecrate_canon::fixture!(mod review_holes2 = "fixtures/review_holes2.rs");

use review_holes2::K;

#[test]
fn review_holes2_match_rust() {
    support::equivalence("review_holes2", review_holes2::SOURCE, |cases| {
        for (i, x) in [(0usize, 1i32), (5, i32::MAX), (1, i32::MAX), (5, 0)] {
            cases.push(case!(review_holes2::written_out(i, x)));
        }
        for s in ["", "x,\u{1}", "ab\u{1}", "\u{1}"] {
            cases.push(case!(review_holes2::first_piece(s)));
            cases.push(case!(review_holes2::any_empty(s)));
        }
        for n in [0u32, 5] {
            cases.push(case!(review_holes2::block_scoped(n)));
        }
        cases.push(case!(review_holes2::block_scoped_vec(vec![5])));
        for o in [Ordering::Less, Ordering::Equal, Ordering::Greater] {
            cases.push(case!(review_holes2::narrowed_copy(o)));
        }
        for x in [0u8, 3] {
            cases.push(case!(review_holes2::matched_variant(x)));
        }
        for e in [K::A, K::B(2)] {
            cases.push(case!(review_holes2::reassigned_variant(e)));
        }
        for (s, c) in [("axb", 'b'), ("", 'x')] {
            cases.push(case!(review_holes2::shadowed_pattern(s, c)));
            cases.push(case!(review_holes2::calls_is_x(c)));
        }
        cases.push(case!(review_holes2::top_value()));
    });
}
