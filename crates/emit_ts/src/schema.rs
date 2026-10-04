//! Wire schemas for one chosen library. The shape is serde's default JSON.
//! Numeric fields use the shared `purecrate-*` adapter, so the result is the
//! domain brand (`I32`, not a bare `number`). `toJson` writes the same shape
//! back, as serde_json writes the Rust value, with no schema library.

use purecrate_ir::{Crate, Item, Serde, Struct, Ty, VariantFields};

use crate::closed_ctor;
use crate::doc;
use crate::js::{self, arrow, call, call_path, expr_body, member, raw, str_lit, ternary, Body, Js, Stmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireSchema {
    Zod,
    Valibot,
    Arktype,
}

impl WireSchema {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "zod" => Some(Self::Zod),
            "valibot" => Some(Self::Valibot),
            "arktype" => Some(Self::Arktype),
            _ => None,
        }
    }

    pub fn package(self) -> &'static str {
        match self {
            Self::Zod => "purecrate-zod",
            Self::Valibot => "purecrate-valibot",
            Self::Arktype => "purecrate-arktype",
        }
    }

    pub fn runtime_dep(self) -> &'static str {
        match self {
            Self::Zod => "zod",
            Self::Valibot => "valibot",
            Self::Arktype => "arktype",
        }
    }

    pub fn version(self) -> &'static str {
        match self {
            Self::Zod => "4.6.5",
            Self::Valibot => "1.1.0",
            Self::Arktype => "2.1.22",
        }
    }
}

/// The serde derives of a struct or enum (design/04 §3.2): a schema reads
/// one that derives `Deserialize`, `toJson` writes one that derives
/// `Serialize`. std's `Ordering` derives neither, and `check::accept` keeps
/// a type without the derive out of one with it.
fn serde(item: &Item) -> Serde {
    match item {
        Item::Struct(s) => s.serde,
        Item::Enum(e) => e.serde,
        _ => Serde::default(),
    }
}

/// Some exported type derives `Serialize` or `Deserialize`, so `--schema`
/// has something to write.
pub fn has_wire(krate: &Crate) -> bool {
    krate.exported().any(|i| serde(i).any())
}

pub fn emit_wire(krate: &Crate, schema: WireSchema) -> String {
    crate::with_internal_names(krate, || emit_wire_in(krate, schema))
}

/// A piece of the wire module: text as it is (imports, comments, blank
/// lines), or a declaration that `js` lays out.
enum Chunk {
    Text(String),
    Decl { export: bool, name: String, ty: Option<String>, value: Js },
}

impl Chunk {
    fn decl(export: bool, name: impl Into<String>, ty: Option<String>, value: Js) -> Self {
        Chunk::Decl { export, name: name.into(), ty, value }
    }
}

fn render(chunks: &[Chunk]) -> String {
    let mut out = String::new();
    for c in chunks {
        match c {
            Chunk::Text(t) => out.push_str(t),
            Chunk::Decl { export, name, ty, value } => {
                out.push_str(&doc::print(&js::decl(*export, name, ty.as_deref(), value), crate::WIDTH, 0));
                out.push('\n');
            }
        }
    }
    out
}

fn emit_wire_in(krate: &Crate, schema: WireSchema) -> String {
    let mut chunks = vec![Chunk::Text(format!("{}\n{}", super::HEADER, header(schema)))];
    let wired: Vec<&Item> = krate.exported().filter(|i| serde(i).de).collect();
    chunks.push(Chunk::Text(wire_imports(krate, &wired)));
    if schema == WireSchema::Arktype {
        // Arktype compiles each shape on first use (`memo`), so any order works.
        for item in &wired {
            match item {
                Item::Struct(s) => ark_struct(s, &refusal(krate, s), &doc_of(item), &mut chunks),
                Item::Enum(e) => ark_enum(e, &doc_of(item), &mut chunks),
                _ => {}
            }
        }
    } else {
        for (item, recursive) in wire_order(&wired) {
            let value = match item {
                Item::Struct(s) => struct_schema(schema, s, &refusal(krate, s)),
                Item::Enum(e) => enum_schema(schema, e.name.as_str(), &e.variants),
                _ => continue,
            };
            chunks.push(Chunk::Text(format!("\n{}", doc_of(item))));
            let name = item.name().as_str();
            let (ty, value) = declare(schema, name, value, recursive);
            chunks.push(Chunk::decl(true, name, Some(ty), value));
        }
    }
    from_json(schema, &wired, &mut chunks);
    to_json(krate, &mut chunks);
    // The layout reads the names as printed (`DomainYen`, not `Yen$`).
    let names = crate::plain::wire_name_map(&render(&chunks));
    let rename = |s: &str| crate::plain::rename_idents(s, &names);
    for c in &mut chunks {
        match c {
            Chunk::Text(t) => *t = rename(t),
            Chunk::Decl { name, ty, value, .. } => {
                *name = rename(name);
                if let Some(t) = ty {
                    *t = rename(t);
                }
                value.rename(&rename);
            }
        }
    }
    crate::tidy::wrap(&super::imports::prune_unused(&render(&chunks)), crate::WIDTH)
}

