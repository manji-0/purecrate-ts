//! Closed structs (design/01 §4): a struct with a field that is not `pub`
//! is branded and has no `of`, so outside the crate its value comes only from
//! the crate's functions, as in Rust. Inside the package, construction goes
//! through the file's `$of`, also from another file and in a struct update.

#[macro_use]
mod support;

use std::fs;

use purecrate_check::accept;
use purecrate_pack::{assemble, disk_path};
use purecrate_syntax::parse_source;

purecrate_canon::fixture!(mod closed = "fixtures/closed.rs");

const SOURCE: &str = closed::SOURCE;

#[test]
fn closed_types_are_built_through_the_crate_and_match_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for code in 0u8..3 {
            for age in [0u8, 17, 18, 254, 255] {
                cases.push(case!(closed::signup(code, age)));
            }
            cases.push(case!(closed::adult_after(code)));
        }
        cases
    });
    support::assert_equivalent("closed", SOURCE, &cases);
}

/// What a consumer of the package can and cannot write. Each
/// `@ts-expect-error` must be an error, or `tsc` fails on the directive.
const CONSUMER: &str = r#"import { Account, Email, type EmailError, type Result, type U8 } from "./index.ts";
import * as pkg from "./index.ts";

declare const age: U8;
const parsed: Result<Email, EmailError> = Email.parse("a@example.com");
if (parsed.kind === "Ok") {
  const opened = Account.open(parsed.value, age);
  if (opened.kind === "Ok") {
    const older: Account = Account.birthday(opened.value);
    const years: U8 = older.age;
    void years;
  }
}

// @ts-expect-error: a closed newtype has no `of`
Email.of("x");
// @ts-expect-error: a closed struct has no `of`
Account.of(parsed, age);
// @ts-expect-error: a plain string is not an `Email`
const e: Email = "x";
// @ts-expect-error: an object literal is not an `Account`
const a: Account = { email: e, age };
// @ts-expect-error: the package-internal constructor is not exported
pkg.Email$of;
void a;
"#;

#[test]
fn a_consumer_cannot_build_a_closed_type() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let krate = parse_source("closed", SOURCE).expect("parse");
    let typed = accept(&krate).unwrap_or_else(|d| panic!("closed rejected: {d:#?}"));
    let dir = std::env::temp_dir().join(format!("purecrate-closed-consumer-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear scratch");
    }
    for file in assemble(&typed).files {
        let path = dir.join(disk_path(&file.stem));
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(path, file.source).expect("write");
    }
    fs::write(dir.join("src/consumer.ts"), CONSUMER).expect("write consumer");
    support::link_purecrate(&dir);
    support::typecheck(&dir);
    fs::remove_dir_all(&dir).ok();
}
