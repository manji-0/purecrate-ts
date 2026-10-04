//! `bool` literal patterns: both named, one and `_`, `|`, in a tuple
//! `match` with guards, and in `matches!`.

use crate::support;

purecrate_canon::fixture!(mod bool_patterns = "fixtures/bool_patterns.rs");

#[test]
fn generated_bool_patterns_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for b in [true, false] {
            cases.push(case!(bool_patterns::pick(b)));
            cases.push(case!(bool_patterns::only_true(b)));
            cases.push(case!(bool_patterns::either(b)));
            cases.push(case!(bool_patterns::is_on(b)));
            for code in [None, Some(0), Some(100), Some(101)] {
                for strict in [true, false] {
                    cases.push(case!(bool_patterns::gate(b, code, strict)));
                }
            }
        }
        cases
    });
    support::assert_equivalent("bool_patterns", bool_patterns::SOURCE, &cases);
}