/// The type's `///` comment as JSDoc for its schema, so a hover on the
/// schema reads what the type is; empty without one.
fn doc_of(item: &Item) -> String {
    let doc = match item {
        Item::Struct(s) => &s.doc,
        Item::Enum(e) => &e.doc,
        _ => return String::new(),
    };
    crate::items::jsdoc(doc, "")
}

/// `fromJson.T(text)`: serde_json's text of a `T` read into the domain
/// value, `toJson.T`'s inverse. The text goes through `parseJson`, so a
/// 64-bit integer past 2^53 stays exact; a malformed text or a value the
/// schema refuses throws, as `JSON.parse` and the library's `parse` do.
fn from_json(schema: WireSchema, wired: &[&Item], chunks: &mut Vec<Chunk>) {
    if wired.is_empty() {
        return;
    }
    chunks.push(Chunk::Text(
        "\n/**\n * Each type read from the JSON text serde_json writes, through `parseJson`;\n \
         * throws on malformed text or a value the schema refuses.\n */\n"
            .to_string(),
    ));
    let entries = wired
        .iter()
        .map(|item| {
            let name = item.name().as_str();
            let text = call_path("parseJson", vec![raw("text")]);
            let read = match schema {
                WireSchema::Zod => call_path(&format!("{name}.parse"), vec![text]),
                WireSchema::Valibot => call_path("v.parse", vec![raw(name), text]),
                WireSchema::Arktype => call_path(&format!("{name}.assert"), vec![text]),
            };
            (name.to_string(), arrow("(text: string)", Some(&format!("{name}$")), expr_body(read)))
        })
        .collect();
    chunks.push(Chunk::decl(true, "fromJson", None, Js::As(Box::new(Js::Object(entries)), "const".into())));
}

/// The types an item's schema reads by name.
fn named_in(item: &Item) -> Vec<String> {
    fn walk(ty: &Ty, out: &mut Vec<String>) {
        match ty {
            Ty::Named(n) => out.push(n.as_str().to_string()),
            Ty::Option(t) | Ty::Vec(t) | Ty::Ignored { inner: t, .. } => walk(t, out),
            Ty::Tuple(ts) => ts.iter().for_each(|t| walk(t, out)),
            Ty::Result { ok, err } => {
                walk(ok, out);
                walk(err, out);
            }
            Ty::Fn { .. } | Ty::Prim(_) | Ty::Never => {}
        }
    }
    let mut out = Vec::new();
    match item {
        Item::Struct(s) => {
            s.fields.iter().for_each(|f| walk(&f.ty, &mut out));
            if let Some(t) = &s.wire_from {
                walk(t, &mut out);
            }
        }
        Item::Enum(e) => {
            for v in &e.variants {
                match &v.fields {
                    VariantFields::Unit => {}
                    VariantFields::Tuple(ts) => ts.iter().for_each(|t| walk(t, &mut out)),
                    VariantFields::Struct(fs) => fs.iter().for_each(|f| walk(&f.ty, &mut out)),
                }
            }
        }
        _ => {}
    }
    out
}

/// The schemas in an order where each follows the ones it reads, so a
/// `const` never reads one not yet defined; `true` for those in a cycle of
/// references (a recursive type), which alone need `lazy`. Tarjan's
/// strongly connected components come out dependencies first; the input's
/// order breaks ties, so the output is deterministic.
fn wire_order<'a>(items: &[&'a Item]) -> Vec<(&'a Item, bool)> {
    struct Tarjan<'b> {
        edges: Vec<Vec<usize>>,
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<(usize, bool)>,
        _p: std::marker::PhantomData<&'b ()>,
    }
    impl Tarjan<'_> {
        fn visit(&mut self, v: usize) {
            self.index[v] = Some(self.next);
            self.low[v] = self.next;
            self.next += 1;
            self.stack.push(v);
            self.on_stack[v] = true;
            for w in self.edges[v].clone() {
                match self.index[w] {
                    None => {
                        self.visit(w);
                        self.low[v] = self.low[v].min(self.low[w]);
                    }
                    Some(iw) if self.on_stack[w] => self.low[v] = self.low[v].min(iw),
                    Some(_) => {}
                }
            }
            if Some(self.low[v]) == self.index[v] {
                let mut component = Vec::new();
                while let Some(w) = self.stack.pop() {
                    self.on_stack[w] = false;
                    component.push(w);
                    if w == v {
                        break;
                    }
                }
                let cyclic = component.len() > 1 || self.edges[v].contains(&v);
                component.sort_unstable();
                self.out.extend(component.into_iter().map(|w| (w, cyclic)));
            }
        }
    }
    let position: std::collections::HashMap<&str, usize> =
        items.iter().enumerate().map(|(i, it)| (it.name().as_str(), i)).collect();
    let edges = items
        .iter()
        .map(|it| {
            let mut e: Vec<usize> = named_in(it).iter().filter_map(|n| position.get(n.as_str()).copied()).collect();
            e.sort_unstable();
            e.dedup();
            e
        })
        .collect();
    let n = items.len();
    let mut t = Tarjan {
        edges,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
        _p: std::marker::PhantomData,
    };
    for v in 0..n {
        if t.index[v].is_none() {
            t.visit(v);
        }
    }
    t.out.into_iter().map(|(i, cyclic)| (items[i], cyclic)).collect()
}

