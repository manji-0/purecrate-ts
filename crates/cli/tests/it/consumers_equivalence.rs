//! Iterator consumers: `all`, `any`, `position`, `count`, and `sum` over
//! `chars()`, `bytes()`, and `iter()`, with a closure or a function name,
//! stopping where std stops, and after `split(c)`; `sum` overflowing; and `for` over
//! `.enumerate()` with `continue`.

use crate::support;

purecrate_canon::fixture!(mod consumers = "fixtures/consumers.rs");

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
            cases.push(case!(consumers::pieces(s)));
            for w in ["", "a", "b", "Hi", "there"] {
                cases.push(case!(consumers::has_token(s, w)));
                cases.push(case!(consumers::token_at(s, w)));
            }
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
    support::assert_equivalent("consumers", consumers::SOURCE, &cases);
}

/// The consumers print as calls to the runtime's `Iter`, the closure as an
/// arrow, with no labelled loop or inline function around them.
#[test]
fn consumers_are_runtime_calls() {
    let source = "pub fn digits(s: &str) -> bool { !s.is_empty() && s.bytes().all(|b| b >= b'0' && b <= b'9') }\n\
                  pub fn total(xs: Vec<u8>) -> u8 { xs.iter().sum::<u8>() }\n\
                  pub fn at(s: &str) -> Option<usize> { s.chars().position(|c| c == '@') }\n";
    let krate = purecrate_syntax::parse_source("consume", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    // Without line breaks: a long call is opened one argument per line.
    let file = |stem: &str| {
        let src = pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
        src.lines().map(str::trim).collect::<Vec<_>>().join(" ").replace("( ", "(").replace(", )", ")")
    };
    assert!(file("digits").contains("Iter.all(Str.bytes(s), (b: U8): boolean => "), "{}", file("digits"));
    assert!(file("total").contains("Iter.sum(xs, Int.u8.add, 0 as U8)"), "{}", file("total"));
    assert!(file("at").contains("Iter.position(s as Iterable<Char>, (c: Char): boolean => "), "{}", file("at"));
    for stem in ["digits", "total", "at"] {
        assert!(!file(stem).contains("(() =>") && !file(stem).contains("$acc"), "{}", file(stem));
    }
}
