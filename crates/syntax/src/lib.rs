//! Lower a single Rust source file into a flattened PureCrate IR.
//! The caller supplies the crate name and the source text.

mod expr;
mod item;
mod survey;
mod ty;

use purecrate_ir::{Crate, Item, Reason};
use syn::parse_file;

pub use item::{LineCol, ParseError};
pub use survey::{module_decls, survey_files, Unit, UnitKind};

pub fn parse_source(crate_name: &str, source: &str) -> Result<Crate, ParseError> {
    parse_source_spanned(crate_name, source).map(|(krate, _)| krate)
}

/// Also returns where each item's name is, parallel to `Crate::items`.
pub fn parse_source_spanned(
    crate_name: &str,
    source: &str,
) -> Result<(Crate, Vec<LineCol>), ParseError> {
    let file = parse_file(source).map_err(|e| ParseError::new(Reason::InvalidSyntax, e.to_string()).or_at(e.span()))?;
    let mut cx = item::Cx::scan(&file);
    let mut items: Vec<Item> = Vec::new();
    let mut spans: Vec<LineCol> = Vec::new();
    for syn_item in file.items.into_iter().filter(|i| !item::is_test_only(i)) {
        for (item, at) in item::lower_item(&mut cx, syn_item)? {
            items.push(item);
            spans.push(at);
        }
    }
    Ok((Crate::new(crate_name, items), spans))
}

#[cfg(test)]
mod tests {
    use super::*;
    use purecrate_ir::counter_example;

    const COUNTER: &str = include_str!("../../../examples/counter/src/lib.rs");

    #[test]
    fn counter_matches_handwritten_ir() {
        let parsed = parse_source("counter", COUNTER).expect("parse");
        assert_eq!(parsed, counter_example());
    }

    fn with_arms(arms: &str) -> String {
        format!(
            "pub enum Cmd {{ Move(i32, i32), Stop }}
             pub enum Dir {{ Up }}
             pub fn run(cmd: Cmd) -> i32 {{ match cmd {{ {arms} }} }}"
        )
    }

    fn rejects(arms: &str, needle: &str) {
        let err = parse_source("c", &with_arms(arms)).expect_err(arms);
        assert!(err.message.contains(needle), "{arms}: {}", err.message);
    }

    #[test]
    fn variant_arms_with_name_bindings_are_accepted() {
        parse_source("c", &with_arms("Cmd::Move(a, _) => a, Cmd::Stop => 0")).expect("parse");
    }

    fn error_at(source: &str) -> (usize, usize, String) {
        let err = parse_source("c", source).expect_err(source);
        let at = err.at.unwrap_or_else(|| panic!("no location: {}", err.message));
        (at.line, at.col, err.message)
    }

    #[test]
    fn errors_point_at_the_offending_node() {
        let src = "pub struct S { pub n: i32 }\n\
                   pub fn f(s: S) -> i32 {\n    let r = &mut s;\n    0\n}\n";
        let (line, col, msg) = error_at(src);
        assert_eq!((line, col), (3, 13), "{msg}");

        assert_eq!(error_at(src).2, "`&mut` borrows are not in v0: `&mut s`");

        let (line, col, msg) = error_at("pub fn f(x: Box<i32>) -> i32 { 0 }");
        assert_eq!((line, col), (1, 13), "{msg}");
        assert_eq!(msg, "`Box` is not allowed in v0");

        let (_, _, msg) = error_at("pub fn f(x: std::fs::File) -> i32 { 0 }");
        assert!(msg.contains("qualified type path `std::fs::File`"), "{msg}");

        let (line, col, msg) = error_at(&with_arms("Cmd::Stop => 0,\n _ => 1"));
        assert_eq!(line, 4, "{msg}");
        assert_eq!(col, 2, "{msg}");

        let (line, _, msg) = error_at("pub fn f( -> i32 { 0 }");
        assert_eq!(line, 1, "{msg}");
    }

