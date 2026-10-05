//! `examples/signup` accepts and rejects the same input in Rust and in the
//! generated package, error payload included. The e-mail check is also held
//! against the regular expression the WHATWG spec states, run in node: the
//! byte-by-byte reading must be the same language.

use crate::support;

use std::process::Command;

purecrate_canon::fixture!(mod signup = "../../../examples/signup/src/lib.rs", "fixtures/signup_driver.rs");

fn emails() -> Vec<String> {
    let label63 = "a".repeat(63);
    let label64 = "a".repeat(64);
    let mut out: Vec<String> = [
        "",
        "a",
        "@",
        "a@",
        "@b",
        "a@b",
        "a.b@c.d",
        "user+tag@example.co.jp",
        "!#$%&'*+/=?^_`{|}~-@x",
        "a b@c",
        "a@b@c",
        "a@-b",
        "a@b-",
        "a@b-c",
        "a@b..c",
        "a@.b",
        "a@b.",
        "a@b_c",
        "é@b",
        "a@é",
        "a@😀",
        ".@b",
        "a\"b@c",
        "a@1.2.3.4",
        "A@B",
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
    let mut out: Vec<String> =
        ["", "passwordpassword", "123456789012345", "qwertyuiopasdfgh"].iter().map(|s| s.to_string()).collect();
    for n in [14, 15, 64, 65] {
        out.push("x".repeat(n));
        out.push("é".repeat(n));
        out.push("😀".repeat(n));
    }
    // JS `.length` and the byte length both say 16 or more; the code points are fewer than 15.
    out.push("😀".repeat(8));
    out.push("é".repeat(8));
    // Not normalized: 15 code points, 8 in NFC.
    out.push(DECOMPOSED.to_string());
    out
}

#[test]
fn signup_matches_rust() {
    support::equivalence("signup", signup::SOURCE, |cases| {
        for e in emails() {
            cases.push(case!(signup::parse_email(e.clone())));
        }
        for p in passwords() {
            cases.push(case!(signup::parse_password(p.clone())));
        }
        for (e, p) in [("a@b", "x"), ("a", "x"), ("a@b", "passwordpassword"), ("a@b", "abcdefghijklmnop")] {
            cases.push(case!(signup::parse_signup(e.to_string(), p.to_string())));
        }
        for (e, p, _) in PASSWORD_IS_EMAIL {
            cases.push(case!(signup::parse_signup(e.to_string(), p.to_string())));
        }
        cases.push(case!(signup::parse_signup(SHORT_EMAIL.to_string(), SHORT_EMAIL.to_string())));
    });
}

/// An e-mail and a password, and whether the sign-up is refused as a
/// password that is the address (ignoring ASCII case only).
const PASSWORD_IS_EMAIL: [(&str, &str, bool); 8] = [
    ("someone@example.com", "someone@example.com", true),
    ("someone@example.com", "SomeOne@Example.COM", true),
    ("SOMEONE@EXAMPLE.COM", "someone@example.com", true),
    // Not the address: one character more or less, or a non-ASCII letter.
    ("someone@example.com", "someone@example.co", false),
    ("someone@example.com", "someone@example.com.", false),
    ("someone@example.com", "someone@exämple.com", false),
    // `K` and the Kelvin sign fold together in Unicode, not in ASCII.
    ("kelvin@example.com", "\u{212a}elvin@example.com", false),
    ("someone@example.com", "xsomeone@example.comx", false),
];

#[test]
fn password_is_not_the_email() {
    for (e, p, refused) in PASSWORD_IS_EMAIL {
        let got = signup::Signup::parse(e.to_string(), p.to_string());
        assert_eq!(matches!(got, Err(signup::SignupError::PasswordIsEmail)), refused, "{e:?} / {p:?}");
        assert_eq!(got.is_ok(), !refused, "{e:?} / {p:?}");
    }
    // The password's own errors come first: a 14-character address as the
    // password is too short.
    let short = signup::Signup::parse(SHORT_EMAIL.to_string(), SHORT_EMAIL.to_string());
    assert!(matches!(short, Err(signup::SignupError::Password(signup::PasswordError::TooShort))));
}

const SHORT_EMAIL: &str = "abcdefg@hij.kl";

/// `e` and a combining acute, seven times, then `x`: 15 code points, 8
/// once NFC composes each pair into `é`.
const DECOMPOSED: &str = "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}x";

#[test]
fn password_is_counted_as_given() {
    // Not normalized (the header's policy): the decomposed form is long
    // enough, the composed one is not.
    assert!(signup::Password::parse(DECOMPOSED.to_string()).is_ok());
    let composed = "\u{e9}".repeat(7) + "x";
    assert!(matches!(signup::Password::parse(composed), Err(signup::PasswordError::TooShort)));
}

/// The WHATWG "valid e-mail address" regular expression, verbatim.
const WHATWG: &str = r"/^[a-zA-Z0-9.!#$%&'*+\/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$/";

#[test]
fn email_is_the_whatwg_language() {
    if std::env::var_os("PURECRATE_SKIP_NODE").is_some() {
        return;
    }
    let inputs = emails();
    let json = format!("[{}]", inputs.iter().map(|s| support::Js::js(s.as_str())).collect::<Vec<_>>().join(", "));
    let script = format!("const re = {WHATWG};\nconsole.log({json}.map((s) => re.test(s)).join(\"\\n\"));\n");
    let output = Command::new("node").args(["-e", &script]).output().expect("run node");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let spec: Vec<bool> = String::from_utf8(output.stdout).expect("utf8").lines().map(|l| l == "true").collect();
    let differ: Vec<String> = inputs
        .iter()
        .zip(&spec)
        .filter(|(s, ok)| signup::Email::parse(s.to_string()).is_ok() != **ok)
        .map(|(s, ok)| format!("{s:?}: spec {ok}"))
        .collect();
    assert_eq!(spec.len(), inputs.len());
    assert!(differ.is_empty(), "differs from the WHATWG expression:\n{}", differ.join("\n"));
}
