use super::*;
use purecrate_ir::counter_example;

fn file<'a>(pkg: &'a Package, stem: &str) -> &'a str {
    &pkg.files.iter().find(|f| f.stem == stem).unwrap_or_else(|| panic!("missing {stem}")).source
}

#[test]
fn erased_wrappers_are_named_in_types_only() {
    use purecrate_ir::{Fn, Item, Param, Vis, Wrapper};
    let share = Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("share"),
        owner: None,
        params: vec![Param { name: Name::new("n"), ty: Ty::ignored(Wrapper::Mutex, Ty::i32()) }],
        ret: Ty::ignored(Wrapper::Box, Ty::i32()),
        body: Expr::Ignored { wrapper: Wrapper::Arc, expr: Box::new(Expr::var("n")) },
        doc: None,
    });
    let pkg = emit(&Crate::new("wraps", vec![share]));
    let src = file(&pkg, "share");
    assert!(src.contains("(n: /* Mutex */ I32): /* Box */ I32 => n;"), "{src}");
}

#[test]
fn doc_comments_become_jsdoc() {
    let mut krate = counter_example();
    for item in &mut krate.items {
        match item {
            Item::Enum(e) => {
                e.doc = Some("What happens.\n\nOne at a time; ends in */ here.".into());
                e.variants[0].doc = Some("Up by one.".into());
            }
            Item::Struct(s) => s.fields[0].doc = Some("The count.".into()),
            Item::Fn(f) => f.doc = Some("The transition.".into()),
            _ => {}
        }
    }
    let pkg = emit(&krate);
    assert!(
        file(&pkg, "event")
            .contains("/**\n * What happens.\n *\n * One at a time; ends in *\\/ here.\n */\nexport type Event =\n"),
        "{}",
        file(&pkg, "event")
    );
    assert!(file(&pkg, "event").contains("  /** Up by one. */\n  Inc: "), "{}", file(&pkg, "event"));
    assert!(file(&pkg, "state").contains("Readonly<{\n  /** The count. */\n  n: I32;\n}>"), "{}", file(&pkg, "state"));
    assert!(file(&pkg, "step").contains("/** The transition. */\nexport const step = "), "{}", file(&pkg, "step"));
}

#[test]
fn counter_kamae_shape() {
    let pkg = emit(&counter_example());
    let stems: Vec<&str> = pkg.files.iter().map(|f| f.stem.as_str()).collect();
    assert!(stems.contains(&"event"));
    assert!(stems.contains(&"state"));
    assert!(stems.contains(&"step"));

    let event = file(&pkg, "event");
    assert!(event.contains("kind: \"Inc\""));
    assert!(event.contains("export const Event = {"));
    assert!(!event.contains("tag:"));

    let state = file(&pkg, "state");
    assert!(state.contains("export type State = Readonly<{"));
    assert!(state.contains("of: (n: I32): State =>"));

    let step = file(&pkg, "step");
    assert!(step.contains("export const step ="));
    assert!(step.contains("switch (event.kind)"));
    assert!(step.contains("assertNever(event)"));
    assert!(step.contains("import type { Event }"));
    assert!(step.contains("import type { State }"));
}

#[test]
fn counter_step_is_a_switch_statement() {
    let pkg = emit(&counter_example());
    let expected = "\
export const step = (state: State, event: Event): State => {
  switch (event.kind) {
    case \"Inc\":
      return { n: state.n + 1 };
    case \"Dec\":
      return { n: state.n - 1 };
    case \"Reset\":
      return { n: 0 };
    default:
      return assertNever(event);
  }
};
";
    assert!(file(&pkg, "step").ends_with(expected), "{}", file(&pkg, "step"));
}

fn cmd_crate(body: Expr) -> Crate {
    use purecrate_ir::{Name, Param, Variant, Vis};
    let cmd = Item::Enum(Enum {
        vis: Vis::Pub,
        name: Name::new("Cmd"),
        variants: vec![
            Variant {
                name: Name::new("Move"),
                fields: VariantFields::Tuple(vec![Ty::i32(), Ty::i32()]),
                discriminant: None,
                doc: None,
            },
            Variant {
                name: Name::new("Paint"),
                fields: VariantFields::Struct(vec![purecrate_ir::Field {
                    name: Name::new("color"),
                    ty: Ty::i32(),
                    doc: None,
                }]),
                discriminant: None,
                doc: None,
            },
        ],
        repr: None,
        std: false,
        serde: purecrate_ir::Serde::default(),
        doc: None,
    });
    let run = Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("run"),
        owner: None,
        params: vec![Param { name: Name::new("cmd"), ty: Ty::named("Cmd") }],
        ret: Ty::i32(),
        body,
        doc: None,
    });
    Crate::new("cmds", vec![cmd, run])
}

