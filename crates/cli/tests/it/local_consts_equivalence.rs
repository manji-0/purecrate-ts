//! `const` items in function bodies: used before their declaration,
//! referring to crate consts and discriminants, unused, floats, named like
//! a crate item, inside an arm, and overflowing where their value makes
//! the computation overflow.

use crate::support;

purecrate_canon::fixture!(mod local_consts = "fixtures/local_consts.rs");

#[test]
fn generated_local_consts_match_rust() {
    support::equivalence("local_consts", local_consts::SOURCE, |cases| {
        grid!(cases, local_consts::limit; x in [0, 1, 7, 715_827_882, 715_827_883, u32::MAX]);
        grid!(cases, local_consts::scale; x in [0.0, -1.5, 4.0, f64::MAX]);
        grid!(cases, local_consts::classify; n in [0, 1, 3, 4, 255]);
        grid!(cases, local_consts::bump; x in [0, 55, 56, 255]);
    });
}
