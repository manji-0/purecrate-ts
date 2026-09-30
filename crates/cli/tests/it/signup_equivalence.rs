//! `examples/signup` accepts and rejects the same input in Rust and in the
//! generated package, error payload included. The e-mail check is also held
//! against the regular expression the WHATWG spec states, run in node: the
//! byte-by-byte reading must be the same language.

use crate::support;


use std::process::Command;

purecrate_canon::fixture!(mod signup = "../../../examples/signup/src/lib.rs", "fixtures/signup_driver.rs");

const SOURCE: &str = signup::SOURCE;

fn emails() -> Vec<String> {
    let label63 = "a".repeat(63);
    let label64 = "a".repeat(64);
    let mut out: Vec<String> = [
        "", "a", "@", "a@", "@b", "a@b", "a.b@c.d", "user+tag@example.co.jp",
        "!#$%&'*+/=?^_`{|}~-@x", "a b@c", "a@b@c", "a@-b", "a@b-", "a@b-c", "a@b..c", "a@.b", "a@b.",
        "a@b_c", "é@b", "a@é", "a@😀", ".@b", "a\"b@c", "a@1.2.3.4", "A@B",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    out.push(format!("a@{label63}"));
    out.push(format!("a@{label64}"));
    out.push(format!("a@{label63}.{label63}"));
    out.push(format!("a@b.{label64}"));
    out
}

fn passwords() -> Vec<String> {
    let mut out: Vec<String> = ["", "passwordpassword", "123456789012345", "qwertyuiopasdfgh"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for n in [14, 15, 64, 65] {
        out.push("x".repeat(n));
        out.push("é".repeat(n));
        out.push("😀".repeat(n));
    }
    // JS `.length` and the byte length both say 16 or more; the code points are fewer than 15.
    out.push("😀".repeat(8));
    out.push("é".repeat(8));
    out
}

#[test]
fn signup_matches_rust() {
    let cases = support::quietly(|| {
        let mut cases = Vec::new();
        for e in emails() {
            cases.push(case!(signup::parse_email(e.clone())));
        }
        for p in passwords() {
            cases.push(case!(signup::parse_password(p.clone())));
        }
        for (e, p) in [("a@b", "x"), ("a", "x"), ("a@b", "passwordpassword"), ("a@b", "abcdefghijklmnop")] {
            cases.push(case!(signup::parse_signup(e.to_string(), p.to_string())));
        }
        cases
    });
    support::assert_equivalent("signup", SOURCE, &cases);
}

/// The WHATWG "valid e-mail address" regular expression, verbatim.
const WHATWG: &str = r"/^[a-zA-Z0-9.!#$%&'*+\/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/";

#[test]
fn email_is_the_whatwg_language() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let inputs = emails();
    let json = format!(
        "[{}]",
        inputs.iter().map(|s| support::Js::js(s.as_str())).collect::<Vec<_>>().join(", ")
    );
    let script = format!("const re = {WHATWG};\nconsole.log({json}.map((s) => re.test(s)).join(\"\\n\"));\n");
    let output = Command::new("node").args(["-e", &script]).output().expect("run node");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let spec: Vec<bool> = String::from_utf8(output.stdout)
        .expect("utf8")
        .lines()
        .map(|l| l == "true")
        .collect();
    let differ: Vec<String> = inputs
        .iter()
        .zip(&spec)
        .filter(|(s, ok)| signup::Email::parse(s.to_string()).is_ok() != **ok)
        .map(|(s, ok)| format!("{s:?}: spec {ok}"))
        .collect();
    assert_eq!(spec.len(), inputs.len());
    assert!(differ.is_empty(), "differs from the WHATWG expression:\n{}", differ.join("\n"));
}
