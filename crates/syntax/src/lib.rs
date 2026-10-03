//! Lower a crate's source files into a flattened PureCrate IR. The caller
//! supplies the crate name and the text of the root and every module file.

mod expr;
mod item;
mod modules;
mod std_ordering;
mod survey;
mod ty;

use purecrate_ir::{Crate, Item, Reason};
use syn::parse_file;

pub use item::{LineCol, ParseError};
pub use modules::{parse_files_spanned, Source};
pub use survey::{module_decls, module_decls_vis, survey_files, Unit, UnitKind};

/// Without source positions: bodies carry no `Expr::At`.
pub fn parse_source(crate_name: &str, source: &str) -> Result<Crate, ParseError> {
    parse_source_spanned(crate_name, source).map(|(mut krate, _)| {
        for item in &mut krate.items {
            if let Item::Fn(f) = item {
                f.body.strip_positions();
            }
        }
        krate
    })
}

/// Also returns where each item's name is, parallel to `Crate::items`.
/// Function bodies mark statements, block tails and `match` arms with
/// `Expr::At`, so diagnostics can point inside a function.
pub fn parse_source_spanned(
    crate_name: &str,
    source: &str,
) -> Result<(Crate, Vec<LineCol>), ParseError> {
    let mut file = parse_file(source).map_err(|e| ParseError::new(Reason::InvalidSyntax, e.to_string()).or_at(e.span()))?;
    if file.items.iter().any(|i| matches!(i, syn::Item::Mod(_)) && !item::is_test_only(i)) {
        // One file with modules: the module-aware path, which reads the
        // inline ones and reports an out-of-line one as missing.
        return parse_files_spanned(crate_name, &[Source { text: source, public: true }])
            .map(|(krate, spans)| (krate, spans.into_iter().map(|(_, at)| at).collect()))
            .map_err(|(_, e)| e);
    }
    let mut ordering = std_ordering::StdOrdering::default();
    ordering.rewrite(0, &mut file.items, &Default::default())?;
    let injected = ordering.injected().map_err(|(_, e)| e)?;
    let mut cx = item::Cx::scan(&file);
    cx.set_source(source);
    if injected.is_some() {
        cx.add_std_ordering();
    }
    let mut items: Vec<Item> = Vec::new();
    let mut spans: Vec<LineCol> = Vec::new();
    for syn_item in file.items.into_iter().filter(|i| !item::is_test_only(i)) {
        for (item, at) in item::lower_item(&mut cx, syn_item)? {
            items.push(item);
            spans.push(at);
        }
    }
    if let Some((vis, (_, at))) = injected {
        items.push(Item::Enum(purecrate_ir::Enum::std_ordering(vis)));
        spans.push(at);
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

        let (line, col, msg) = error_at("pub fn f(x: Rc<i32>) -> i32 { 0 }");
        assert_eq!((line, col), (1, 13), "{msg}");
        assert_eq!(msg, "`Rc` is not allowed in v0");

        let (_, _, msg) = error_at("pub fn f(x: std::fs::File) -> i32 { 0 }");
        assert!(msg.contains("qualified type path `std::fs::File`"), "{msg}");

        let (line, col, msg) = error_at(&with_arms("Cmd::Stop => 0,\n other => 1"));
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
        assert_eq!(reason(&body("x.into::<i32>()")), (Reason::MethodCall, Some("into".into())));
        assert_eq!(reason(&body("format!(\"{x}\"); x")), (Reason::Macro, Some("format".into())));
        assert_eq!(reason(&body("let v = vec![x; 2]; x")), (Reason::Macro, Some("vec".into())));
        assert_eq!(reason(&body("std::cmp::max(x, 1)")), (Reason::ExternalPath, Some("std::cmp::max".into())));
        assert_eq!(reason(&body("x as *const i32")).0, Reason::Cast);
        assert_eq!(reason("#[repr(C)]\npub enum E { A }").1, Some("repr".into()));
        assert_eq!(reason("pub enum E { A(i32) = 1, B }").1, Some("discriminant".into()));
        assert_eq!(reason(&body("loop {}")).0, Reason::Loop);
        assert_eq!(reason(&body("while let Some(y) = None::<i32> { let _ = y; } x")).0, Reason::Loop);
        assert_eq!(
            reason("pub enum E { A { n: i32 } } pub fn f(e: E) -> E { E::A { n: 1, ..e } }").0,
            Reason::StructUpdate
        );
        assert_eq!(
            reason("pub fn f(x: chrono::NaiveDate) -> i32 { 0 }"),
            (Reason::QualifiedPath, Some("chrono::NaiveDate".into()))
        );
        assert_eq!(reason("pub fn f(x: Rc<i32>) -> i32 { 0 }"), (Reason::DisallowedType, Some("Rc".into())));
        assert_eq!(reason("pub fn f(x: Mutex<i32>) -> i32 { 0 }"), (Reason::Mutex, Some("Mutex".into())));
        assert_eq!(reason("pub fn f(n: i32) -> i32 { Mutex::new(n); n }").0, Reason::Mutex);
        assert_eq!(reason("pub fn f(n: i32) -> i32 { Box::new(n, n) }").0, Reason::ConstructShape);
        assert_eq!(reason("pub trait T {}"), (Reason::UnsupportedItem, Some("trait".into())));
        assert_eq!(reason("pub enum Void {}"), (Reason::UnsupportedItem, Some("empty-enum".into())));
        assert_eq!(reason("pub fn f<T>(x: T) -> i32 { 0 }").0, Reason::Generics);
        assert_eq!(reason("pub fn f(x: isize) -> i32 { 0 }"), (Reason::DisallowedType, Some("isize".into())));
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
        assert_eq!(tags.params[1].ty, Ty::Prim(purecrate_ir::Prim::Str));
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
    fn doc_comments_are_kept_on_items_variants_and_fields() {
        use purecrate_ir::{Item, VariantFields};
        let src = "/// One line.\npub struct S {\n    /// The count.\n    ///\n    /// Never negative.\n    pub n: i32,\n    pub m: i32,\n}\n\
                   /** Block. */\npub enum E {\n    /// First.\n    A,\n    B { /// Inner.\n x: i32 },\n}\n\
                   impl S {\n    /// Doubles.\n    pub fn twice(self) -> i32 { self.n * 2 }\n}\n\
                   /// Ten.\npub const TEN: i32 = 10;\n/// An alias.\npub type Count = i32;\npub fn bare() -> i32 { 0 }\n";
        let krate = parse_source("d", src).expect("parse");
        let doc = |name: &str| krate.items.iter().find(|i| i.name().as_str() == name).and_then(|i| match i {
            Item::Struct(s) => s.doc.clone(),
            Item::Enum(e) => e.doc.clone(),
            Item::Fn(f) => f.doc.clone(),
            Item::Const(c) => c.doc.clone(),
            Item::Alias(a) => a.doc.clone(),
        });
        assert_eq!(doc("S").as_deref(), Some("One line."));
        assert_eq!(doc("E").as_deref(), Some("Block."));
        assert_eq!(doc("twice").as_deref(), Some("Doubles."));
        assert_eq!(doc("TEN").as_deref(), Some("Ten."));
        assert_eq!(doc("Count").as_deref(), Some("An alias."));
        assert_eq!(doc("bare"), None);
        let Some(Item::Struct(s)) = krate.items.iter().find(|i| i.name().as_str() == "S") else { panic!("S") };
        assert_eq!(s.fields[0].doc.as_deref(), Some("The count.\n\nNever negative."));
        assert_eq!(s.fields[1].doc, None);
        let Some(Item::Enum(e)) = krate.items.iter().find(|i| i.name().as_str() == "E") else { panic!("E") };
        assert_eq!(e.variants[0].doc.as_deref(), Some("First."));
        let VariantFields::Struct(fs) = &e.variants[1].fields else { panic!("B") };
        assert_eq!(fs[0].doc.as_deref(), Some("Inner."));
    }

    #[test]
    fn inline_modules_flatten_with_rusts_public_surface() {
        use purecrate_ir::Vis;
        let src = "mod a {\n    pub fn one() -> i32 { 1 }\n    pub fn two() -> i32 { 2 }\n}\n\
                   pub mod b {\n    pub fn three() -> i32 { super::a::one() }\n    fn four() -> i32 { 4 }\n}\n\
                   mod c {\n    pub fn five() -> i32 { 5 }\n}\n\
                   pub use a::one;\npub use c::*;\n";
        let krate = parse_source("m", src).expect("parse");
        let vis = |name: &str| krate.items.iter().find(|i| i.name().as_str() == name).map(|i| i.vis());
        assert_eq!(vis("one"), Some(Vis::Pub), "re-exported by name");
        assert_eq!(vis("two"), Some(Vis::Internal), "private module, not re-exported");
        assert_eq!(vis("three"), Some(Vis::Pub), "pub module");
        assert_eq!(vis("four"), Some(Vis::Internal));
        assert_eq!(vis("five"), Some(Vis::Pub), "glob re-export");

        let err = parse_source("m", "mod a {\n    pub fn one() -> i32 { 1 }\n}\npub use a::one as uno;\n").expect_err("rename");
        assert!(err.message.contains("renames an export"), "{}", err.message);
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
    fn wildcard_and_binding_free_or_arms_are_accepted() {
        parse_source("c", &with_arms("Cmd::Stop => 0, _ => 1")).expect("`_`");
        parse_source("c", &with_arms("Cmd::Stop | Cmd::Move(_, _) => 0")).expect("`|`");
    }

    #[test]
    fn unsupported_arm_patterns_are_rejected() {
        rejects("Cmd::Stop => 0, other => 1", "found binding `other`");
        rejects("Cmd::Stop => 0, Cmd::Move(a, _) | Cmd::Move(_, a) => a", "`|` arms may not bind names");
        rejects("Cmd::Stop | _ => 0", "each side of `|` must name an enum variant");
        parse_source("c", &with_arms("Cmd::Stop | Cmd::Move(1, _) => 0, _ => 1")).expect("`|` testing inside");
        parse_source("c", "pub fn f(s: &str) -> i32 { match s { \"a\" | \"b\" => 1, _ => 2 } }").expect("str pattern");
        let err = parse_source("c", "pub fn f(s: &str) -> i32 { match s { \"a\" | 1 => 1, _ => 2 } }").expect_err("mixed");
        assert!(err.message.contains("found a literal"), "{}", err.message);
        parse_source("c", "pub fn f(x: u8) -> bool { matches!(x, 1..=9 if x % 2 == 0) }").expect("guard");
        let err = parse_source("c", "pub fn f(x: u8) -> bool { matches!(x, _) }").expect_err("always true");
        assert!(err.message.contains("does not test `x`"), "{}", err.message);
        let err = parse_source("c", "pub fn f(x: i32) -> i32 { match x { 5.. => 1, _ => 2 } }").expect_err("half-open");
        assert!(err.message.contains("range patterns need a literal at both ends"), "{}", err.message);
        // Inside a case, a literal or a variant is tested further in.
        parse_source("c", "pub enum Dir { Up } pub enum Cmd { Move(i32, Dir), Stop } pub fn f(c: Cmd) -> i32 { match c { Cmd::Move(1, Dir::Up) => 1, _ => 0 } }")
            .expect("nested");
        parse_source("c", "pub enum Cmd { Move(i32, (i32, i32)), Stop } pub fn f(c: Cmd) -> i32 { match c { Cmd::Move(a, (_, 1)) => a, _ => 0 } }")
            .expect("tuple with a literal");
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
                        right: Box::new(Expr::Lit(Lit::Int { value: 1, ty: None, byte: false })),
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
