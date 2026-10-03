//! `while`, `break`, and `continue`: jumps inside `match` arms (a `switch`
//! in TS, which a bare `break` would leave instead of the loop), nested
//! loops, `?` in a `while` condition, early `return`, and overflow.

use crate::support;


purecrate_canon::fixture!(mod while_break = "fixtures/while_break.rs");

const SOURCE: &str = while_break::SOURCE;

#[test]
fn generated_loops_with_jumps_match_rust() {
    use while_break::Token::{self, Digit, Skip, Stop};
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for n in [0u32, 1, 2, 3, 7, 27, 97, 1_431_655_765, u32::MAX] {
            cases.push(case!(while_break::steps(n)));
        }
        let seqs: Vec<Vec<Token>> = vec![
            vec![],
            vec![Digit(1), Digit(2)],
            vec![Digit(1), Skip, Digit(2), Stop, Digit(9)],
            vec![Stop, Digit(1)],
            vec![Skip, Skip],
            vec![Digit(9); 8],
        ];
        for tokens in seqs {
            cases.push(case!(while_break::read(tokens.clone())));
        }
        for n in [0u32, 1, 4] {
            for width in [0u32, 1, 5, 200] {
                cases.push(case!(while_break::rows(n, width)));
            }
        }
        for budget in [0u32, 3, 5, 10] {
            cases.push(case!(while_break::guarded(budget)));
        }
        for xs in [vec![], vec![1, 3], vec![1, 4, 6], vec![46_341], vec![-7, -46_342]] {
            cases.push(case!(while_break::first_even_square(xs.clone())));
        }
        for s in ["", "ab", "abcdef", "ab1cd", "é", "Zabc"] {
            cases.push(case!(while_break::prefix_len(s)));
        }
        cases
    });
    support::assert_equivalent("while_break", SOURCE, &cases);
}

/// A loop whose jumps are not inside a `match` has no label, and a jump that
/// is all of its `if` sits on the `if`'s line; one inside a `match` names
/// its loop.
#[test]
fn a_label_only_where_a_switch_would_take_the_jump() {
    let source = "pub fn first_zero(xs: &[u32]) -> u32 {\n\
                      let mut n = 0u32;\n\
                      for x in xs { if *x == 0 { break; } n += 1; }\n\
                      n\n\
                  }\n\
                  pub enum K { A, B }\n\
                  pub fn until_b(ks: &[K]) -> u32 {\n\
                      let mut n = 0u32;\n\
                      for k in ks { match k { K::A => n += 1, K::B => break } }\n\
                      n\n\
                  }\n";
    let krate = purecrate_syntax::parse_source("labels", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    let first = file("first-zero");
    assert!(!first.contains("loop") && first.contains("if (x === 0) break;"), "{first}");
    assert!(file("until-b").contains("loop: for") && file("until-b").contains("break loop;"), "{}", file("until-b"));
}
