mod common;

use common::{assert_clean, assert_rejects};

const DEFS: &str = "pub enum Cmd { Move(i32, i32), Paint { color: i32 }, Stop }\n\
                    pub enum Dir { Up, Down }\n";

/// Rejected by the parser, before any check runs.
fn assert_parse_rejects(source: &str, needle: &str) {
    let err = purecrate_syntax::parse_source("c", source).expect_err("rejected");
    assert!(err.message.contains(needle), "expected {needle:?}, got {:?}", err.message);
}

fn run(body: &str) -> String {
    format!("{DEFS}pub fn f(c: Cmd, d: Dir, n: i32, o: Option<i32>) -> i32 {{ {body} }}")
}

#[test]
fn tuple_arms_are_accepted() {
    assert_clean(&run(
        "match (c, d) { (Cmd::Move(x, _), Dir::Up) => x, (Cmd::Paint { color }, _) => color, (_, Dir::Down) => 1, (Cmd::Stop | Cmd::Move(_, _), _) => 0 }",
    ));
    assert_clean(&run("match (n, o) { (0, Some(v)) => v, (1..=9, None) => 1, (m, _) => m }"));
    assert_clean(&run("match (d, n) { (Dir::Up, 0) | (Dir::Down, 1) => 1, _ => 0 }"));
    assert_clean(&run("if matches!((d, o), (Dir::Up, None)) { 1 } else { 0 }"));
}

#[test]
fn tuples_do_not_nest() {
    assert_parse_rejects(&run("match ((d, n), o) { ((Dir::Up, _), _) => 1, _ => 0 }"), "tuple patterns may not nest");
}

#[test]
fn elements_follow_the_arm_rules() {
    assert_parse_rejects(&run("match (o, n) { (Some(Some(_)), _) => 1, _ => 0 }"), "variant fields may only bind names");
    assert_parse_rejects(&run("match (n, n) { (0, 0) if true => 1, _ => 0 }"), "match guards");
}

#[test]
fn alternatives_of_tuples_bind_nothing() {
    assert_parse_rejects(
        &run("match (d, n) { (Dir::Up, m) | (Dir::Down, m) => m }"),
        "`|` arms may not bind names",
    );
}

#[test]
fn tuple_patterns_need_a_tuple() {
    assert_rejects(&run("match n { (0, 1) => 1, _ => 0 }"), "does not match a value of type `i32`");
}
