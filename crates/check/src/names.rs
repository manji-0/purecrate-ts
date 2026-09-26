//! Flattened names must be unique, map to distinct files, and be usable as
//! TS identifiers without renaming.

use std::collections::HashMap;

use purecrate_ir::{Crate, Expr, Fields, Item, Name, Pattern, VariantBind, VariantFields};

use crate::Diagnostic;

/// Reserved in strict-mode ES modules, or bound by the emitted code itself.
const TS_RESERVED: &[&str] = &[
    "arguments", "await", "break", "case", "catch", "class", "const", "continue", "debugger",
    "default", "delete", "do", "else", "enum", "eval", "export", "extends", "false", "finally",
    "for", "function", "if", "implements", "import", "in", "instanceof", "interface", "let",
    "new", "null", "package", "private", "protected", "public", "return", "static", "super",
    "switch", "this", "throw", "true", "try", "typeof", "undefined", "var", "void", "while",
    "with", "yield",
];

/// Cannot name a TS type alias.
const TS_TYPE_KEYWORDS: &[&str] = &[
    "any", "bigint", "boolean", "never", "number", "object", "string", "symbol", "unknown",
];

/// Top-level names the emitted package defines or relies on.
const GENERATED_NAMES: &[&str] = &["Result", "assertNever", "Readonly", "ReadonlyArray"];

/// File stems the emitted package already uses.
const GENERATED_STEMS: &[&str] = &["index", "result", "assert-never"];

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    files_are_distinct(krate, &mut out);
    companion_members_are_distinct(krate, &mut out);
    for (i, item) in krate.items.iter().enumerate() {
        identifiers_are_usable(i, item, &mut out);
    }
    out
}

fn is_top_level(item: &Item) -> bool {
    !matches!(item, Item::Fn(f) if f.owner.is_some())
}

fn files_are_distinct(krate: &Crate, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (i, item) in krate.items.iter().enumerate().filter(|(_, it)| is_top_level(it)) {
        let name = item.name().as_str();
        let stem = item.file_stem();
        if GENERATED_NAMES.contains(&name) {
            out.push(Diagnostic::at(
                i,
                format!("`{name}` is reserved by the generated package"),
            ));
            continue;
        }
        if GENERATED_STEMS.contains(&stem.as_str()) {
            out.push(Diagnostic::at(
                i,
                format!("`{name}` would be emitted as `{stem}.ts`, which the generated package already uses"),
            ));
            continue;
        }
        match seen.get(&stem) {
            Some(&first) => {
                let other = krate.items[first].name().as_str();
                let message = if other == name {
                    format!("`{name}` is defined more than once after flattening")
                } else {
                    format!("`{other}` and `{name}` would both be emitted as `{stem}.ts`")
                };
                out.push(Diagnostic {
                    item: i,
                    also: vec![first],
                    message,
                });
            }
            None => {
                seen.insert(stem, i);
            }
        }
    }
}

/// Companion members: `of` (structs) or variant constructors (enums), plus methods.
fn companion_members_are_distinct(krate: &Crate, out: &mut Vec<Diagnostic>) {
    let mut builtin: HashMap<&str, Vec<&str>> = HashMap::new();
    for item in &krate.items {
        match item {
            Item::Struct(s) => {
                builtin.insert(s.name.as_str(), vec!["of"]);
            }
            Item::Enum(e) => {
                builtin.insert(e.name.as_str(), e.variants.iter().map(|v| v.name.as_str()).collect());
            }
            _ => {}
        }
    }
    let mut seen: HashMap<(&str, &str), usize> = HashMap::new();
    for (i, item) in krate.items.iter().enumerate() {
        let Item::Fn(f) = item else { continue };
        let Some(owner) = &f.owner else { continue };
        let (owner, name) = (owner.as_str(), f.name.as_str());
        if builtin.get(owner).is_some_and(|b| b.contains(&name)) {
            out.push(Diagnostic::at(
                i,
                format!("method `{owner}.{name}` collides with the generated companion member `{name}`"),
            ));
        } else if let Some(&first) = seen.get(&(owner, name)) {
            out.push(Diagnostic {
                item: i,
                also: vec![first],
                message: format!("method `{owner}.{name}` is defined more than once"),
            });
        } else {
            seen.insert((owner, name), i);
        }
    }
}

