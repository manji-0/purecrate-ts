//! Shared references, `&self`, `Self`, newtypes and `Type::method` calls:
//! every case in `fixtures/borrow.rs` agrees between Rust and the generated
//! package.

use crate::support;

purecrate_canon::fixture!(mod borrow = "fixtures/borrow.rs");

const SOURCE: &str = borrow::SOURCE;

#[test]
fn generated_borrowing_code_matches_rust() {
    let cases = support::quietly(|| {
        vec![
            case!(borrow::trip(0i32, 3i32, 10i32)),
            case!(borrow::trip(5i32, 2i32, 2i32)),
            case!(borrow::trip(i32::MIN, 0i32, 1i32)),
            case!(borrow::back(1i32, 4i32)),
            case!(borrow::back(4i32, 1i32)),
            case!(borrow::hours(&12i32, false, 0i32)),
            case!(borrow::hours(&12i32, true, 4i32)),
            case!(borrow::hours(&12i32, true, 0i32)),
            case!(borrow::hours(&-7i32, true, 2i32)),
            case!(borrow::label("north", "south", true)),
            case!(borrow::label("north", "south", false)),
        ]
    });
    support::assert_equivalent("borrow", SOURCE, &cases);
}