    #[test]
    fn rejections_carry_a_reason_and_what_it_is_about() {
        use purecrate_ir::Reason;
        let reason = |src: &str| {
            let e = parse_source("c", src).expect_err(src);
            (e.reason, e.detail)
        };
        let body = |b: &str| format!("pub fn f(x: i32) -> i32 {{ {b} }}");
        assert_eq!(reason(&body("x.parse::<i32>()")), (Reason::MethodCall, Some("parse".into())));
        assert_eq!(reason(&body("format!(\"{x}\"); x")), (Reason::Macro, Some("format".into())));
        assert_eq!(reason(&body("std::cmp::max(x, 1)")), (Reason::ExternalPath, Some("std::cmp::max".into())));
        assert_eq!(reason(&body("x as i32")).0, Reason::Cast);
        assert_eq!(reason(&body("for i in 0..x {} x")).0, Reason::Loop);
        assert_eq!(
            reason("pub fn f(x: chrono::NaiveDate) -> i32 { 0 }"),
            (Reason::QualifiedPath, Some("chrono::NaiveDate".into()))
        );
        assert_eq!(reason("pub fn f(x: Box<i32>) -> i32 { 0 }"), (Reason::DisallowedType, Some("Box".into())));
        assert_eq!(reason("pub trait T {}"), (Reason::UnsupportedItem, Some("trait".into())));
        assert_eq!(reason("pub fn f<T>(x: T) -> i32 { 0 }").0, Reason::Generics);
        assert_eq!(reason("pub fn f(x: usize) -> i32 { 0 }"), (Reason::DisallowedType, Some("usize".into())));
        assert_eq!(reason("pub fn f() -> Self { 0 }").0, Reason::SelfType);
        assert_eq!(reason("pub fn f(x: &mut i32) -> i32 { 0 }").0, Reason::RefType);
        assert_eq!(reason("pub struct S { pub n: i32 } impl S { pub fn f(&mut self) {} }").0, Reason::RefReceiver);
        assert_eq!(reason("pub struct P(i32, i32);").0, Reason::TupleStruct);
        assert_eq!(reason("pub fn f(x: (i32, i32)) -> i32 { x.1 }").0, Reason::TupleField);
    }

    #[test]
    fn shared_references_self_and_newtypes_lower_to_values() {
        use purecrate_ir::{Callee, Expr, Item, Name, Ty};
        let krate = parse_source(
            "c",
            "pub struct Id(u32);
             impl Id {
                 pub fn new(n: &u32) -> Self { Self(*n) }
                 pub fn get(&self) -> u32 { self.0 }
             }
             pub fn tags(ids: &[Id], name: &str) -> String { let _x = &ids; name }",
        )
        .expect("parse")
        .items;
        let Item::Struct(id) = &krate[0] else { panic!("struct") };
        assert_eq!(id.newtype_inner(), Some(&Ty::Prim(purecrate_ir::Prim::U32)));
        let Item::Fn(new) = &krate[1] else { panic!("fn") };
        assert_eq!(new.ret, Ty::Named(Name::new("Id")));
        assert_eq!(
            new.body,
            Expr::Call { callee: Callee::StructNew(Name::new("Id")), args: vec![Expr::var("n")] }
        );
        let Item::Fn(get) = &krate[2] else { panic!("fn") };
        assert_eq!(get.params[0].ty, Ty::Named(Name::new("Id")));
        assert_eq!(get.body, Expr::Field { base: Box::new(Expr::var("self")), name: Name::new("0") });
        let Item::Fn(tags) = &krate[3] else { panic!("fn") };
        assert_eq!(tags.params[0].ty, Ty::Vec(Box::new(Ty::Named(Name::new("Id")))));
        assert_eq!(tags.params[1].ty, Ty::Prim(purecrate_ir::Prim::String));
    }

    #[test]
    fn methods_get_the_same_signature_checks_as_free_fns() {
        let src = "pub struct S { pub n: i32 }\n\
                   impl S {\n    pub async fn f(self) -> S { self }\n}\n";
        let (line, col, msg) = error_at(src);
        assert!(msg.contains("async"), "{msg}");
        assert_eq!((line, col), (3, 9));

        let (_, _, msg) = error_at("pub fn f(self) -> i32 { 0 }");
        assert!(msg.contains("outside an impl"), "{msg}");
    }

