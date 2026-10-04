//! `bool` literal patterns: both named, one and `_`, `|`, in a tuple
//! `match` with guards, and in `matches!`.

use crate::support;

purecrate_canon::fixture!(mod bool_patterns = "fixtures/bool_patterns.rs");

#[test]
fn generated_bool_patterns_match_rust() {
    support::equivalence("bool_patterns", bool_patterns::SOURCE, |cases| {
        grid!(
            cases, [bool_patterns::pick, bool_patterns::only_true, bool_patterns::either, bool_patterns::is_on];
            b in [true, false]
        );
        grid!(
            cases, bool_patterns::gate;
            b in [true, false], code in [None, Some(0), Some(100), Some(101)], strict in [true, false]
        );
    });
}
