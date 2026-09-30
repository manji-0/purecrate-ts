
use crate::common::{assert_clean, assert_rejects, messages};

const DEFS: &str = "pub enum Cmd { Move(i32, i32), Paint { color: i32 }, Stop }\n\
                    pub enum Dir { Up, Down }\n";

fn run(body: &str) -> String {
    format!("{DEFS}pub fn f(c: Cmd, d: Dir) -> i32 {{ {body} }}")
}

#[test]
fn covering_every_variant_once_is_clean() {
    assert_clean(&run(
        "match c { Cmd::Stop => 0, Cmd::Paint { color } => color, Cmd::Move(a, _) => a }",
    ));
}

#[test]
fn missing_variants_are_listed() {
    assert_rejects(
        &run("match c { Cmd::Stop => 0 }"),
        "match on `Cmd` is missing `Cmd::Move`, `Cmd::Paint`",
    );
}

#[test]
fn duplicate_arms_are_rejected() {
    assert_rejects(
        &run("match d { Dir::Up => 1, Dir::Up => 2, Dir::Down => 3 }"),
        "`Dir::Up` is matched more than once",
    );
}

#[test]
fn arms_from_two_enums_are_rejected() {
    assert_rejects(
        &run("match d { Dir::Up => 1, Cmd::Stop => 2 }"),
        "match mixes cases of `Dir` and `Cmd`",
    );
}

#[test]
fn nested_matches_are_checked() {
    let src = run(
        "match c { Cmd::Stop => match d { Dir::Up => 1 }, Cmd::Paint { color } => color, Cmd::Move(a, _) => a }",
    );
    assert_eq!(messages(&src), vec!["match on `Dir` is missing `Dir::Down`".to_string()]);
}

#[test]
fn a_last_wildcard_takes_the_remaining_variants() {
    assert_clean(&run("match c { Cmd::Stop => 0, _ => 1 }"));
    assert_clean(&run("match c { Cmd::Stop | Cmd::Move(_, _) => 0, Cmd::Paint { .. } => 1 }"));
    assert_clean(&run("match c { Cmd::Stop | Cmd::Paint { .. } => 0, _ => 1 }"));
    assert_clean(&format!(
        "{DEFS}pub fn f(x: Option<i32>, r: Result<i32, i32>) -> i32 {{ \
         let a = match x {{ Some(v) => v, _ => 0 }}; \
         let b = match r {{ Err(e) => e, _ => 0 }}; a + b }}"
    ));
}

#[test]
fn misplaced_wildcards_are_rejected() {
    assert_rejects(&run("match c { _ => 1, Cmd::Stop => 0 }"), "`_` must be the last arm");
    assert_rejects(&run("match d { _ => 1 }"), "a match whose only arm is `_`");
    // As in rustc (a warning there), a `_` that takes nothing is accepted.
    assert_clean(&run("match d { Dir::Up => 1, Dir::Down => 2, _ => 3 }"));
    assert_clean(&run("match d { Dir::Up | Dir::Down => 1, _ => 3 }"));
    assert_rejects(&run("match d { Dir::Up | Dir::Up => 1, Dir::Down => 3 }"), "`Dir::Up` is matched more than once");
    assert_rejects(&run("match c { Cmd::Stop | Dir::Up => 1, _ => 3 }"), "match mixes cases of `Cmd` and `Dir`");
    assert_rejects(&run("match c { Cmd::Stop | Cmd::Move(_, _) => 0 }"), "match on `Cmd` is missing `Cmd::Paint`");
}

#[test]
fn integer_arms_end_in_a_wildcard() {
    assert_clean(&format!("{DEFS}pub fn g(b: u8) -> i32 {{ match b {{ b'0'..=b'9' | b'_' => 1, 200..255 => 2, _ => 3 }} }}"));
    assert_clean(&format!("{DEFS}pub fn g(x: i64) -> i32 {{ match x {{ -1 => 1, 0..=9 => 2, _ => 3 }} }}"));
    assert_rejects(&format!("{DEFS}pub fn g(b: u8) -> i32 {{ match b {{ 0..=255 => 1 }} }}"), "a match on integers must end in a `_` arm");
    assert_rejects(&format!("{DEFS}pub fn g(b: u8) -> i32 {{ match b {{ _ => 1, 0 => 2 }} }}"), "`_` must be the last arm");
    assert_rejects(&run("match c { Cmd::Stop => 0, 1 => 1, _ => 2 }"), "match mixes integer arms with other arms");
    assert_rejects(&format!("{DEFS}pub fn g(b: u8) -> i32 {{ match b {{ 1i32 => 1, _ => 2 }} }}"), "pattern `1i32` does not match a value of type `u8`");
    assert_rejects(&format!("{DEFS}pub fn g(s: bool) -> i32 {{ match s {{ 1 => 1, _ => 2 }} }}"), "integer patterns do not match a value of type `bool`");
}

#[test]
fn a_match_on_bool_names_both_or_ends_in_a_wildcard() {
    assert_clean("pub fn f(b: bool) -> u32 { match b { true => 1, false => 0 } }");
    assert_clean("pub fn f(b: bool) -> u32 { match b { false => 1, _ => 0 } }");
    assert_rejects("pub fn f(b: bool) -> u32 { match b { true => 1 } }", "match on `bool` is missing `false`");
    assert_rejects("pub fn f(x: u32) -> u32 { match x { true => 1, _ => 0 } }", "`bool` patterns do not match a value of type `u32`");
}

#[test]
fn a_guarded_arm_covers_nothing() {
    assert_rejects("pub fn f(o: Option<u32>) -> u32 { match o { Some(v) if v > 3 => 1, None => 0 } }", "match on `Option` is missing `Some`");
    assert_clean("pub fn f(o: Option<u32>) -> u32 { match o { Some(v) if v > 3 => 1, Some(_) => 2, None => 0 } }");
    assert_clean("pub fn f(n: u32) -> u32 { match n { m if m > 3 => 1, _ => 0 } }");
    assert_rejects(
        "fn g(x: u32) -> Option<u32> { Some(x) }\npub fn f(o: Option<u32>) -> Option<u32> { match o { Some(v) if g(v)? > 3 => Some(1), _ => Some(2) } }",
        "`?` or `return` inside a match guard is not in v0",
    );
}