fn cmd_match(scrutinee: Expr) -> Expr {
    use purecrate_ir::{Arm, Name};
    let var = |n: &str| Pattern::Var(Name::new(n));
    Expr::Match {
        scrutinee: Box::new(scrutinee),
        arms: vec![
            Arm {
                guard: None,
                pattern: Pattern::Variant {
                    ty: Name::new("Cmd"),
                    variant: Name::new("Move"),
                    bind: VariantBind::Tuple(vec![var("a"), var("b")]),
                },
                body: Expr::Binary { op: BinOp::Add, left: Box::new(Expr::var("a")), right: Box::new(Expr::var("b")) },
            },
            Arm {
                guard: None,
                pattern: Pattern::Variant {
                    ty: Name::new("Cmd"),
                    variant: Name::new("Paint"),
                    bind: VariantBind::Struct(vec![(Name::new("color"), var("c"))]),
                },
                body: Expr::var("c"),
            },
        ],
    }
}

#[test]
fn bindings_read_from_the_scrutinee() {
    let pkg = emit(&cmd_crate(cmd_match(Expr::var("cmd"))));
    let run = file(&pkg, "run");
    assert!(run.contains("case \"Move\": {\n      const a = cmd.content[0];"), "{run}");
    assert!(run.contains("const b = cmd.content[1];"), "{run}");
    assert!(run.contains("const c = cmd.color;"), "{run}");
    assert!(!run.contains("event"), "{run}");
}

#[test]
fn non_place_scrutinee_is_bound_once() {
    use purecrate_ir::{Callee, Name};
    let call = Expr::Call { callee: Callee::Fn(Name::new("next")), args: vec![Expr::var("cmd")] };
    let pkg = emit(&cmd_crate(cmd_match(call)));
    let run = file(&pkg, "run");
    // Named for its enum, numbered past the parameter of that name.
    assert!(run.contains("const cmd2 = next(cmd);\n  switch (cmd2.kind)"), "{run}");
    assert!(run.contains("const a = cmd2.content[0];"), "{run}");
}

#[test]
fn object_literal_arrow_body_is_parenthesized() {
    use purecrate_ir::{Name, Param, Vis};
    let mut krate = counter_example();
    krate.items.push(Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("zero"),
        owner: Some(Name::new("State")),
        params: vec![Param { name: Name::new("self"), ty: Ty::named("State") }],
        ret: Ty::named("State"),
        body: Expr::Construct {
            ty: Name::new("State"),
            variant: None,
            fields: Fields::Named(vec![(Name::new("n"), Expr::int(0))]),
            base: None,
        },
        doc: None,
    }));
    let pkg = emit(&krate);
    assert!(file(&pkg, "state").contains("zero: (_self: State): State => ({ n: 0 }),"), "{}", file(&pkg, "state"));
}

#[test]
fn a_helper_with_one_user_file_is_printed_there() {
    use purecrate_ir::{Param, Vis};
    let helper = |name: &str, body: Expr| {
        Item::Fn(Fn {
            vis: Vis::Internal,
            name: Name::new(name),
            owner: None,
            params: vec![Param { name: Name::new("cmd"), ty: Ty::named("Cmd") }],
            ret: Ty::named("Cmd"),
            body,
            doc: None,
        })
    };
    let call = |f: &str| Expr::Call { callee: Callee::Fn(Name::new(f)), args: vec![Expr::var("cmd")] };
    let mut krate = cmd_crate(cmd_match(call("next")));
    // `next` calls `inner`; both land in `run.ts`, `inner` by following `next`.
    krate.items.push(helper("next", call("inner")));
    krate.items.push(helper("inner", Expr::var("cmd")));
    let pkg = emit(&krate);
    assert!(pkg.files.iter().all(|f| f.stem != "next" && f.stem != "inner"));
    let run = file(&pkg, "run");
    assert!(run.contains("\nconst next = (cmd: Cmd): Cmd => inner(cmd);\n"), "{run}");
    assert!(run.contains("\nconst inner = (cmd: Cmd): Cmd => cmd;\n"), "{run}");
    assert!(!run.contains("import { next }") && !run.contains("import { inner }"), "{run}");

    // Called from two files, it keeps its own and is exported from it.
    let mut krate = cmd_crate(cmd_match(call("next")));
    krate.items.push(helper("next", Expr::var("cmd")));
    krate.items.push(helper("again", call("next")));
    let pkg = emit(&krate);
    assert!(file(&pkg, "next").contains("export const next = "));
}

