//! `for` over a `Vec` or slice, over `s.bytes()`, and over `s.split(c)`
//! (empty pieces, separators of every UTF-8 length): every spelling of the
//! source, overflow in the body (the first one panics on both sides), `?`
//! and early `return` inside nested loops, and a `match` in the body.

use crate::support;

purecrate_canon::fixture!(mod for_each = "fixtures/for_each.rs");

const SOURCE: &str = for_each::SOURCE;

#[test]
fn generated_for_each_matches_rust() {
    use for_each::Line;
    let vecs = [vec![], vec![1], vec![-3, 4, 9], vec![i32::MAX, 1], vec![i32::MIN, -1], vec![5, 5, 0, 10]];
    let strings = ["", "abc", "a1b22", "é9😀", "٣", "12\u{10ffff}"];
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for xs in &vecs {
            cases.push(case!(for_each::sum(xs.clone())));
            cases.push(case!(for_each::sum_iter(xs.clone())));
            cases.push(case!(for_each::sum_owned(xs.clone())));
            for target in [0, 10, 13, -4] {
                cases.push(case!(for_each::first_pair(xs.clone(), target)));
            }
        }
        for bytes in [vec![], vec![0u8], vec![255u8, 255, 1]] {
            cases.push(case!(for_each::sum_slice(bytes.as_slice())));
        }
        for s in strings {
            cases.push(case!(for_each::digits(s)));
            cases.push(case!(for_each::high_bytes(String::from(s))));
            cases.push(case!(for_each::classify(s)));
        }
        let line = |qty: u32, price: i64| Line { qty, price };
        for lines in [
            vec![],
            vec![line(2, 300)],
            vec![line(1, 5), line(1001, 1), line(2000, 1)],
            vec![line(1000, i64::MAX / 999), line(1, 1)],
            vec![line(3, -7), line(0, i64::MIN)],
        ] {
            cases.push(case!(for_each::total(lines.clone())));
        }
        let texts = ["", " ", "a", "a b", " a  b ", "openid profile", "aébéc", "😀x😀", "é", "x\u{10ffff}y"];
        for s in texts {
            for sep in [' ', 'é', '😀', 'x', '\u{10ffff}'] {
                cases.push(case!(for_each::pieces(s, sep)));
            }
            for word in ["", "a", "b", "openid", "é"] {
                cases.push(case!(for_each::has_word(s, word)));
            }
        }
        for xs in [vec![], vec![None], vec![Some(0u8), None, Some(255)]] {
            cases.push(case!(for_each::count_some(xs.clone())));
        }
        cases
    });
    support::assert_equivalent("for_each", SOURCE, &cases);
}
