//! The std allow-list beyond strings: `Vec::is_empty`, `Option::is_some`,
//! `Option::is_none`, on values, borrows, fields, and falsy payloads (`0`,
//! `false`, `""`), agree between Rust and the generated package.

use crate::support;

purecrate_canon::fixture!(mod std_methods = "fixtures/std_methods.rs");

const SOURCE: &str = std_methods::SOURCE;

#[test]
fn generated_std_methods_match_rust() {
    use std_methods::Cart;
    let cart = |items: Vec<u32>, coupon: Option<&str>| Cart { items, coupon: coupon.map(String::from) };
    let cart_js = |items: &[u32], coupon: Option<&str>| {
        format!("{{ items: {}, coupon: {} }}", support::Js::js(items), support::Js::js(&coupon))
    };
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for xs in [vec![], vec![0], vec![-1, 2]] {
            cases.push(case!(std_methods::empty(xs.clone())));
            for s in ["", "a", "é"] {
                cases.push(case!(std_methods::blank(xs.as_slice(), s)));
            }
        }
        for x in [None, Some(0), Some(-1)] {
            cases.push(case!(std_methods::some(x)));
        }
        for x in [None, Some(String::new()), Some(String::from("x"))] {
            cases.push(case!(std_methods::none(x.clone())));
        }
        for x in [None, Some(0u64), Some(u64::MAX)] {
            for y in [None, Some(false), Some(true)] {
                cases.push(case!(std_methods::zero_is_some(x, y)));
            }
        }
        for items in [vec![], vec![0u32]] {
            for coupon in [None, Some(""), Some("SAVE")] {
                for note in [None, Some('\0'), Some('😀')] {
                    let rust = std_methods::ready(&cart(items.clone(), coupon), &note);
                    cases.push(support::Case {
                        name: "ready",
                        call: format!("ready({}, {})", cart_js(&items, coupon), support::Js::js(&note)),
                        rust: support::Show::show(&rust),
                    });
                }
            }
        }
        cases
    });
    support::assert_equivalent("std_methods", SOURCE, &cases);
}