#[test]
fn imports_follow_body_and_nested_types() {
    use purecrate_ir::{Param, Vis};
    let mut krate =
        cmd_crate(cmd_match(Expr::Call { callee: Callee::Fn(Name::new("next")), args: vec![Expr::var("cmd")] }));
    // `pub`, so it keeps its file (`homes`).
    krate.items.push(Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("next"),
        owner: None,
        params: vec![Param { name: Name::new("cmd"), ty: Ty::named("Cmd") }],
        ret: Ty::named("Cmd"),
        body: Expr::var("cmd"),
        doc: None,
    }));
    krate.items.push(Item::Struct(Struct {
        vis: Vis::Pub,
        name: Name::new("Log"),
        fields: vec![purecrate_ir::Field {
            name: Name::new("last"),
            ty: Ty::option(Ty::Vec(Box::new(Ty::named("Cmd")))),
            doc: None,
        }],
        closed: false,
        wire_from: None,
        serde: purecrate_ir::Serde::default(),
        doc: None,
    }));
    let pkg = emit(&krate);

    let run = file(&pkg, "run");
    assert!(run.contains("import { next } from \"./next.ts\";\n"), "{run}");
    assert!(run.contains("import type { Cmd } from \"./cmd.ts\";\n"), "{run}");

    let log = file(&pkg, "log");
    assert!(log.contains("import type { Cmd } from \"./cmd.ts\";\n"), "{log}");
    let cmd = file(&pkg, "cmd");
    assert!(cmd.contains("import type { I32 } from \"purecrate\";\n"), "{cmd}");
    assert!(!cmd.contains("from \"./cmd.ts\""), "{cmd}");
}

#[test]
fn enum_methods_join_the_companion() {
    use purecrate_ir::{Param, Vis};
    let mut krate = cmd_crate(Expr::int(0));
    krate.items.push(Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("weight"),
        owner: Some(Name::new("Cmd")),
        params: vec![Param { name: Name::new("self"), ty: Ty::named("Cmd") }],
        ret: Ty::i32(),
        body: cmd_match(Expr::var("self")),
        doc: None,
    }));
    let pkg = emit(&krate);
    let cmd = file(&pkg, "cmd");
    assert!(cmd.contains("  weight: (self: Cmd): I32 => {\n    switch (self.kind) {"), "{cmd}");
    assert!(cmd.ends_with("  },\n} as const;\n"), "{cmd}");
    assert!(cmd.starts_with(&format!("{HEADER}\nimport {{ assertNever, type I32 }}")), "{cmd}");
}

#[test]
fn index_exports_each_name_once() {
    let pkg = emit(&counter_example());
    let index = file(&pkg, "index");
    for name in ["Panic", "assertNever", "Int", "I32", "Event", "State", "step"] {
        // The names, not the paths they come from.
        let hits = index
            .lines()
            .filter_map(|l| l.split(" from ").next())
            .flat_map(|l| l.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')))
            .filter(|w| *w == name)
            .count();
        assert_eq!(hits, 1, "{name} in:\n{index}");
    }
    assert_eq!(index.lines().filter(|l| l.contains("export type {")).count(), 1, "{index}");
}

#[test]
fn result_is_imported_once() {
    use purecrate_ir::{Callee, Name, Param, Vis};
    let parse = Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("parse"),
        owner: None,
        params: vec![Param { name: Name::new("n"), ty: Ty::i32() }],
        ret: Ty::result(Ty::i32(), Ty::Prim(purecrate_ir::Prim::String)),
        body: Expr::Call { callee: Callee::ResultOk, args: vec![Expr::var("n")] },
        doc: None,
    });
    let pkg = emit(&Crate::new("p", vec![parse]));
    let src = file(&pkg, "parse");
    assert_eq!(src.matches("import ").count(), 1, "{src}");
    assert!(src.contains("import { Result, type I32 } from \"purecrate\";"), "{src}");
}

fn f64_fn(body: Expr) -> String {
    use purecrate_ir::{Name, Param, Prim, Vis};
    let f = Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("f"),
        owner: None,
        params: vec![Param { name: Name::new("x"), ty: Ty::Prim(Prim::F64) }],
        ret: Ty::Prim(Prim::F64),
        body,
        doc: None,
    });
    file(&emit(&Crate::new("p", vec![f])), "f").to_string()
}

