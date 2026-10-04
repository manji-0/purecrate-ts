//! `const` items in function bodies: used before their declaration,
//! referring to crate consts and discriminants, unused, floats, named like
//! a crate item, inside an arm, and overflowing where their value makes
//! the computation overflow.

use crate::support;

purecrate_canon::fixture!(mod local_consts = "fixtures/local_consts.rs");

#[test]
fn generated_local_consts_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for x in [0, 1, 7, 715_827_882, 715_827_883, u32::MAX] {
            cases.push(case!(local_consts::limit(x)));
        }
        for x in [0.0, -1.5, 4.0, f64::MAX] {
            cases.push(case!(local_consts::scale(x)));
        }
        for n in [0, 1, 3, 4, 255] {
            cases.push(case!(local_consts::classify(n)));
        }
        for x in [0, 55, 56, 255] {
            cases.push(case!(local_consts::bump(x)));
        }
        cases
    });
    support::assert_equivalent("local_consts", local_consts::SOURCE, &cases);
}