/// The schema's declared type, and its value behind `lazy` only for a
/// recursive type.
fn declare(schema: WireSchema, name: &str, value: Js, recursive: bool) -> (String, Js) {
    let (ty, lazy) = match schema {
        // `unknown` is the input type's default.
        WireSchema::Zod => (format!("z.ZodType<{name}$>"), "z.lazy"),
        WireSchema::Valibot => (format!("v.GenericSchema<unknown, {name}$>"), "v.lazy"),
        WireSchema::Arktype => unreachable!("arktype declarations are printed by `ark_*`"),
    };
    let value = if recursive { call_path(lazy, vec![arrow("()", None, expr_body(value))]) } else { value };
    (ty, value)
}

/// `{ kind: "V", a: .. }`, the domain value a variant arm builds.
fn variant_value(variant: &str, rest: Vec<(String, Js)>) -> Js {
    let mut fields = vec![("kind".to_string(), str_lit(variant))];
    fields.extend(rest);
    Js::Object(fields)
}

/// `recv.method(args)`.
fn method(recv: Js, name: &str, args: Vec<Js>) -> Js {
    call(member(recv, name), args)
}

/// What names a refused `#[serde(try_from)]` read: the error's text when
/// its type has `toString` (from `impl Display`, the text serde reports),
/// else its variant when it is an enum of the crate, else the type alone.
#[derive(Clone, PartialEq)]
enum Refusal {
    Type,
    Variant,
    /// The error type, whose `toString` gives the text.
    Text(String),
}

