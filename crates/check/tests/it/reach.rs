use purecrate_check::prune_unreachable;
use purecrate_syntax::parse_source;

fn kept(source: &str) -> Vec<String> {
    let krate = parse_source("c", source).expect("parse");
    prune_unreachable(&krate)
        .items
        .iter()
        .map(|item| match item {
            purecrate_ir::Item::Fn(f) => match &f.owner {
                Some(o) => format!("{}.{}", o.as_str(), f.name.as_str()),
                None => f.name.as_str().to_string(),
            },
            other => other.name().as_str().to_string(),
        })
        .collect()
}

#[test]
fn counter_keeps_everything() {
    let counter = include_str!("../../../../examples/counter/src/lib.rs");
    assert_eq!(kept(counter), ["Event", "State", "step"]);
}

#[test]
fn internal_items_are_kept_only_when_reached() {
    let src = "pub struct S { pub n: i32 }\n\
               struct Inner { pub k: Tag }\n\
               enum Tag { A }\n\
               struct Unused { pub n: i32 }\n\
               fn helper(s: S) -> i32 { s.n }\n\
               fn dead(s: S) -> i32 { s.n }\n\
               fn wrap(n: i32) -> Inner { Inner { k: Tag::A } }\n\
               pub fn run(s: S) -> i32 { let i = wrap(1); helper(s) }";
    assert_eq!(kept(src), ["S", "Inner", "Tag", "helper", "wrap", "run"]);
}

#[test]
fn methods_follow_their_owner() {
    let src = "pub struct S { pub n: i32 }\n\
               struct Hidden { pub n: i32 }\n\
               impl S { pub fn bump(self) -> S { S { n: self.n + 1 } } fn secret(self) -> S { self } }\n\
               impl Hidden { pub fn show(self) -> i32 { self.n } }";
    assert_eq!(kept(src), ["S", "S.bump"]);
}

#[test]
fn a_kept_method_pulls_in_what_it_uses() {
    let src = "pub struct S { pub n: i32 }\n\
               fn twice(n: i32) -> i32 { n + n }\n\
               impl S { pub fn grow(self) -> S { S { n: twice(self.n) } } }";
    assert_eq!(kept(src), ["S", "twice", "S.grow"]);
}
