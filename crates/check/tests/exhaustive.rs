mod common;

use common::{assert_clean, assert_rejects, messages};

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
        "match mixes variants of `Dir` and `Cmd`",
    );
}

#[test]
fn nested_matches_are_checked() {
    let src = run(
        "match c { Cmd::Stop => match d { Dir::Up => 1 }, Cmd::Paint { color } => color, Cmd::Move(a, _) => a }",
    );
    assert_eq!(messages(&src), vec!["match on `Dir` is missing `Dir::Down`".to_string()]);
}
