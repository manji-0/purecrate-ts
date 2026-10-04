use crate::common::{assert_clean, assert_rejects};

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
fn tuples_nest() {
    assert_clean(&run("match ((d, n), o) { ((Dir::Up, _), _) => 1, _ => 0 }"));
}

#[test]
fn a_tuple_of_names_inside_a_variant_is_one_pattern() {
    assert_clean("pub fn f(s: &str) -> &str { match s.split_once('+') { Some((a, _)) => a, None => s } }");
    assert_clean(
        "pub fn f(s: &str) -> bool { match s.split_once('-') { Some((a, b)) if a.is_empty() => b.is_empty(), _ => false } }",
    );
    assert_clean(
        "pub fn f(o: Option<(i32, i32)>, n: i32) -> i32 { match (o, n) { (Some((a, b)), 0) => a + b, _ => n } }",
    );
    assert_clean("pub enum E { P((i32, i32)) } pub fn f(e: E) -> i32 { match e { E::P((a, b)) => a + b } }");
    assert_clean(
        "pub fn f(s: &str) -> &str { match s.split_once('+') { Some((\"a\", b)) => b, Some((a, _)) => a, None => s } }",
    );
    assert_clean("pub fn f(o: Option<(i32, (i32, i32))>) -> i32 { match o { Some((a, (b, 1))) => a + b, _ => 0 } }");
}

#[test]
fn elements_follow_the_arm_rules() {
    // An element may nest as an arm may (`nested_patterns_equivalence.rs`).
    assert_clean(&run("match (o, c) { (Some(0), Cmd::Move(1, y)) => y, (_, Cmd::Paint { color: 3 }) => 1, _ => 0 }"));
    assert_parse_rejects(&run("match (o, n) { (Some(x) | None, _) => 1, _ => 0 }"), "each side of `|`");
    // Guards on tuple arms are accepted (`guards_equivalence.rs`).
    assert_clean(&run("match (n, d) { (0, Dir::Up) if o.is_some() => 1, (m, _) if m > 3 => 2, _ => 0 }"));
}

#[test]
fn alternatives_of_tuples_bind_nothing() {
    assert_parse_rejects(&run("match (d, n) { (Dir::Up, m) | (Dir::Down, m) => m }"), "`|` arms may not bind names");
}

#[test]
fn tuple_patterns_need_a_tuple() {
    assert_rejects(&run("match n { (0, 1) => 1, _ => 0 }"), "does not match a value of type `i32`");
}

#[test]
fn patterns_nest_in_a_case() {
    assert_clean(&run(
        "match c { Cmd::Move(0, y) => y, Cmd::Move(x, 1..=9) => x, Cmd::Paint { color: 7 } => 7, _ => 0 }",
    ));
    assert_clean(&run("match o { Some(0) => 1, Some(m) if m > n => m, Some(_) => 2, None => 3 }"));
    assert_clean(&run("if matches!(c, Cmd::Move(_, 0)) { 1 } else { 0 }"));
    assert_clean(
        "pub enum Dir { Up, Down } pub fn f(r: Result<Option<Dir>, u8>) -> i32 { match r { Ok(Some(Dir::Up)) => 1, Ok(_) => 2, Err(0) => 3, Err(_) => 4 } }",
    );
    assert_rejects(&run("match o { _ => 0, Some(0) => 1 }"), "`_` must be the last arm");
}