fn identifiers_are_usable(i: usize, item: &Item, out: &mut Vec<Diagnostic>) {
    let mut bad = |what: &str, name: &Name, why: &str| {
        out.push(Diagnostic::at(
            i,
            format!("{what} `{}` {why}", name.as_str()),
        ));
    };
    let mut ident = |what: &str, name: &Name| {
        let s = name.as_str();
        if s.starts_with("r#") {
            bad(what, name, "is a raw identifier; rename it in the crate");
        } else if TS_RESERVED.contains(&s) {
            bad(what, name, "is a reserved word in TypeScript");
        } else if GENERATED_NAMES.contains(&s) {
            bad(what, name, "shadows a name the generated code uses");
        }
    };

    match item {
        Item::Struct(s) => {
            ident("type", &s.name);
            s.fields.iter().for_each(|f| ident("field", &f.name));
        }
        Item::Enum(e) => {
            ident("type", &e.name);
            for v in &e.variants {
                if let VariantFields::Struct(fields) = &v.fields {
                    for f in fields {
                        ident("field", &f.name);
                    }
                }
            }
        }
        Item::Alias(a) => ident("type", &a.name),
        Item::Fn(f) => {
            if f.owner.is_none() {
                ident("function", &f.name);
            }
            f.params.iter().for_each(|p| ident("parameter", &p.name));
            for_each_binding(&f.body, &mut |n| ident("binding", n));
        }
    }

    let type_name = match item {
        Item::Struct(s) => Some(&s.name),
        Item::Enum(e) => Some(&e.name),
        Item::Alias(a) => Some(&a.name),
        Item::Fn(_) => None,
    };
    if let Some(n) = type_name.filter(|n| TS_TYPE_KEYWORDS.contains(&n.as_str())) {
        out.push(Diagnostic::at(
            i,
            format!("type `{}` is a TypeScript type keyword", n.as_str()),
        ));
    }
    if let Item::Enum(e) = item {
        for v in &e.variants {
            if let VariantFields::Struct(fields) = &v.fields {
                if fields.iter().any(|f| f.name.as_str() == "kind") {
                    out.push(Diagnostic::at(
                        i,
                        format!(
                            "variant `{}::{}` has a field `kind`, which is the union discriminant",
                            e.name.as_str(),
                            v.name.as_str()
                        ),
                    ));
                }
            }
        }
    }
}

fn for_each_binding(expr: &Expr, f: &mut impl FnMut(&Name)) {
    match expr {
        Expr::Let { name, value, then } => {
            f(name);
            for_each_binding(value, f);
            for_each_binding(then, f);
        }
        Expr::Match { scrutinee, arms } => {
            for_each_binding(scrutinee, f);
            for arm in arms {
                pattern_bindings(&arm.pattern, f);
                for_each_binding(&arm.body, f);
            }
        }
        Expr::If { cond, then, else_ } => {
            for_each_binding(cond, f);
            for_each_binding(then, f);
            for_each_binding(else_, f);
        }
        Expr::Call { args, .. } | Expr::Tuple(args) | Expr::Array(args) => {
            args.iter().for_each(|a| for_each_binding(a, f))
        }
        Expr::Construct { fields, .. } => match fields {
            Fields::Positional(xs) => xs.iter().for_each(|x| for_each_binding(x, f)),
            Fields::Named(xs) => xs.iter().for_each(|(_, x)| for_each_binding(x, f)),
            Fields::Unit => {}
        },
        Expr::Field { base, .. } | Expr::Unary { expr: base, .. } | Expr::Return(base) => {
            for_each_binding(base, f)
        }
        Expr::Binary { left, right, .. } => {
            for_each_binding(left, f);
            for_each_binding(right, f);
        }
        Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable => {}
    }
}

fn pattern_bindings(pattern: &Pattern, f: &mut impl FnMut(&Name)) {
    match pattern {
        Pattern::Var(n) => f(n),
        Pattern::Variant { bind, .. } => match bind {
            VariantBind::Unit => {}
            VariantBind::Tuple(ps) => ps.iter().for_each(|p| pattern_bindings(p, f)),
            VariantBind::Struct(ps) => ps.iter().for_each(|(_, p)| pattern_bindings(p, f)),
        },
        Pattern::Wildcard | Pattern::Lit(_) => {}
    }
}
