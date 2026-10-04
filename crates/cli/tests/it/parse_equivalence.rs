//! `str::parse` into each integer type, and `Result::ok` / `map_err`
//! around it: what Rust's `from_str_radix(s, 10)` accepts and refuses, at
//! each type's bounds.

use crate::support;

purecrate_canon::fixture!(mod parse = "fixtures/parse.rs");

const SOURCE: &str = parse::SOURCE;

const TEXTS: [&str; 32] = [
    "",
    "0",
    "7",
    "+7",
    "-7",
    "+",
    "-",
    "+-1",
    "-+1",
    "007",
    "-0",
    " 1",
    "1 ",
    "1_000",
    "0x10",
    "1e3",
    "١٢",
    "12a",
    "127",
    "128",
    "-128",
    "-129",
    "255",
    "256",
    "65535",
    "65536",
    "2147483648",
    "-2147483649",
    "4294967296",
    "9223372036854775808",
    "18446744073709551615",
    "18446744073709551616",
];

#[test]
fn generated_parse_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for s in TEXTS {
            cases.push(case!(parse::as_i8(s)));
            cases.push(case!(parse::as_i16(s)));
            cases.push(case!(parse::as_i32(s)));
            cases.push(case!(parse::as_i64(s)));
            cases.push(case!(parse::as_u8(s)));
            cases.push(case!(parse::as_u16(s)));
            cases.push(case!(parse::as_u32(s)));
            cases.push(case!(parse::as_u64(s)));
            cases.push(case!(parse::or_zero(s)));
            cases.push(case!(parse::doubled(s)));
            cases.push(case!(parse::kept(s)));
            cases.push(case!(parse::named(s)));
        }
        // `usize` above 2^53−1 panics in TS (design/01 §3); below, it agrees.
        for s in ["", "0", "+9007199254740991", "-1", "1x"] {
            cases.push(case!(parse::as_usize(s)));
        }
        cases
    });
    support::assert_equivalent("parse", SOURCE, &cases);
}

/// A file that only names `Result` as a type imports it with `type`; one
/// that builds `Result.ok` / `Result.err` imports the value.
#[test]
fn result_as_a_type_only_is_a_type_import() {
    let source = "pub fn as_i32(r: Result<i32, i32>) -> Option<i32> { r.ok() }\n\
                  pub fn wrap(n: i32) -> Result<i32, i32> { Ok(n) }\n";
    let krate = purecrate_syntax::parse_source("results", source).expect("parse");
    let typed = purecrate_check::accept(&krate).expect("accept");
    let pkg = purecrate_pack::assemble(&typed);
    let file = |stem: &str| pkg.files.iter().find(|f| f.stem == stem).expect(stem).source.clone();
    assert!(file("as-i32").contains("import type { I32, Result }"), "{}", file("as-i32"));
    assert!(file("wrap").contains("import { Result, type I32 }"), "{}", file("wrap"));
}
