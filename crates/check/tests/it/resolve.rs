use crate::common::{assert_clean, assert_rejects};

const CMD: &str = "pub enum Cmd { Move(i32, i32), Paint { color: i32 }, Stop }\n\
                   pub struct Pos { pub x: i32, pub y: i32 }\n";

fn with(rest: &str) -> String {
    format!("{CMD}{rest}")
}

#[test]
fn well_formed_references_are_clean() {
    assert_clean(&with(
        "pub type Maybe = Option<Vec<Pos>>;\n\
         impl Pos { pub fn shift(self, d: i32) -> Pos { Pos { x: self.x + d, y: self.y } } }\n\
         fn dx(cmd: Cmd) -> i32 { match cmd { Cmd::Move(a, _) => a, Cmd::Paint { color } => color, Cmd::Stop => 0 } }\n\
         pub fn apply(p: Pos, cmd: Cmd) -> Result<Pos, Cmd> { let d = dx(cmd); Ok(Pos { x: p.x + d, y: p.y }) }",
    ));
}

#[test]
fn unknown_types_are_rejected_wherever_they_appear() {
    assert_rejects("pub struct S { pub t: Thing }", "type `Thing` is not defined");
    assert_rejects("pub fn f(x: Option<Vec<Thing>>) -> i32 { 0 }", "type `Thing` is not defined");
    assert_rejects("pub type T = (i32, Thing);", "type `Thing` is not defined");
    assert_rejects("pub enum E { A(Thing) }", "type `Thing` is not defined");
}

#[test]
fn calls_must_resolve_with_the_right_arity() {
    assert_rejects("pub fn f(n: i32) -> i32 { g(n) }", "function `g` is not defined");
    assert_rejects("pub fn f(n: i32) -> i32 { f(n, n) }", "`f` takes 1 argument(s), got 2");
    assert_rejects("pub fn f() -> i32 { i32::MAX }", "function `i32::MAX` is not defined");
    assert_rejects("pub fn f(n: i32) -> Option<i32> { Some(n, n) }", "`Some` takes 1");
}

#[test]
fn constructors_must_match_declared_fields() {
    assert_rejects(&with("pub fn f() -> Pos { Pos { x: 1 } }"), "`Pos` is missing field(s) y");
    assert_rejects(&with("pub fn f() -> Pos { Pos { x: 1, y: 2, z: 3 } }"), "`Pos` has no field(s) z");
    assert_rejects(&with("pub fn f() -> Cmd { Cmd::Move(1) }"), "`Cmd::Move` takes 2");
    assert_rejects(&with("pub fn f() -> Cmd { Cmd::Jump }"), "enum `Cmd` has no variant `Jump`");
    assert_rejects(&with("pub fn f() -> Cmd { Cmd::Paint { hue: 1 } }"), "missing field(s) color");
    assert_rejects(&with("pub fn f() -> Cmd { Cmd::Move }"), "`Cmd::Move` is constructed with the wrong shape");
}

#[test]
fn struct_update_fills_omitted_fields() {
    assert_clean(&with("pub fn f(p: Pos) -> Pos { Pos { x: p.x + 1, ..p } }"));
    assert_clean(&with("pub fn f(p: Pos) -> Pos { Pos { ..p } }"));
    assert_rejects(
        &with("pub fn f(p: Pos) -> Pos { Pos { x: 1, x: 2, ..p } }"),
        "field `x` of `Pos` is specified more than once",
    );
    assert_rejects(&with("pub fn f(p: Pos) -> Pos { Pos { z: 1, ..p } }"), "`Pos` has no field(s) z");
    assert_rejects(
        "pub struct Id(i32); pub fn f(id: Id) -> Id { Id { ..id } }",
        "struct update on newtype `Id` is not in v0",
    );
    assert_rejects(
        "pub struct A { pub n: i32 } pub struct B { pub n: i32 } pub fn f(b: B) -> A { A { ..b } }",
        "expected `A`, found `B`",
    );
}

#[test]
fn patterns_must_match_variant_fields() {
    let arms = |arm: &str| {
        with(&format!(
            "pub fn f(c: Cmd) -> i32 {{ match c {{ {arm}, Cmd::Paint {{ color }} => color, Cmd::Stop => 0 }} }}"
        ))
    };
    assert_rejects(&arms("Cmd::Move(a) => a"), "pattern `Cmd::Move` does not match");
    assert_rejects(
        &with(
            "pub fn f(c: Cmd) -> i32 { match c { Cmd::Paint { hue } => hue, Cmd::Move(a, _) => a, Cmd::Stop => 0 } }",
        ),
        "`Cmd::Paint` has no field `hue`",
    );
}

#[test]
fn variables_must_be_in_scope() {
    assert_rejects("pub fn f(n: i32) -> i32 { LIMIT }", "`LIMIT` is not a parameter or local binding");
    assert_rejects(
        &with("pub fn f(c: Cmd) -> i32 { let k = match c { Cmd::Move(a, _) => a, Cmd::Paint { color } => color, Cmd::Stop => 0 }; a }"),
        "`a` is not a parameter",
    );
    assert_clean("pub fn f(n: i32) -> i32 { let m = n; let k = m + 1; k }");
}

#[test]
fn impl_needs_a_crate_type() {
    assert_rejects("impl Ghost { pub fn f(self) -> i32 { 0 } }", "`impl Ghost` has no struct or enum");
}