    #[test]
    fn attributes_that_change_the_build_are_rejected_in_place() {
        let (line, col, msg) = error_at(
            "#[derive(Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct S { pub total_count: i64 }",
        );
        assert_eq!((line, col), (2, 1), "{msg}");
        assert!(msg.contains("`#[serde(...)]` changes the JSON shape"), "{msg}");

        let (line, col, msg) = error_at("pub struct S {\n    #[serde(rename = \"n\")]\n    pub count: i64,\n}");
        assert_eq!((line, col), (2, 5), "{msg}");

        let (line, _, msg) = error_at("pub enum E {\n    #[serde(rename = \"a\")]\n    A,\n}");
        assert_eq!(line, 2, "{msg}");

        let (_, _, msg) = error_at("#[cfg(feature = \"x\")]\npub fn f() -> i32 { 0 }");
        assert!(msg.contains("conditional compilation"), "{msg}");

        let (_, _, msg) = error_at("#[cfg_attr(feature = \"x\", serde(tag = \"t\"))]\npub struct S { pub n: i32 }");
        assert!(msg.contains("conditional compilation"), "{msg}");
    }

    #[test]
    fn inert_attributes_and_test_modules_are_accepted() {
        let src = "/// Doc.\n#[derive(Debug, Clone, Serialize, Deserialize)]\n#[allow(dead_code)]\n\
                   pub struct S { #[doc = \"n\"] pub n: i32 }\n\
                   #[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";
        let krate = parse_source("c", src).expect("parse");
        assert_eq!(krate.items.len(), 1);
    }

    #[test]
    fn if_let_becomes_a_two_arm_match() {
        use purecrate_ir::{Expr, Item, Pattern};
        let krate = parse_source(
            "c",
            "pub fn f(x: Result<i32, i32>) -> i32 { if let Ok(v) = x { v } else { 0 } }",
        )
        .expect("parse");
        let Item::Fn(f) = &krate.items[0] else { panic!("fn") };
        let Expr::Match { arms, .. } = &f.body else {
            panic!("match, got {:?}", f.body)
        };
        let patterns: Vec<&Pattern> = arms.iter().map(|a| &a.pattern).collect();
        assert_eq!(
            patterns,
            [
                &Pattern::ResultOk(Box::new(Pattern::Var(purecrate_ir::Name::new("v")))),
                &Pattern::ResultErr(Box::new(Pattern::Wildcard)),
            ]
        );

        let (_, _, msg) = error_at(&with_arms("Cmd::Stop => 0, Cmd::Move(a, _) => a") .replace(
            "match cmd { Cmd::Stop => 0, Cmd::Move(a, _) => a }",
            "if let Cmd::Stop = cmd { 0 } else { 1 }",
        ));
        assert!(msg.contains("`if let` on an enum variant is not in v0"), "{msg}");
    }

    #[test]
    fn unsupported_arm_patterns_are_rejected() {
        rejects("Cmd::Stop => 0, _ => 1", "found `_`");
        rejects("Cmd::Stop => 0, other => 1", "found binding `other`");
        rejects("Cmd::Move(1, b) => b, Cmd::Stop => 0", "found a literal");
        rejects(
            "Cmd::Move(a, Dir::Up) => a, Cmd::Stop => 0",
            "found nested variant `Dir::Up`",
        );
    }

    #[test]
    fn statements_become_seq_and_compound_assignment_expands() {
        use purecrate_ir::{BinOp, Expr, Item, Lit, Name};
        let krate = parse_source("c", "pub fn f(a: i32) -> i32 { let mut x = a; x += 1; x }").expect("parse");
        let Item::Fn(f) = &krate.items[0] else { panic!("fn") };
        let Expr::Let { mutable: true, then, .. } = &f.body else {
            panic!("let mut, got {:?}", f.body)
        };
        let x = || Box::new(Expr::var("x"));
        assert_eq!(
            **then,
            Expr::Seq {
                first: Box::new(Expr::Assign {
                    name: Name::new("x"),
                    value: Box::new(Expr::Binary {
                        op: BinOp::Add,
                        left: x(),
                        right: Box::new(Expr::Lit(Lit::Int { value: 1, ty: None })),
                    }),
                }),
                then: x(),
            }
        );
    }

    #[test]
    fn mut_parameters_and_field_assignment_are_rejected() {
        let err = parse_source("c", "pub fn f(mut a: i32) -> i32 { a }").expect_err("mut param");
        assert!(err.message.contains("write `let mut a = a;`"), "{}", err.message);
        let err = parse_source(
            "c",
            "pub struct S { pub n: i32 }
             pub fn f(s: S) -> i32 { let mut t = s; t.n = 1; t.n }",
        )
        .expect_err("field assignment");
        assert!(err.message.contains("assigning to a field is not in v0"), "{}", err.message);
    }
}
