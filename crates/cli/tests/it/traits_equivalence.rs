//! Traits (design/01 §7.25): default bodies through the dictionary, a
//! supertrait's members through a subtrait's bound, an associated const
//! with a default and an override, a receiver-less function (`T::zero()`),
//! a generic implementor, an inherent `impl<T: Priced>` method, two bounds
//! on one parameter and two bounded parameters, `impl Tr` as a parameter's
//! type, every call on a known type, an empty list, non-ASCII text from a
//! trait method, and `i64` overflow inside a default body and an impl.

use crate::support;

purecrate_canon::fixture!(mod traits = "fixtures/traits.rs");

#[test]
fn generated_traits_match_rust() {
    support::equivalence("traits", traits::SOURCE, |cases| {
        let item = |sku: &str, price: i64| traits::Item { sku: sku.to_string(), price };
        for (i, c) in [
            (item("a", 1), traits::Coin::Small),
            (item("", -4), traits::Coin::Big(9)),
            (item("é😀", i64::MAX / 2), traits::Coin::Big(0)),
            (item("x", i64::MAX), traits::Coin::Small),
            (item("x", 1), traits::Coin::Big(i64::MAX - 50)),
        ] {
            cases.push(case!(traits::known(i, c)));
        }
        for prices in [vec![], vec![1i64], vec![3, -2, 5], vec![i64::MAX / 2 + 1], vec![i64::MIN]] {
            for name in ["", "é😀"] {
                cases.push(case!(traits::items(prices.clone(), name.to_string())));
            }
        }
        for ns in [vec![], vec![0i64], vec![0, 5, -3], vec![i64::MAX - 99], vec![i64::MAX - 100]] {
            cases.push(case!(traits::coins(ns)));
        }
        for (fee, n) in [(0i64, 0usize), (5, 3), (-1, 4), (i64::MAX, 2), (i64::MAX / 4, 1)] {
            cases.push(case!(traits::bags(fee, n)));
        }
        for (i, n) in [(item("ab", 2), 3i64), (item("€", -7), 0), (item("q", 1), i64::MAX / 2)] {
            cases.push(case!(traits::mixed(i, n)));
        }
    });
}