fn refusal(krate: &Crate, s: &Struct) -> Refusal {
    if s.wire_from.is_none() {
        return Refusal::Type;
    }
    let err = krate.items.iter().find_map(|item| match item {
        Item::Fn(f) if f.owner.as_ref() == Some(&s.name) && f.name.as_str() == "tryFrom" => match &f.ret {
            Ty::Result { err, .. } => match &**err {
                Ty::Named(e) => Some(e.clone()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    });
    let Some(err) = err else { return Refusal::Type };
    let has_text = krate
        .items
        .iter()
        .any(|i| matches!(i, Item::Fn(f) if f.owner.as_ref() == Some(&err) && f.name.as_str() == "toString"));
    if has_text {
        Refusal::Text(err.as_str().to_string())
    } else if krate.items.iter().any(|i| matches!(i, Item::Enum(en) if en.name == err)) {
        Refusal::Variant
    } else {
        Refusal::Type
    }
}

/// One import per module: values then `type` aliases, each domain type and
/// its companion under one alias (`DomainYen`, from `Yen$`), including an
/// error type whose text names a refusal.
fn wire_imports(krate: &Crate, wired: &[&Item]) -> String {
    #[derive(Default)]
    struct Specs {
        values: Vec<String>,
        types: Vec<String>,
    }
    let mut by_file: std::collections::BTreeMap<String, Specs> = std::collections::BTreeMap::new();
    // A value import of `T` names its type too, so a type is imported alone
    // only where nothing reads its companion.
    let mut push = |stem: String, value: Option<String>, ty: Option<String>| {
        let e = by_file.entry(stem).or_default();
        if let Some(v) = value {
            e.types.retain(|t| *t != v);
            if !e.values.contains(&v) {
                e.values.push(v);
            }
        }
        if let Some(t) = ty {
            if !e.types.contains(&t) && !e.values.contains(&t) {
                e.types.push(t);
            }
        }
    };
    for item in krate.exported().filter(|i| serde(i).any()) {
        let name = item.name().as_str();
        let stem = item.file_stem();
        let alias = format!("{name} as {name}$");
        let value =
            matches!(item, Item::Struct(s) if (s.newtype_inner().is_some() && !s.closed) || s.wire_from.is_some());
        if matches!(item, Item::Struct(s) if s.closed && s.wire_from.is_none()) {
            push(stem.clone(), Some(closed_ctor(name)), None);
            push(stem, None, Some(alias));
        } else if value {
            push(stem, Some(alias), None);
        } else {
            push(stem, None, Some(alias));
        }
    }
    for item in wired {
        if let Item::Struct(s) = item {
            if let Refusal::Text(err) = refusal(krate, s) {
                let stem = purecrate_ir::Name::new(err.clone()).file_stem();
                push(stem, Some(format!("{err} as {err}$")), None);
            }
        }
    }
    by_file
        .into_iter()
        .map(|(stem, specs)| {
            let mut parts = specs.values;
            parts.extend(specs.types.into_iter().map(|t| format!("type {t}")));
            format!("import {{ {} }} from \"./{stem}.ts\";\n", parts.join(", "))
        })
        .collect()
}

fn header(schema: WireSchema) -> String {
    let own: &str = match schema {
        WireSchema::Zod => "\
import { z } from \"zod\";
import { bool, char, f32, f64, i16, i32, i64, i8, nullable, optionalField, str, u16, u32, u64, u8, unit, unitEnum, unitVariant, usize, uuid, uuidError } from \"purecrate-zod\";
",
        WireSchema::Valibot => "\
import * as v from \"valibot\";
import { bool, char, f32, f64, i16, i32, i64, i8, nullable, str, u16, u32, u64, u8, unit, unitEnum, unitVariant, usize, uuid, uuidError } from \"purecrate-valibot\";
",
        WireSchema::Arktype => "\
import { type } from \"arktype\";
import { bool, char, f32, f64, fail, i16, i32, i64, i8, keyed, memo, nullable, str, u16, u32, u64, u8, unit, unitEnum, usize, uuid, uuidError, type Wire } from \"purecrate-arktype\";
",
    };
    format!("import {{ assertNever, Json, parseJson }} from \"purecrate\";\n{own}")
}

/// Zod and valibot. Where the object's output is not the domain value as it
/// is, the value is built field by field: a schema's inferred object type
/// makes a field that may be `undefined` optional (`unit?: undefined`),
/// which the domain type does not accept.
fn struct_schema(schema: WireSchema, s: &Struct, refused: &Refusal) -> Js {
    let name = s.name.as_str();
    if let Some(from) = &s.wire_from {
        return try_from_schema(schema, s, from, refused);
    }
    if let Some(inner) = s.newtype_inner() {
        let value = schema_ty(schema, inner);
        // `x`, not `v`, which names valibot's namespace.
        let build = arrow("(x)", Some(&format!("{name}$")), expr_body(newtype_build(s, "x")));
        return match schema {
            WireSchema::Zod => method(value, "transform", vec![build]),
            WireSchema::Valibot => call_path("v.pipe", vec![value, call_path("v.transform", vec![build])]),
            WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
        };
    }
    let fields = Js::Object(object_fields(schema, &s.fields));
    // The object's own output is the domain value, but for a closed struct
    // (built through its constructor), a `()` field (which inference makes
    // optional), or with valibot an `Option` field (missing reads as
    // `undefined`, which `?? null` turns into `None`). Both libraries drop
    // unknown keys, as serde does.
    let unit = |t: &Ty| {
        matches!(t, Ty::Prim(purecrate_ir::Prim::Unit))
            || matches!(t, Ty::Option(i) | Ty::Ignored { inner: i, .. } if matches!(**i, Ty::Prim(purecrate_ir::Prim::Unit)))
    };
    let built = s.closed
        || s.fields.iter().any(|f| unit(&f.ty))
        || (schema == WireSchema::Valibot && s.fields.iter().any(|f| matches!(f.ty, Ty::Option(_))));
    let object = match schema {
        WireSchema::Zod => call_path("z.object", vec![fields]),
        _ => call_path("v.object", vec![fields]),
    };
    if !built {
        return object;
    }
    let build = arrow("(x)", Some(&format!("{name}$")), expr_body(record_build(s, fill(schema, &s.fields, "x"))));
    match schema {
        WireSchema::Zod => method(object, "transform", vec![build]),
        WireSchema::Valibot => call_path("v.pipe", vec![object, call_path("v.transform", vec![build])]),
        WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
    }
}

/// `name: schema` for each field of a struct or struct variant.
fn object_fields(schema: WireSchema, fields: &[purecrate_ir::Field]) -> Vec<(String, Js)> {
    fields.iter().map(|f| (f.name.as_str().to_string(), schema_ty_in(schema, &f.ty, true))).collect()
}

/// The message of the issue a failed `try_from` raises: the type, and the
/// error's text or variant (`refusal`).
fn try_from_message(name: &str, refused: &Refusal) -> Js {
    match refused {
        Refusal::Text(err) => raw(format!("`{name}: ${{{err}$.toString(r.error)}}`")),
        Refusal::Variant => raw(format!("`{name}: ${{r.error.kind}}`")),
        Refusal::Type => str_lit(name),
    }
}

/// `#[serde(try_from = "T")]`: read `T`, then `S.tryFrom`; `Err` fails the
/// read, as serde fails deserialization (design/04 §5). The value comes
/// only from the checked constructor, so a closed type keeps its invariant.
fn try_from_schema(schema: WireSchema, s: &Struct, from: &Ty, refused: &Refusal) -> Js {
    let name = s.name.as_str();
    let from = schema_ty(schema, from);
    let message = try_from_message(name, refused);
    let ret = format!("{name}$");
    let failed = raw("r.kind === \"Err\"");
    match schema {
        WireSchema::Zod => {
            let issue = Js::Object(vec![
                ("code".into(), str_lit("custom")),
                ("message".into(), message),
                ("input".into(), raw("x")),
                ("params".into(), Js::Object(vec![("error".into(), raw("r.error"))])),
            ]);
            let body = vec![
                Stmt::Const("r".into(), call_path(&format!("{name}$.tryFrom"), vec![raw("x")])),
                Stmt::IfBlock(
                    failed,
                    vec![Stmt::Expr(call_path("ctx.addIssue", vec![issue])), Stmt::Return(raw("z.NEVER"))],
                ),
                Stmt::Return(raw("r.value")),
            ];
            method(from, "transform", vec![arrow("(x, ctx)", Some(&ret), Body::Block(body))])
        }
        WireSchema::Valibot => {
            let body = vec![
                Stmt::Const("r".into(), call_path(&format!("{name}$.tryFrom"), vec![raw("dataset.value")])),
                Stmt::IfBlock(
                    failed,
                    vec![
                        Stmt::Expr(call_path("addIssue", vec![Js::Object(vec![("message".into(), message)])])),
                        Stmt::Return(raw("NEVER")),
                    ],
                ),
                Stmt::Return(raw("r.value")),
            ];
            let transform = arrow("({ dataset, addIssue, NEVER })", Some(&ret), Body::Block(body));
            call_path("v.pipe", vec![from, call_path("v.rawTransform", vec![transform])])
        }
        WireSchema::Arktype => unreachable!("arktype reads `try_from` in `ark_struct`"),
    }
}

/// The domain value of a newtype from its parsed inner value. A closed
/// newtype has no `of`; serde's derive builds it from the shape alone, and so
/// does the schema, through the package-internal constructor (design/04 §5).
fn newtype_build(s: &Struct, from: &str) -> Js {
    let name = s.name.as_str();
    if s.closed {
        call(raw(closed_ctor(name)), vec![raw(from)])
    } else {
        call_path(&format!("{name}$.of"), vec![raw(from)])
    }
}

/// `{ a: v.a }`, or through the constructor when the struct is closed.
fn record_build(s: &Struct, fields: Vec<(String, Js)>) -> Js {
    if s.closed {
        call(raw(closed_ctor(s.name.as_str())), vec![Js::Object(fields)])
    } else {
        Js::Object(fields)
    }
}

/// `a: v.a, b: v.b ?? null`: the fields of a parsed JSON object copied into
/// the domain value. Valibot leaves a missing optional field `undefined`.
fn fill(schema: WireSchema, fields: &[purecrate_ir::Field], from: &str) -> Vec<(String, Js)> {
    fields
        .iter()
        .map(|f| {
            let n = f.name.as_str();
            let read = if schema == WireSchema::Valibot && matches!(f.ty, Ty::Option(_)) {
                raw(format!("{from}.{n} ?? null"))
            } else {
                raw(format!("{from}.{n}"))
            };
            (n.to_string(), read)
        })
        .collect()
}

/// The names of a fieldless enum's variants, or `None` when one has
/// fields: the adapters' `unitEnum` reads it whole.
fn unit_names(variants: &[purecrate_ir::Variant]) -> Option<Vec<Js>> {
    variants.iter().map(|v| matches!(v.fields, VariantFields::Unit).then(|| str_lit(v.name.as_str()))).collect()
}

fn enum_schema(schema: WireSchema, name: &str, variants: &[purecrate_ir::Variant]) -> Js {
    if let Some(names) = unit_names(variants) {
        return call_path("unitEnum", vec![Js::Array(names)]);
    }
    let arms = variants.iter().map(|v| variant_arm(schema, name, v.name.as_str(), &v.fields)).collect();
    match schema {
        WireSchema::Zod => call_path("z.union", vec![Js::Array(arms)]),
        WireSchema::Valibot => call_path("v.union", vec![Js::Array(arms)]),
        WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
    }
}

/// serde's externally tagged enum: `{"Variant": ..}` has exactly one key, so
/// the wrapper rejects any other key. A unit variant is `"Variant"`, and
/// serde_json also reads `{"Variant": null}`. The fields inside a struct variant are a
/// struct's, and unknown ones are ignored as serde does by default.
fn variant_arm(schema: WireSchema, enum_name: &str, variant: &str, fields: &VariantFields) -> Js {
    let ret = format!("{enum_name}$");
    let (wrapper, rest) = match fields {
        VariantFields::Unit => {
            let value = arrow("()", Some(&ret), expr_body(variant_value(variant, Vec::new())));
            let tag = call_path("unitVariant", vec![str_lit(variant)]);
            return match schema {
                WireSchema::Zod => method(tag, "transform", vec![value]),
                WireSchema::Valibot => call_path("v.pipe", vec![tag, call_path("v.transform", vec![value])]),
                WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
            };
        }
        VariantFields::Tuple(tys) => tuple_json(schema, variant, tys, "x"),
        VariantFields::Struct(fs) => {
            let fields = Js::Object(object_fields(schema, fs));
            let inner = match schema {
                WireSchema::Zod => call_path("z.object", vec![fields]),
                _ => call_path("v.object", vec![fields]),
            };
            (inner, fill(schema, fs, &format!("x.{variant}")))
        }
    };
    let value = arrow("(x)", Some(&ret), expr_body(variant_value(variant, rest)));
    let keyed = Js::Object(vec![(variant.to_string(), wrapper)]);
    match schema {
        WireSchema::Zod => {
            method(method(call_path("z.object", vec![keyed]), "strict", Vec::new()), "transform", vec![value])
        }
        WireSchema::Valibot => {
            call_path("v.pipe", vec![call_path("v.strictObject", vec![keyed]), call_path("v.transform", vec![value])])
        }
        WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
    }
}

/// `type("unknown").pipe((v, ctx): T$ => { .. })`, an arktype morph whose
/// body starts by reading `wire` into `parsed`.
fn ark_morph(name: &str, body: Vec<Stmt>) -> Js {
    let pipe = arrow("(v, ctx)", Some(&format!("{name}$")), Body::Block(body));
    method(call_path("type", vec![str_lit("unknown")]), "pipe", vec![pipe])
}

fn ark_parsed(wire: &str) -> Vec<Stmt> {
    vec![
        Stmt::Const("parsed".into(), call(call(raw(wire), Vec::new()), vec![raw("v")])),
        Stmt::If(
            raw("parsed instanceof type.errors"),
            Box::new(Stmt::Return(call_path("fail", vec![raw("ctx"), raw("parsed")]))),
        ),
    ]
}

/// `memo(() => shape)`.
fn memo(shape: Js) -> Js {
    call_path("memo", vec![arrow("()", None, expr_body(shape))])
}

/// Arktype: every schema is a morph from `unknown`, annotated `Wire<T>` so
/// recursion does not make TypeScript infer `any`. The JSON shape is compiled
/// on first use (`memo`), so a definition may refer to a schema declared
/// later in the file. Variants are tried one at a time: arktype rejects an
/// unordered union of two object morphs.
fn ark_struct(s: &Struct, refused: &Refusal, doc: &str, chunks: &mut Vec<Chunk>) {
    let name = s.name.as_str();
    let wire = format!("{name}$wire");
    let (shape, tail) = match (&s.wire_from, s.newtype_inner()) {
        (Some(from), _) => {
            let message = try_from_message(name, refused);
            let tail = vec![
                Stmt::Const("r".into(), call_path(&format!("{name}$.tryFrom"), vec![raw("parsed")])),
                Stmt::If(
                    raw("r.kind === \"Err\""),
                    Box::new(Stmt::Return(Js::As(Box::new(call_path("ctx.error", vec![message])), "never".into()))),
                ),
                Stmt::Return(raw("r.value")),
            ];
            (schema_ty(WireSchema::Arktype, from), tail)
        }
        (None, Some(inner)) => (schema_ty(WireSchema::Arktype, inner), vec![Stmt::Return(newtype_build(s, "parsed"))]),
        (None, None) => {
            let fields = s
                .fields
                .iter()
                .map(|f| (f.name.as_str().to_string(), schema_ty_in(WireSchema::Arktype, &f.ty, true)))
                .collect();
            let build = record_build(s, fill(WireSchema::Arktype, &s.fields, "parsed"));
            (call_path("type", vec![Js::Object(fields)]), vec![Stmt::Return(build)])
        }
    };
    let mut body = ark_parsed(&wire);
    body.extend(tail);
    chunks.push(Chunk::Text("\n".into()));
    chunks.push(Chunk::decl(false, wire, None, memo(shape)));
    chunks.push(Chunk::Text(doc.to_string()));
    chunks.push(Chunk::decl(true, name, Some(format!("Wire<{name}$>")), ark_morph(name, body)));
}

fn ark_enum(en: &purecrate_ir::Enum, doc: &str, chunks: &mut Vec<Chunk>) {
    let name = en.name.as_str();
    chunks.push(Chunk::Text("\n".into()));
    if let Some(names) = unit_names(&en.variants) {
        chunks.push(Chunk::Text(doc.to_string()));
        let value = call_path("unitEnum", vec![str_lit(name), Js::Array(names)]);
        chunks.push(Chunk::decl(true, name, Some(format!("Wire<{name}$>")), value));
        return;
    }
    let mut body = Vec::new();
    for variant in &en.variants {
        let (arm, shape, tests) = ark_variant(name, variant);
        let keyed = Js::Object(vec![("\"+\"".into(), str_lit("reject")), (variant.name.as_str().to_string(), shape)]);
        chunks.push(Chunk::decl(false, arm, None, memo(call_path("type", vec![keyed]))));
        body.extend(tests);
    }
    body.push(Stmt::Return(Js::As(Box::new(call_path("ctx.error", vec![str_lit(name)])), "never".into())));
    chunks.push(Chunk::Text(doc.to_string()));
    let pipe = arrow("(v, ctx)", Some(&format!("{name}$")), Body::Block(body));
    let value = method(call_path("type", vec![str_lit("unknown")]), "pipe", vec![pipe]);
    chunks.push(Chunk::decl(true, name, Some(format!("Wire<{name}$>")), value));
}

/// One variant's memoized shape: its name, the JSON under the variant's key,
/// and the statements that try it.
fn ark_variant(en: &str, variant: &purecrate_ir::Variant) -> (String, Js, Vec<Stmt>) {
    let name = variant.name.as_str();
    let arm = format!("{en}$arm${name}");
    let parsed = Stmt::Const("parsed".into(), call(call(raw(arm.clone()), Vec::new()), vec![raw("v")]));
    let keyed = Stmt::If(
        call_path("keyed", vec![raw("v"), str_lit(name)]),
        Box::new(Stmt::Return(call_path("fail", vec![raw("ctx"), raw("parsed")]))),
    );
    let accepted = |value: Js| Stmt::If(raw("!(parsed instanceof type.errors)"), Box::new(Stmt::Return(value)));
    let (shape, rest) = match &variant.fields {
        VariantFields::Unit => {
            let value = variant_value(name, Vec::new());
            let tests = vec![
                Stmt::If(raw(format!("v === \"{name}\"")), Box::new(Stmt::Return(value.clone()))),
                Stmt::Block(vec![parsed, accepted(value), keyed]),
            ];
            return (arm, str_lit("null"), tests);
        }
        VariantFields::Tuple(tys) => tuple_json(WireSchema::Arktype, name, tys, "parsed"),
        VariantFields::Struct(fields) => {
            let inner = fields
                .iter()
                .map(|f| (f.name.as_str().to_string(), schema_ty_in(WireSchema::Arktype, &f.ty, true)))
                .collect();
            (Js::Object(inner), fill(WireSchema::Arktype, fields, &format!("parsed.{name}")))
        }
    };
    (arm, shape, vec![Stmt::Block(vec![parsed, accepted(variant_value(name, rest)), keyed])])
}

/// A one-element tuple variant is serde's newtype variant: `{"Add": 4}`, not
/// `{"Add": [4]}`. The domain value is still a one-element tuple.
/// The schema of a tuple variant's JSON, and the fields of its TS value
/// read from `from`: `value: x.V` for one field, `content: x.V` for several.
fn tuple_json(schema: WireSchema, variant: &str, tys: &[Ty], from: &str) -> (Js, Vec<(String, Js)>) {
    if tys.len() == 1 {
        return (schema_ty(schema, &tys[0]), vec![("value".into(), raw(format!("{from}.{variant}")))]);
    }
    let inner = Js::Array(tys.iter().map(|ty| schema_ty(schema, ty)).collect());
    let json = match schema {
        WireSchema::Zod => call_path("z.tuple", vec![inner]),
        WireSchema::Valibot => call_path("v.tuple", vec![inner]),
        WireSchema::Arktype => inner,
    };
    (json, vec![("content".into(), raw(format!("{from}.{variant}")))])
}

fn schema_ty(schema: WireSchema, ty: &Ty) -> Js {
    schema_ty_in(schema, ty, false)
}

fn never(schema: WireSchema) -> Js {
    match schema {
        WireSchema::Zod => call_path("z.never", Vec::new()),
        WireSchema::Valibot => call_path("v.never", Vec::new()),
        WireSchema::Arktype => raw("type.never"),
    }
}

fn schema_ty_in(schema: WireSchema, ty: &Ty, struct_field: bool) -> Js {
    match ty {
        Ty::Prim(p) => raw(match p {
            purecrate_ir::Prim::Bool => "bool".to_string(),
            purecrate_ir::Prim::String | purecrate_ir::Prim::Str => "str".into(),
            purecrate_ir::Prim::Char => "char".into(),
            purecrate_ir::Prim::Uuid => "uuid".into(),
            purecrate_ir::Prim::UuidError => "uuidError".into(),
            purecrate_ir::Prim::ParseIntError => "parseIntError".into(),
            purecrate_ir::Prim::Unit => "unit".into(),
            other => other
                .int()
                .map(|t| t.as_str().to_string())
                .or_else(|| {
                    other.float().map(|t| match t {
                        purecrate_ir::FloatTy::F32 => "f32".into(),
                        purecrate_ir::FloatTy::F64 => "f64".into(),
                    })
                })
                .unwrap_or_else(|| "str".into()),
        }),
        Ty::Option(inner) => {
            let inner = schema_ty(schema, inner);
            if !struct_field {
                return call_path("nullable", vec![inner]);
            }
            // A missing struct field is `None`, matching serde. The domain value is `T | null`.
            match schema {
                WireSchema::Zod => call_path("optionalField", vec![inner]),
                WireSchema::Valibot => call_path("v.optional", vec![call_path("nullable", vec![inner])]),
                WireSchema::Arktype => method(call_path("nullable", vec![inner]), "default", vec![raw("null")]),
            }
        }
        Ty::Vec(inner) => match schema {
            WireSchema::Zod => call_path("z.array", vec![schema_ty(schema, inner)]),
            WireSchema::Valibot => call_path("v.array", vec![schema_ty(schema, inner)]),
            WireSchema::Arktype => method(schema_ty(schema, inner), "array", Vec::new()),
        },
        Ty::Tuple(elems) => {
            let inner = Js::Array(elems.iter().map(|t| schema_ty(schema, t)).collect());
            match schema {
                WireSchema::Zod => call_path("z.tuple", vec![inner]),
                WireSchema::Valibot => call_path("v.tuple", vec![inner]),
                WireSchema::Arktype => call_path("type", vec![inner]),
            }
        }
        Ty::Result { .. } | Ty::Fn { .. } | Ty::Never => never(schema),
        Ty::Named(n) => raw(n.as_str()),
        Ty::Ignored { inner, .. } => schema_ty_in(schema, inner, struct_field),
    }
}

/// `toJson.T(x)`: the JSON text serde_json writes for the Rust value of `x`
/// (design/04 §6). Struct fields in declaration order; a newtype is its
/// content; enums are externally tagged, a one-field tuple variant as the
/// field itself.
fn to_json(krate: &Crate, chunks: &mut Vec<Chunk>) {
    chunks.push(Chunk::Text("\n/** Each type written as serde_json writes the Rust value. */\n".into()));
    let mut entries = Vec::new();
    for item in krate.exported().filter(|i| serde(i).ser) {
        let name = item.name().as_str();
        let body = match item {
            Item::Struct(s) => expr_body(match s.newtype_inner() {
                Some(inner) => write_json(inner, "x", 0),
                None => object_json(s.fields.iter().map(|f| (f.name.as_str(), &f.ty)), "x."),
            }),
            // A variant name is a Rust identifier: nothing in it needs escaping.
            Item::Enum(e) if e.variants.iter().all(|v| matches!(v.fields, VariantFields::Unit)) => {
                expr_body(raw("`\"${x.kind}\"`"))
            }
            Item::Enum(e) => {
                let cases = e
                    .variants
                    .iter()
                    .map(|v| {
                        let var = v.name.as_str();
                        let tagged = |inner: Js| {
                            call_path("Json.object", vec![Js::Array(vec![Js::Array(vec![str_lit(var), inner])])])
                        };
                        let value = match &v.fields {
                            VariantFields::Unit => str_lit(format!("\"{var}\"")),
                            VariantFields::Tuple(tys) if tys.len() == 1 => tagged(write_json(&tys[0], "x.value", 0)),
                            VariantFields::Tuple(tys) => {
                                let elems = tys
                                    .iter()
                                    .enumerate()
                                    .map(|(i, t)| write_json(t, &format!("x.content[{i}]"), 0))
                                    .collect();
                                tagged(call_path("Json.tuple", vec![Js::Array(elems)]))
                            }
                            VariantFields::Struct(fields) => {
                                tagged(object_json(fields.iter().map(|f| (f.name.as_str(), &f.ty)), "x."))
                            }
                        };
                        (str_lit(var), vec![Stmt::Return(value)])
                    })
                    .collect::<Vec<_>>();
                // Every variant has its case; the `default`, as in the domain
                // files, keeps every path returning.
                let mut cases = cases;
                cases.push((raw("default"), vec![Stmt::Return(call_path("assertNever", vec![raw("x")]))]));
                Body::Block(vec![Stmt::Switch(raw("x.kind"), cases)])
            }
            Item::Alias(_) | Item::Fn(_) | Item::Const(_) => continue,
        };
        entries.push((name.to_string(), arrow(&format!("(x: {name}$)"), Some("string"), body)));
    }
    chunks.push(Chunk::decl(true, "toJson", None, Js::As(Box::new(Js::Object(entries)), "const".into())));
}

/// `{"a":…,"b":…}` for fields read from `<prefix><name>`, through
/// `Json.object`, whose pairs break one per line where there are two or more.
fn object_json<'a>(fields: impl Iterator<Item = (&'a str, &'a Ty)>, prefix: &str) -> Js {
    let pairs: Vec<Js> = fields
        .map(|(name, ty)| Js::Array(vec![str_lit(name), write_json(ty, &format!("{prefix}{name}"), 0)]))
        .collect();
    if pairs.is_empty() {
        return str_lit("{}");
    }
    call_path("Json.object", vec![Js::Array(pairs)])
}

/// A TS expression for the JSON text of `value`, of type `ty`. `depth`
/// names the parameters of nested array writers apart.
fn write_json(ty: &Ty, value: &str, depth: usize) -> Js {
    use purecrate_ir::{FloatTy, Prim};
    let throws = |what: &str| {
        let body =
            Body::Block(vec![Stmt::Expr(raw(format!("throw new globalThis.Error(\"{what} has no JSON form\")")))]);
        call(Js::Paren(Box::new(arrow("()", Some("never"), body))), Vec::new())
    };
    match ty {
        Ty::Prim(p) => match p {
            Prim::Bool => call_path("Json.bool", vec![raw(value)]),
            Prim::String | Prim::Str | Prim::Char | Prim::Uuid => call_path("Json.str", vec![raw(value)]),
            // Rust cannot serialize one either.
            Prim::UuidError => throws("uuid::Error"),
            Prim::ParseIntError => throws("ParseIntError"),
            Prim::Unit => str_lit("null"),
            other => match other.float() {
                Some(FloatTy::F32) => call_path("Json.f32", vec![raw(value)]),
                Some(FloatTy::F64) => call_path("Json.f64", vec![raw(value)]),
                None => call_path("Json.int", vec![raw(value)]),
            },
        },
        Ty::Option(inner) => {
            ternary(raw(format!("{value} === null")), str_lit("null"), write_json(inner, value, depth))
        }
        Ty::Vec(inner) => {
            let v = format!("v{depth}");
            let each = arrow(&format!("({v})"), None, expr_body(write_json(inner, &v, depth + 1)));
            call_path("Json.array", vec![raw(value), each])
        }
        Ty::Tuple(elems) => {
            let parts = elems.iter().enumerate().map(|(i, t)| write_json(t, &format!("{value}[{i}]"), depth)).collect();
            call_path("Json.tuple", vec![Js::Array(parts)])
        }
        Ty::Result { ok, err } => ternary(
            raw(format!("{value}.kind === \"Ok\"")),
            raw(format!("`{{\"Ok\":{}}}`", splice(&write_json(ok, &format!("{value}.value"), depth)))),
            raw(format!("`{{\"Err\":{}}}`", splice(&write_json(err, &format!("{value}.error"), depth)))),
        ),
        Ty::Named(n) => call_path(&format!("toJson.{}", n.as_str()), vec![raw(value)]),
        Ty::Ignored { inner, .. } => write_json(inner, value, depth),
        Ty::Fn { .. } | Ty::Never => str_lit("null"),
    }
}

/// `expr` placed inside a template literal: a constant string or a nested
/// template is inlined, anything else becomes `${expr}`.
fn splice(expr: &Js) -> String {
    match expr {
        Js::Str(s) if s == "null" || s == "{}" => s.clone(),
        Js::Raw(r) if r.len() > 1 && r.starts_with('`') && r.ends_with('`') => r[1..r.len() - 1].into(),
        other => format!("${{{}}}", doc::print(&other.doc(), usize::MAX / 2, 0)),
    }
}
