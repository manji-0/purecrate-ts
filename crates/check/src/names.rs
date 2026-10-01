//! Flattened names must be unique, map to distinct files, and be usable as
//! TS identifiers without renaming.

use std::collections::HashMap;

use purecrate_ir::{to_camel, Crate, Expr, Item, Name, Reason, VariantFields, PROTO_KEY, TS_GLOBALS};

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

/// Top-level names the emitted package defines; see also `TS_GLOBALS`.
const PACKAGE_NAMES: &[&str] = &[
    "Result", "assertNever", "Int", "Str", "Slice", "Ord", "Iter", "Char", "Uuid", "UuidError", "parseJson", "I8", "I16", "I32", "I64", "U8", "U16", "U32", "U64", "Usize", "F32", "F64",
];

fn is_generated(name: &str) -> bool {
    PACKAGE_NAMES.contains(&name) || TS_GLOBALS.contains(&name)
}

/// File stems the emitted package already uses.
const GENERATED_STEMS: &[&str] = &["index", "purecrate-wire", "purecrate-runtime", "purecrate-zod", "purecrate-valibot", "purecrate-arktype"];

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
        // A function or const prints as its `to_camel` spelling (`check::rename`).
        let printed = match item {
            Item::Fn(_) | Item::Const(_) => to_camel(name),
            _ => name.to_string(),
        };
        if is_generated(&printed) {
            out.push(Diagnostic::at(
                i, Reason::ReservedName,
                format!("`{name}` is reserved by the generated package"),
            ));
            continue;
        }
        if GENERATED_STEMS.contains(&stem.as_str()) {
            out.push(Diagnostic::at(
                i, Reason::NameCollision,
                format!("`{name}` would be emitted as `{stem}.ts`, which the generated package already uses"),
            ));
            continue;
        }
        match seen.get(&stem) {
            // Consts share `consts.ts`; rustc keeps their names distinct.
            Some(&first) if matches!(item, Item::Const(_)) && matches!(krate.items[first], Item::Const(_)) => {}
            Some(&first) if matches!(item, Item::Const(_)) || matches!(krate.items[first], Item::Const(_)) => {
                let other = krate.items[first].name().as_str();
                out.push(
                    Diagnostic::at(i, Reason::NameCollision, format!(
                        "`{other}` and `{name}` would both be emitted as `{stem}.ts`, which holds the crate's consts"
                    ))
                    .also(first),
                );
            }
            Some(&first) => {
                let other = krate.items[first].name().as_str();
                let message = if other == name {
                    format!("`{name}` is defined more than once after flattening")
                } else {
                    format!("`{other}` and `{name}` would both be emitted as `{stem}.ts`")
                };
                out.push(Diagnostic::at(i, Reason::NameCollision, message).also(first));
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
    let mut seen: HashMap<(&str, String), usize> = HashMap::new();
    for (i, item) in krate.items.iter().enumerate() {
        let Item::Fn(f) = item else { continue };
        let Some(owner) = &f.owner else { continue };
        let (owner, rust) = (owner.as_str(), f.name.as_str());
        // A method prints as its `to_camel` spelling (`check::rename`).
        let printed = to_camel(rust);
        let name = printed.as_str();
        if name == PROTO_KEY {
            out.push(Diagnostic::at(
                i, Reason::ReservedName,
                format!("method `{owner}.{name}` would set the prototype of the emitted companion object"),
            ));
        } else if builtin.get(owner).is_some_and(|b| b.contains(&name)) {
            out.push(Diagnostic::at(
                i, Reason::NameCollision,
                format!("method `{owner}.{name}` collides with the generated companion member `{name}`"),
            ));
        } else if let Some(&first) = seen.get(&(owner, printed.clone())) {
            let other = krate.items[first].name().as_str();
            let message = if other == rust {
                format!("method `{owner}.{rust}` is defined more than once")
            } else {
                format!("methods `{owner}.{other}` and `{owner}.{rust}` would both be `{owner}.{name}` in TS")
            };
            out.push(Diagnostic::at(i, Reason::NameCollision, message).also(first));
        } else {
            seen.insert((owner, printed.clone()), i);
        }
    }
}

fn identifiers_are_usable(i: usize, item: &Item, out: &mut Vec<Diagnostic>) {
    let mut bad = |what: &str, name: &Name, why: &str| {
        out.push(Diagnostic::at(
            i, Reason::ReservedName,
            format!("{what} `{}` {why}", name.as_str()),
        ));
    };
    let mut ident = |what: &str, name: &Name| {
        // Functions, consts, parameters, and bindings print as `to_camel`.
        let printed = match what {
            "field" | "variant" | "type" => name.as_str().to_string(),
            _ => to_camel(name.as_str()),
        };
        let s = printed.as_str();
        if s == PROTO_KEY && matches!(what, "field" | "variant") {
            bad(what, name, "would set the prototype of the emitted object literal");
        } else if what == "variant" {
            // A variant is only a property key and a `kind` string.
        } else if s.starts_with("r#") {
            bad(what, name, "is a raw identifier; rename it in the crate");
        } else if TS_RESERVED.contains(&s) {
            bad(what, name, "is a reserved word in TypeScript");
        } else if is_generated(s) {
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
                ident("variant", &v.name);
                if let VariantFields::Struct(fields) = &v.fields {
                    for f in fields {
                        ident("field", &f.name);
                    }
                }
            }
        }
        Item::Alias(a) => ident("type", &a.name),
        Item::Const(c) => ident("const", &c.name),
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
        Item::Fn(_) | Item::Const(_) => None,
    };
    if let Some(n) = type_name.filter(|n| TS_TYPE_KEYWORDS.contains(&n.as_str())) {
        out.push(Diagnostic::at(
            i, Reason::ReservedName,
            format!("type `{}` is a TypeScript type keyword", n.as_str()),
        ));
    }
    if let Item::Enum(e) = item {
        for v in &e.variants {
            if let VariantFields::Struct(fields) = &v.fields {
                if fields.iter().any(|f| f.name.as_str() == "kind") {
                    out.push(Diagnostic::at(
                        i, Reason::ReservedName,
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
        Expr::Let { name, .. } | Expr::For { var: name, .. } | Expr::ForEach { var: name, .. } => f(name),
        Expr::Match { arms, .. } => arms
            .iter()
            .for_each(|arm| arm.pattern.bindings().into_iter().for_each(&mut *f)),
        Expr::Closure { params, .. } => params.iter().for_each(|p| f(&p.name)),
        _ => {}
    }
    expr.children().into_iter().for_each(|c| for_each_binding(c, f));
}