#[test]
fn nested_operators_keep_their_grouping() {
    use purecrate_ir::UnOp;
    let bin = |op, l, r| Expr::Binary { op, left: Box::new(l), right: Box::new(r) };
    let sum = bin(BinOp::Add, Expr::var("x"), Expr::var("x"));
    let src = f64_fn(bin(BinOp::Mul, sum.clone(), Expr::var("x")));
    assert!(src.contains("=> (x + x) * x;"), "{src}");
    let neg = |e| Expr::Unary { op: UnOp::Neg, expr: Box::new(e) };
    let src = f64_fn(neg(neg(Expr::var("x"))));
    assert!(src.contains("=> x * -1 * -1;"), "{src}");
    let src = f64_fn(neg(sum.clone()));
    assert!(src.contains("=> (x + x) * -1;"), "{src}");
    let src = f64_fn(bin(BinOp::Add, sum, Expr::var("x")));
    assert!(src.contains("=> x + x + x;"), "{src}");
    let src = f64_fn(bin(BinOp::Sub, Expr::var("x"), bin(BinOp::Sub, Expr::var("x"), Expr::var("x"))));
    assert!(src.contains("=> x - (x - x);"), "{src}");
}

fn bool_fn(body: Expr) -> String {
    use purecrate_ir::{Name, Param, Prim, Vis};
    let f = Item::Fn(Fn {
        vis: Vis::Pub,
        name: Name::new("f"),
        owner: None,
        params: vec![
            Param { name: Name::new("a"), ty: Ty::Prim(Prim::Bool) },
            Param { name: Name::new("b"), ty: Ty::Prim(Prim::Bool) },
            Param { name: Name::new("c"), ty: Ty::Prim(Prim::Bool) },
        ],
        ret: Ty::Prim(Prim::Bool),
        body,
        doc: None,
    });
    file(&emit(&Crate::new("p", vec![f])), "f").to_string()
}

#[test]
fn boolean_operators_follow_precedence() {
    let bin = |op, l, r| Expr::Binary { op, left: Box::new(l), right: Box::new(r) };
    let and = bin(BinOp::And, Expr::var("a"), Expr::var("b"));
    let src = bool_fn(bin(BinOp::Or, and, Expr::var("c")));
    // Not needed for precedence, but read as oxfmt writes it.
    assert!(src.contains("=> (a && b) || c;"), "{src}");
    let or = bin(BinOp::Or, Expr::var("a"), Expr::var("b"));
    let src = bool_fn(bin(BinOp::And, or, Expr::var("c")));
    assert!(src.contains("=> (a || b) && c;"), "{src}");
}

#[test]
fn typed_literals_print_their_js_form() {
    let big = Lit::Int { value: -5, ty: Some(IntTy::I64), byte: false, hex: false };
    assert_eq!(emit_lit(&big), "(-5n as I64)");
    let single = Lit::Float { digits: "0.1".into(), ty: Some(FloatTy::F32) };
    assert_eq!(emit_lit(&single), "(0.10000000149011612 as F32)");
}

#[test]
fn strings_escape_as_json_does() {
    let s = emit_lit(&Lit::Str("q\\\n\r\t\u{8}\u{c}\u{0}\u{1b}\u{2028}\u{2029}é".into()));
    assert_eq!(s, "\"q\\\\\\n\\r\\t\\b\\f\\u0000\\u001b\\u2028\\u2029é\"");
    // The quote that needs fewer escapes, as oxfmt picks it.
    assert_eq!(emit_lit(&Lit::Str("a\"b".into())), "'a\"b'");
    assert_eq!(emit_lit(&Lit::Str("a'b\"c".into())), "\"a'b\\\"c\"");
}

#[test]
fn object_literals_inside_an_erased_wrapper_are_parenthesized() {
    use purecrate_ir::Wrapper;
    let lit = Expr::Construct {
        ty: Name::new("P"),
        variant: None,
        fields: Fields::Named(vec![(Name::new("a"), Expr::var("a"))]),
        base: None,
    };
    let boxed = Expr::Ignored { wrapper: Wrapper::Box, expr: Box::new(lit.clone()) };
    let body = arrow_expr(&boxed, 0);
    assert_eq!(body, "({ a })");
    let mut out = String::new();
    Sink::Effect.finish_expr(&boxed, 0, &mut out);
    assert_eq!(out, "({ a });\n");
    let some = Expr::Call { callee: Callee::OptionSome, args: vec![lit] };
    assert_eq!(arrow_expr(&some, 0), "({ a })");
}

#[test]
fn integer_from_converts_only_into_bigint() {
    let from = |from, to| {
        emit_expr(
            &Expr::Call { callee: purecrate_ir::Callee::IntFrom { from: Some(from), to }, args: vec![Expr::var("x")] },
            0,
        )
    };
    assert_eq!(from(IntTy::U32, IntTy::I64), "(globalThis.BigInt(x) as I64)");
    assert_eq!(from(IntTy::U8, IntTy::I32), "(x as number as I32)");
    assert_eq!(from(IntTy::I64, IntTy::I64), "x");
}
