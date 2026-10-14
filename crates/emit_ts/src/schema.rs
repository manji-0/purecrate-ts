//! Wire schemas for one chosen library. The shape is serde's default JSON.
//! Numeric fields use the shared `purecrate-*` adapter, so the result is the
//! domain brand (`I32`, not a bare `number`). `toJson` writes the same shape
//! back, as serde_json writes the Rust value, with no schema library.

use purecrate_ir::{Crate, Enum, Item, Serde, Struct, Tagging, Ty, Variant, VariantFields};

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

thread_local! {
    /// The type parameters of the generic item whose schema is being
    /// written: its domain type is `Page$<T>`, and `T`'s schema the value
    /// `wireT` (design/04 §3.3).
    static PARAMS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `name$`, the domain type a schema gives, with the type arguments of the
/// generic item being written (`Page$<T>`).
fn dom(name: &str) -> String {
    PARAMS.with(|p| {
        let p = p.borrow();
        if p.is_empty() {
            format!("{name}$")
        } else {
            format!("{name}$<{}>", p.join(", "))
        }
    })
}

/// The value naming a type parameter's schema (`wireT`) or writer
/// (`writeT`) inside a generic item's.
fn param_value(kind: &str, param: &str) -> String {
    format!("{kind}{param}")
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
            let generics = generics_of(item);
            // arktype: a parameter's schema is any `Type`, its value type
            // what that type reads (`WT["infer"]`), as the adapter's
            // `nullable` takes one.
            PARAMS.with(|p| *p.borrow_mut() = generics.iter().map(|g| ark_infer(g)).collect());
            let mut own = Vec::new();
            match item {
                Item::Struct(s) => ark_struct(s, &refusal(krate, s), &doc_of(item), &mut own),
                Item::Enum(e) if e.tagging != Tagging::External => ark_tagged(e, &doc_of(item), &mut own),
                Item::Enum(e) => ark_enum(e, &doc_of(item), &mut own),
                _ => {}
            }
            if generics.is_empty() {
                chunks.extend(own);
            } else {
                chunks.extend(generic_ark(item.name().as_str(), &generics, own));
            }
            PARAMS.with(|p| p.borrow_mut().clear());
        }
    } else {
        for (item, recursive) in wire_order(&wired) {
            let generics = generics_of(item);
            PARAMS.with(|p| *p.borrow_mut() = generics.clone());
            let value = match item {
                Item::Struct(s) => struct_schema(schema, s, &refusal(krate, s)),
                Item::Enum(e) if e.tagging != Tagging::External => tagged_schema(schema, e),
                Item::Enum(e) => enum_schema(schema, e.name.as_str(), &e.variants),
                _ => continue,
            };
            chunks.push(Chunk::Text(format!("\n{}", doc_of(item))));
            let name = item.name().as_str();
            let (ty, value) = declare(schema, name, value, recursive);
            if generics.is_empty() {
                chunks.push(Chunk::decl(true, name, Some(ty), value));
            } else {
                // `<T>(wireT: z.ZodType<T>): z.ZodType<Page$<T>> => ..`: a
                // schema of the parameters' schemas.
                let param_ty = |g: &str| match schema {
                    WireSchema::Zod => format!("z.ZodType<{g}>"),
                    WireSchema::Valibot => format!("v.GenericSchema<unknown, {g}>"),
                    WireSchema::Arktype => unreachable!("arktype is printed above"),
                };
                let params = generics
                    .iter()
                    .map(|g| format!("{}: {}", param_value("wire", g), param_ty(g)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let head = format!("<{}>({params})", generics.join(", "));
                chunks.push(Chunk::decl(true, name, None, arrow(&head, Some(&ty), expr_body(value))));
            }
            PARAMS.with(|p| p.borrow_mut().clear());
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
    super::imports::prune_unused(&render(&chunks))
}

/// What an arktype parameter schema `W<g>` reads: `WT["infer"]`.
fn ark_infer(g: &str) -> String {
    format!("W{g}[\"infer\"]")
}

/// A struct's or enum's type parameters.
fn generics_of(item: &Item) -> Vec<String> {
    let names = match item {
        Item::Struct(s) => &s.generics,
        Item::Enum(e) => &e.generics,
        _ => return Vec::new(),
    };
    names.iter().map(|n| n.as_str().to_string()).collect()
}

/// A generic item's arktype declarations as one function of its
/// parameters' schemas: each shape a `const` inside it, the morph returned.
fn generic_ark(name: &str, generics: &[String], own: Vec<Chunk>) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut body: Vec<Stmt> = generics
        .iter()
        .map(|g| {
            Stmt::Const(
                param_value("anyWire", g),
                Js::As(Box::new(raw(param_value("wire", g))), "Type<unknown>".into()),
            )
        })
        .collect();
    let mut result = None;
    for c in own {
        match c {
            Chunk::Text(t) if t.trim().is_empty() => {}
            Chunk::Text(t) => out.push(Chunk::Text(t)),
            Chunk::Decl { name: n, value, .. } if n == name => result = Some(value),
            Chunk::Decl { name: n, value, .. } => body.push(Stmt::Const(n, value)),
        }
    }
    // arktype infers what a parameter's schema reads as `_distill<T, "out">`,
    // which TS does not reduce to `T` inside the function: the morph's own
    // return type (`Page$<T>`) states it, each value it returns `as never`.
    let mut result = result.expect("the item's own declaration");
    as_never_in(&mut result);
    body.push(Stmt::Return(result));
    let params = generics.iter().map(|g| format!("{}: W{g}", param_value("wire", g))).collect::<Vec<_>>().join(", ");
    let head = format!(
        "<{}>({params})",
        generics.iter().map(|g| format!("W{g} extends Type<any>")).collect::<Vec<_>>().join(", ")
    );
    out.insert(0, Chunk::Text("\n".into()));
    out.push(Chunk::decl(true, name, None, arrow(&head, Some(&format!("Wire<{}>", dom(name))), Body::Block(body))));
    out
}

/// Each value a morph's arrow returns, as `never` (`generic_ark`).
fn as_never_in(js: &mut Js) {
    fn stmts(ss: &mut [Stmt]) {
        for s in ss {
            match s {
                Stmt::Return(v) => {
                    as_never_in(v);
                    // `fail(ctx, parsed)` is already `never`.
                    let fails = matches!(v, Js::Call(f, _) if matches!(&**f, Js::Raw(n) if n == "fail"));
                    if !fails && !matches!(v, Js::As(_, t) if t == "never") {
                        let inner = std::mem::replace(v, raw(""));
                        *v = Js::As(Box::new(inner), "never".into());
                    }
                }
                Stmt::If(_, s) => stmts(std::slice::from_mut(&mut **s)),
                Stmt::IfBlock(_, b) | Stmt::Block(b) => stmts(b),
                Stmt::Switch(_, cases) => cases.iter_mut().for_each(|(_, b)| stmts(b)),
                Stmt::Const(_, v) | Stmt::Expr(v) => as_never_in(v),
            }
        }
    }
    match js {
        Js::Arrow(a) if a.ret.is_some() => match &mut a.body {
            Body::Block(b) => stmts(b),
            Body::Expr(e) => {
                let inner = std::mem::replace(&mut **e, raw(""));
                **e = Js::As(Box::new(inner), "never".into());
            }
        },
        Js::Call(f, args) => {
            as_never_in(f);
            args.iter_mut().for_each(as_never_in);
        }
        Js::Member(o, _) | Js::As(o, _) | Js::Paren(o) => as_never_in(o),
        Js::Array(xs) => xs.iter_mut().for_each(as_never_in),
        Js::Object(kv) => kv.iter_mut().for_each(|(_, v)| as_never_in(v)),
        Js::Ternary(a, b, c) => {
            as_never_in(a);
            as_never_in(b);
            as_never_in(c);
        }
        Js::Arrow(_) | Js::Raw(_) | Js::Str(_) => {}
    }
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
            let generics = generics_of(item);
            PARAMS.with(|p| {
                *p.borrow_mut() = if schema == WireSchema::Arktype {
                    generics.iter().map(|g| ark_infer(g)).collect()
                } else {
                    generics.clone()
                }
            });
            let text = call_path("parseJson", vec![raw("text")]);
            // A generic type's schema is made of its parameters' schemas.
            let schema_of = if generics.is_empty() {
                raw(name)
            } else {
                call_path(name, generics.iter().map(|g| raw(param_value("wire", g))).collect())
            };
            let read = match schema {
                WireSchema::Zod => method(schema_of, "parse", vec![text]),
                WireSchema::Valibot => call_path("v.parse", vec![schema_of, text]),
                WireSchema::Arktype => method(schema_of, "assert", vec![text]),
            };
            let head = if generics.is_empty() {
                "(text: string)".to_string()
            } else {
                let param_ty = |g: &str| match schema {
                    WireSchema::Zod => format!("z.ZodType<{g}>"),
                    WireSchema::Valibot => format!("v.GenericSchema<unknown, {g}>"),
                    WireSchema::Arktype => format!("W{g}"),
                };
                let params =
                    generics.iter().map(|g| format!("{}: {}", param_value("wire", g), param_ty(g))).collect::<Vec<_>>();
                let heads = if schema == WireSchema::Arktype {
                    generics.iter().map(|g| format!("W{g} extends Type<any>")).collect::<Vec<_>>().join(", ")
                } else {
                    generics.join(", ")
                };
                format!("<{heads}>(text: string, {})", params.join(", "))
            };
            let entry = (name.to_string(), arrow(&head, Some(&dom(name)), expr_body(read)));
            PARAMS.with(|p| p.borrow_mut().clear());
            entry
        })
        .collect();
    chunks.push(Chunk::decl(true, "fromJson", None, Js::As(Box::new(Js::Object(entries)), "const".into())));
}

/// The types an item's schema reads by name.
fn named_in(item: &Item) -> Vec<String> {
    fn walk(ty: &Ty, out: &mut Vec<String>) {
        match ty {
            Ty::Named(n) => out.push(n.as_str().to_string()),
            Ty::App { name, args } => {
                out.push(name.as_str().to_string());
                args.iter().for_each(|t| walk(t, out));
            }
            // A function is not on the wire.
            Ty::Fn { .. } => {}
            ty => ty.children().into_iter().for_each(|t| walk(t, out)),
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
        WireSchema::Zod => (format!("z.ZodType<{}>", dom(name)), "z.lazy"),
        WireSchema::Valibot => (format!("v.GenericSchema<unknown, {}>", dom(name)), "v.lazy"),
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

/// A JSON name as an object literal's key: as it is where it is an
/// identifier, else quoted (`rename` may give any string).
fn prop(wire: &str) -> String {
    if is_ident(wire) {
        wire.to_string()
    } else {
        js_string(wire)
    }
}

/// `from.wire`, or `from["wire"]` where the name is no identifier.
fn access(from: &str, wire: &str) -> String {
    if is_ident(wire) {
        format!("{from}.{wire}")
    } else {
        format!("{from}[{}]", js_string(wire))
    }
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// A TS string literal of `s`: JSON's escapes are TS's.
fn js_string(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

/// `s` inside a JSON string as serde_json writes it: `"` and `\` escaped,
/// the short escapes for `\b \f \n \r \t`, any other control character as
/// `\u00XX`; everything else as it is.
fn json_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// An arktype object key: a trailing `?` marks the key optional there, so
/// one that is part of the name is escaped.
fn ark_prop(wire: &str) -> String {
    match wire.strip_suffix('?') {
        Some(stem) => js_string(&format!("{stem}\\?")),
        None => prop(wire),
    }
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
import { adjacentlyTagged, bool, btreeMap, btreeSet, char, f32, f64, i128, i16, i32, i64, i8, internallyTagged, nullable, optionalField, record, str, u128, u16, u32, u64, u8, unit, unitEnum, unitVariant, usize, uuid, uuidError, variant } from \"purecrate-zod\";
",
        WireSchema::Valibot => "\
import * as v from \"valibot\";
import { adjacentlyTagged, bool, btreeMap, btreeSet, char, f32, f64, i128, i16, i32, i64, i8, internallyTagged, nullable, record, str, u128, u16, u32, u64, u8, unit, unitEnum, unitVariant, usize, uuid, uuidError, variant } from \"purecrate-valibot\";
",
        WireSchema::Arktype => "\
import { type, type Type } from \"arktype\";
import { adjacentlyTagged, bool, btreeMap, btreeSet, char, f32, f64, fail, i128, i16, i32, i64, i8, internallyTagged, keyed, memo, nullable, record, sequence, str, u128, u16, u32, u64, u8, unit, unitEnum, usize, uuid, uuidError, type Wire } from \"purecrate-arktype\";
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
        let build = arrow("(x)", Some(&dom(name)), expr_body(newtype_build(s, "x")));
        return match schema {
            WireSchema::Zod => method(value, "transform", vec![build]),
            WireSchema::Valibot => call_path("v.pipe", vec![value, call_path("v.transform", vec![build])]),
            WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
        };
    }
    let fields = Js::Object(object_fields(schema, &s.fields));
    // The object's own output is the domain value, but for a closed struct
    // (built through its constructor), a `()` field (which inference makes
    // optional), a field whose JSON name is not its own, or with valibot an
    // `Option` field (missing reads as
    // `undefined`, which `?? null` turns into `None`). Both libraries drop
    // unknown keys, as serde does.
    let unit = |t: &Ty| {
        matches!(t, Ty::Prim(purecrate_ir::Prim::Unit))
            || matches!(t, Ty::Option(i) | Ty::Ignored { inner: i, .. } if matches!(**i, Ty::Prim(purecrate_ir::Prim::Unit)))
    };
    let built = s.closed
        || s.fields.iter().any(|f| unit(&f.ty) || f.wire.is_some())
        || (schema == WireSchema::Valibot && s.fields.iter().any(|f| matches!(f.ty, Ty::Option(_))));
    // valibot's `v.object` takes an array too (each field missing); `record`
    // refuses one, as zod's object does.
    let object = call_path("record", vec![fields]);
    if !built {
        return object;
    }
    let build = arrow("(x)", Some(&dom(name)), expr_body(record_build(s, fill(schema, &s.fields, "x"))));
    match schema {
        WireSchema::Zod => method(object, "transform", vec![build]),
        WireSchema::Valibot => call_path("v.pipe", vec![object, call_path("v.transform", vec![build])]),
        WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
    }
}

/// `name: schema` for each field of a struct or struct variant, keyed by
/// its JSON name.
fn object_fields(schema: WireSchema, fields: &[purecrate_ir::Field]) -> Vec<(String, Js)> {
    let key = |w: &str| if schema == WireSchema::Arktype { ark_prop(w) } else { prop(w) };
    fields.iter().map(|f| (key(f.wire_name()), schema_ty_in(schema, &f.ty, true))).collect()
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
    let ret = dom(name);
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
/// the domain value, each read under its JSON name. Valibot leaves a
/// missing optional field `undefined`.
fn fill(schema: WireSchema, fields: &[purecrate_ir::Field], from: &str) -> Vec<(String, Js)> {
    fields
        .iter()
        .map(|f| {
            let n = f.name.as_str();
            let at = access(from, f.wire_name());
            let read = if schema == WireSchema::Valibot && matches!(f.ty, Ty::Option(_)) {
                raw(format!("{at} ?? null"))
            } else {
                raw(at)
            };
            (n.to_string(), read)
        })
        .collect()
}

/// The names of a fieldless enum's variants, or `None` when one has
/// fields or a JSON name of its own: the adapters' `unitEnum` reads it
/// whole, each name both the JSON and the `kind`.
fn unit_names(variants: &[purecrate_ir::Variant]) -> Option<Vec<Js>> {
    variants
        .iter()
        .map(|v| (matches!(v.fields, VariantFields::Unit) && v.wire.is_none()).then(|| str_lit(v.name.as_str())))
        .collect()
}

fn enum_schema(schema: WireSchema, name: &str, variants: &[purecrate_ir::Variant]) -> Js {
    if let Some(names) = unit_names(variants) {
        return call_path("unitEnum", vec![Js::Array(names)]);
    }
    let arms = variants.iter().map(|v| variant_arm(schema, name, v)).collect();
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
fn variant_arm(schema: WireSchema, enum_name: &str, v: &purecrate_ir::Variant) -> Js {
    let ret = dom(enum_name);
    let (variant, wire) = (v.name.as_str(), v.wire_name());
    let (wrapper, rest) = match &v.fields {
        VariantFields::Unit => {
            let value = arrow("()", Some(&ret), expr_body(variant_value(variant, Vec::new())));
            let tag = call_path("unitVariant", vec![str_lit(wire)]);
            return match schema {
                WireSchema::Zod => method(tag, "transform", vec![value]),
                WireSchema::Valibot => call_path("v.pipe", vec![tag, call_path("v.transform", vec![value])]),
                WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
            };
        }
        VariantFields::Tuple(tys) => tuple_json(schema, &access("x", wire), tys),
        VariantFields::Struct(fs) => {
            let fields = Js::Object(object_fields(schema, fs));
            let inner = call_path("record", vec![fields]);
            (inner, fill(schema, fs, &access("x", wire)))
        }
    };
    let value = arrow("(x)", Some(&ret), expr_body(variant_value(variant, rest)));
    let keyed = Js::Object(vec![(prop(wire), wrapper)]);
    match schema {
        WireSchema::Zod => method(call_path("variant", vec![keyed]), "transform", vec![value]),
        WireSchema::Valibot => {
            call_path("v.pipe", vec![call_path("variant", vec![keyed]), call_path("v.transform", vec![value])])
        }
        WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
    }
}

/// An internally or adjacently tagged enum (design/04 §3.6): the adapter's
/// `internallyTagged` / `adjacentlyTagged` finds the variant by its JSON
/// name; `units` maps each unit variant's to its `kind`, and each other
/// variant has an arm reading what serde passes it: the object (or the rest
/// of the sequence form) when the tag is internal, the content when it is
/// adjacent.
fn tagged_schema(schema: WireSchema, en: &Enum) -> Js {
    let ret = dom(en.name.as_str());
    let arms = en
        .variants
        .iter()
        .filter_map(|v| {
            let (shape, value) = tagged_arm(schema, v)?;
            let build = arrow("(x)", Some(&ret), expr_body(value));
            let arm = match schema {
                WireSchema::Zod => method(shape, "transform", vec![build]),
                WireSchema::Valibot => call_path("v.pipe", vec![shape, call_path("v.transform", vec![build])]),
                WireSchema::Arktype => unreachable!("arktype tagged enums are printed by `ark_tagged`"),
            };
            Some((prop(v.wire_name()), arm))
        })
        .collect();
    let mut args = vec![Js::Object(arms)];
    args.splice(0..0, tagged_head(en));
    args.extend(tagged_tail(en));
    let helper = if matches!(en.tagging, Tagging::Internal { .. }) { "internallyTagged" } else { "adjacentlyTagged" };
    call_path(helper, args)
}

/// The tag (and content) keys, then `units`.
fn tagged_head(en: &Enum) -> Vec<Js> {
    let units = Js::Object(
        en.variants
            .iter()
            .filter(|v| matches!(v.fields, VariantFields::Unit))
            .map(|v| (prop(v.wire_name()), str_lit(v.name.as_str())))
            .collect(),
    );
    match &en.tagging {
        Tagging::Internal { tag } => vec![str_lit(tag), units],
        Tagging::Adjacent { tag, content } => vec![str_lit(tag), str_lit(content), units],
        Tagging::External => unreachable!("an external tag has no head"),
    }
}

/// An adjacently tagged enum's struct variants, whose content is an
/// object only, and its newtype variants of an `Option`, whose content may
/// be left out; nothing when both are empty, as the adapter defaults them.
fn tagged_tail(en: &Enum) -> Vec<Js> {
    if !matches!(en.tagging, Tagging::Adjacent { .. }) {
        return Vec::new();
    }
    let names = |keep: &dyn Fn(&Variant) -> bool| {
        Js::Array(en.variants.iter().filter(|v| keep(v)).map(|v| str_lit(v.wire_name())).collect())
    };
    let maps = names(&|v| matches!(v.fields, VariantFields::Struct(_)));
    let optional =
        names(&|v| matches!(&v.fields, VariantFields::Tuple(tys) if matches!(tys.as_slice(), [Ty::Option(_)])));
    match (&maps, &optional) {
        (Js::Array(m), Js::Array(o)) if m.is_empty() && o.is_empty() => Vec::new(),
        (_, Js::Array(o)) if o.is_empty() => vec![maps],
        _ => vec![maps, optional],
    }
}

/// A non-unit variant's shape and the domain value built from what it read
/// (`x`); `None` for a unit variant.
fn tagged_arm(schema: WireSchema, v: &Variant) -> Option<(Js, Js)> {
    let kind = v.name.as_str();
    match &v.fields {
        VariantFields::Unit => None,
        VariantFields::Struct(fs) => {
            let shape = match schema {
                WireSchema::Arktype => ark_record(fs),
                _ => call_path("record", vec![Js::Object(object_fields(schema, fs))]),
            };
            Some((shape, variant_value(kind, fill(schema, fs, "x"))))
        }
        VariantFields::Tuple(tys) => {
            let (shape, rest) = tuple_json(schema, "x", tys);
            let shape = match (schema, tys.len()) {
                (WireSchema::Arktype, n) if n > 1 => call_path("type", vec![shape]),
                _ => shape,
            };
            Some((shape, variant_value(kind, rest)))
        }
    }
}

/// Arktype: each non-unit variant's arm a memoized shape piped into the
/// domain value, then the adapter's helper over them (`tagged_schema`).
fn ark_tagged(en: &Enum, doc: &str, chunks: &mut Vec<Chunk>) {
    let name = en.name.as_str();
    let ret = dom(name);
    chunks.push(Chunk::Text("\n".into()));
    let mut arms = Vec::new();
    for v in &en.variants {
        let Some((shape, value)) = tagged_arm(WireSchema::Arktype, v) else { continue };
        let arm = format!("{name}$arm${}", v.name.as_str());
        let piped = method(shape, "pipe", vec![arrow("(x)", Some(&ret), expr_body(value))]);
        chunks.push(Chunk::decl(false, arm.clone(), None, memo(piped)));
        arms.push((prop(v.wire_name()), raw(arm)));
    }
    let mut args = vec![str_lit(name)];
    args.extend(tagged_head(en));
    args.push(Js::Object(arms));
    args.extend(tagged_tail(en));
    let helper = if matches!(en.tagging, Tagging::Internal { .. }) { "internallyTagged" } else { "adjacentlyTagged" };
    chunks.push(Chunk::Text(doc.to_string()));
    // The declared `Wire<E$>` gives the helper's type parameter.
    chunks.push(Chunk::decl(true, name, Some(format!("Wire<{ret}>")), call_path(helper, args)));
}

/// `type("unknown").pipe((v, ctx): T$ => { .. })`, an arktype morph whose
/// body starts by reading `wire` into `parsed`.
fn ark_morph(name: &str, body: Vec<Stmt>) -> Js {
    let pipe = arrow("(v, ctx)", Some(&dom(name)), Body::Block(body));
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

/// A struct's arktype shape: the sequence form read in field order and a
/// field twice refused (`sequence`), then the object, not an array
/// (`record`; an arktype object shape takes an array too).
fn ark_record(fields: &[purecrate_ir::Field]) -> Js {
    let names = Js::Array(fields.iter().map(|f| str_lit(f.wire_name())).collect());
    let object = method(
        call_path("type", vec![Js::Object(object_fields(WireSchema::Arktype, fields))]),
        "narrow",
        vec![raw("record")],
    );
    method(call_path("type", vec![str_lit("unknown")]), "pipe", vec![call_path("sequence", vec![names]), object])
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
            let build = record_build(s, fill(WireSchema::Arktype, &s.fields, "parsed"));
            let shape = ark_record(&s.fields);
            (shape, vec![Stmt::Return(build)])
        }
    };
    let mut body = ark_parsed(&wire);
    body.extend(tail);
    chunks.push(Chunk::Text("\n".into()));
    chunks.push(Chunk::decl(false, wire, None, memo(shape)));
    chunks.push(Chunk::Text(doc.to_string()));
    chunks.push(Chunk::decl(true, name, Some(format!("Wire<{}>", dom(name))), ark_morph(name, body)));
}

fn ark_enum(en: &purecrate_ir::Enum, doc: &str, chunks: &mut Vec<Chunk>) {
    let name = en.name.as_str();
    chunks.push(Chunk::Text("\n".into()));
    if let Some(names) = unit_names(&en.variants) {
        chunks.push(Chunk::Text(doc.to_string()));
        let value = call_path("unitEnum", vec![str_lit(name), Js::Array(names)]);
        chunks.push(Chunk::decl(true, name, Some(format!("Wire<{}>", dom(name))), value));
        return;
    }
    let mut body = Vec::new();
    for variant in &en.variants {
        let (arm, shape, tests) = ark_variant(name, variant);
        let keyed = Js::Object(vec![("\"+\"".into(), str_lit("reject")), (ark_prop(variant.wire_name()), shape)]);
        // A key twice in the wrapper is refused before the shape reads it.
        let wrapper = method(
            call_path("type", vec![str_lit("unknown")]),
            "pipe",
            vec![call_path("sequence", Vec::new()), call_path("type", vec![keyed])],
        );
        chunks.push(Chunk::decl(false, arm, None, memo(wrapper)));
        body.extend(tests);
    }
    body.push(Stmt::Return(Js::As(Box::new(call_path("ctx.error", vec![str_lit(name)])), "never".into())));
    chunks.push(Chunk::Text(doc.to_string()));
    let pipe = arrow("(v, ctx)", Some(&dom(name)), Body::Block(body));
    let value = method(call_path("type", vec![str_lit("unknown")]), "pipe", vec![pipe]);
    chunks.push(Chunk::decl(true, name, Some(format!("Wire<{}>", dom(name))), value));
}

/// One variant's memoized shape: its name, the JSON under the variant's key,
/// and the statements that try it.
fn ark_variant(en: &str, variant: &purecrate_ir::Variant) -> (String, Js, Vec<Stmt>) {
    let (name, wire) = (variant.name.as_str(), variant.wire_name());
    let arm = format!("{en}$arm${name}");
    let parsed = Stmt::Const("parsed".into(), call(call(raw(arm.clone()), Vec::new()), vec![raw("v")]));
    let keyed = Stmt::If(
        call_path("keyed", vec![raw("v"), str_lit(wire)]),
        Box::new(Stmt::Return(call_path("fail", vec![raw("ctx"), raw("parsed")]))),
    );
    let accepted = |value: Js| Stmt::If(raw("!(parsed instanceof type.errors)"), Box::new(Stmt::Return(value)));
    let (shape, rest) = match &variant.fields {
        VariantFields::Unit => {
            let value = variant_value(name, Vec::new());
            let tests = vec![
                Stmt::If(raw(format!("v === {}", js_string(wire))), Box::new(Stmt::Return(value.clone()))),
                Stmt::Block(vec![parsed, accepted(value), keyed]),
            ];
            return (arm, str_lit("null"), tests);
        }
        VariantFields::Tuple(tys) => tuple_json(WireSchema::Arktype, &access("parsed", wire), tys),
        VariantFields::Struct(fields) => {
            (ark_record(fields), fill(WireSchema::Arktype, fields, &access("parsed", wire)))
        }
    };
    (arm, shape, vec![Stmt::Block(vec![parsed, accepted(variant_value(name, rest)), keyed])])
}

/// A one-element tuple variant is serde's newtype variant: `{"Add": 4}`, not
/// `{"Add": [4]}`. The domain value is still a one-element tuple.
/// The schema of a tuple variant's JSON, and the fields of its TS value
/// read from `at`: `value: x.V` for one field, `content: x.V` for several.
fn tuple_json(schema: WireSchema, at: &str, tys: &[Ty]) -> (Js, Vec<(String, Js)>) {
    if tys.len() == 1 {
        return (schema_ty(schema, &tys[0]), vec![("value".into(), raw(at))]);
    }
    let inner = Js::Array(tys.iter().map(|ty| schema_ty(schema, ty)).collect());
    let json = match schema {
        WireSchema::Zod => call_path("z.tuple", vec![inner]),
        WireSchema::Valibot => call_path("v.tuple", vec![inner]),
        WireSchema::Arktype => inner,
    };
    (json, vec![("content".into(), raw(at))])
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
        // `Page(User)`: a generic type's schema of its arguments' schemas.
        Ty::App { name, args } => call_path(name.as_str(), args.iter().map(|t| schema_ty(schema, t)).collect()),
        // arktype validates a definition at the type level, which an
        // unresolved `WT` does not pass: inside the function it reads the
        // schema as a `Type<unknown>` (`generic_ark`).
        Ty::Param(p) if schema == WireSchema::Arktype => raw(param_value("anyWire", p.as_str())),
        Ty::Param(p) => raw(param_value("wire", p.as_str())),
        // An object read into a `Map`, an array into a `Set` (the adapters).
        Ty::Map { value, .. } => call_path("btreeMap", vec![schema_ty(schema, value)]),
        Ty::Set(_) => raw("btreeSet"),
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
        let generics = generics_of(item);
        PARAMS.with(|p| *p.borrow_mut() = generics.clone());
        let body = match item {
            Item::Struct(s) => expr_body(match s.newtype_inner() {
                Some(inner) => write_json(inner, "x", 0),
                None => object_json(&s.fields, "x."),
            }),
            // A variant name is a Rust identifier: nothing in it needs escaping.
            Item::Enum(e)
                if e.tagging == Tagging::External
                    && e.variants.iter().all(|v| matches!(v.fields, VariantFields::Unit) && v.wire.is_none()) =>
            {
                expr_body(raw("`\"${x.kind}\"`"))
            }
            Item::Enum(e) if e.tagging != Tagging::External => tagged_json(e),
            Item::Enum(e) => {
                let cases = e
                    .variants
                    .iter()
                    .map(|v| {
                        let (var, wire) = (v.name.as_str(), json_escape(v.wire_name()));
                        let tagged = |inner: Js| {
                            call_path("Json.object", vec![Js::Array(vec![Js::Array(vec![str_lit(&wire), inner])])])
                        };
                        let value = match &v.fields {
                            VariantFields::Unit => str_lit(format!("\"{wire}\"")),
                            VariantFields::Tuple(tys) if tys.len() == 1 => tagged(write_json(&tys[0], "x.value", 0)),
                            VariantFields::Tuple(tys) => {
                                let elems = tys
                                    .iter()
                                    .enumerate()
                                    .map(|(i, t)| write_json(t, &format!("x.content[{i}]"), 0))
                                    .collect();
                                tagged(call_path("Json.tuple", vec![Js::Array(elems)]))
                            }
                            VariantFields::Struct(fields) => tagged(object_json(fields, "x.")),
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
        // An empty struct writes `{}` without reading its value.
        let x = if reads(&arrow("()", None, body.clone()), "x") { "x" } else { "_x" };
        // A generic type's writer takes one per type parameter
        // (`writeT: (v: T) => string`), unread where nothing holds a `T`.
        let head = if generics.is_empty() {
            format!("({x}: {})", dom(name))
        } else {
            let writers = generics
                .iter()
                .map(|g| {
                    let w = param_value("write", g);
                    let w = if reads(&arrow("()", None, body.clone()), &w) { w } else { format!("_{w}") };
                    format!("{w}: (v: {g}) => string")
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("<{}>({x}: {}, {writers})", generics.join(", "), dom(name))
        };
        entries.push((name.to_string(), arrow(&head, Some("string"), body)));
        PARAMS.with(|p| p.borrow_mut().clear());
    }
    chunks.push(Chunk::decl(true, "toJson", None, Js::As(Box::new(Js::Object(entries)), "const".into())));
}

/// `toJson` of a tagged enum, as serde writes one: the tag first, then,
/// when it is internal, the fields of a struct variant or of the struct a
/// newtype variant holds; when it is adjacent, the content under its key
/// (none for a unit variant).
fn tagged_json(en: &Enum) -> Body {
    let mut cases = Vec::new();
    for v in &en.variants {
        let tag_value = format!("\"{}\"", json_escape(v.wire_name()));
        let value = match &en.tagging {
            Tagging::Internal { tag } => {
                let tag = json_escape(tag);
                let head = Js::Array(vec![str_lit(&tag), str_lit(&tag_value)]);
                match &v.fields {
                    VariantFields::Unit => str_lit(format!("{{\"{tag}\":{tag_value}}}")),
                    VariantFields::Struct(fs) => {
                        let mut pairs = vec![head];
                        if let Js::Call(_, args) = object_json(fs, "x.") {
                            if let Some(Js::Array(fields)) = args.into_iter().next() {
                                pairs.extend(fields);
                            }
                        }
                        call_path("Json.object", vec![Js::Array(pairs)])
                    }
                    VariantFields::Tuple(tys) => call_path(
                        "Json.withTag",
                        vec![str_lit(&tag), str_lit(&tag_value), write_json(&tys[0], "x.value", 0)],
                    ),
                }
            }
            Tagging::Adjacent { tag, content } => {
                let head = Js::Array(vec![str_lit(json_escape(tag)), str_lit(&tag_value)]);
                let inner = match &v.fields {
                    VariantFields::Unit => None,
                    VariantFields::Struct(fs) => Some(object_json(fs, "x.")),
                    VariantFields::Tuple(tys) if tys.len() == 1 => Some(write_json(&tys[0], "x.value", 0)),
                    VariantFields::Tuple(tys) => Some(call_path(
                        "Json.tuple",
                        vec![Js::Array(
                            tys.iter().enumerate().map(|(i, t)| write_json(t, &format!("x.content[{i}]"), 0)).collect(),
                        )],
                    )),
                };
                let mut pairs = vec![head];
                pairs.extend(inner.map(|c| Js::Array(vec![str_lit(json_escape(content)), c])));
                call_path("Json.object", vec![Js::Array(pairs)])
            }
            Tagging::External => unreachable!("an external tag is written by `to_json`"),
        };
        cases.push((str_lit(v.name.as_str()), vec![Stmt::Return(value)]));
    }
    cases.push((raw("default"), vec![Stmt::Return(call_path("assertNever", vec![raw("x")]))]));
    Body::Block(vec![Stmt::Switch(raw("x.kind"), cases)])
}

/// `{"a":…,"b":…}` for fields read from `<prefix><name>`, each under its
/// JSON name, escaped as serde_json escapes it (`Json.object` writes the
/// key as given), through `Json.object`, whose pairs break one per line
/// where there are two or more.
fn object_json(fields: &[purecrate_ir::Field], prefix: &str) -> Js {
    let pairs: Vec<Js> = fields
        .iter()
        .map(|f| {
            let value = write_json(&f.ty, &format!("{prefix}{}", f.name.as_str()), 0);
            Js::Array(vec![str_lit(json_escape(f.wire_name())), value])
        })
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
        // `toJson.Page(x, (v0) => ..)`: a generic type's writer, given one
        // per type argument.
        Ty::App { name, args } => {
            let v = format!("v{depth}");
            let writers = args.iter().map(|t| {
                let item = write_json(t, &v, depth + 1);
                let param =
                    if reads(&arrow("()", None, expr_body(item.clone())), &v) { v.clone() } else { format!("_{v}") };
                arrow(&format!("({param})"), None, expr_body(item))
            });
            call_path(&format!("toJson.{}", name.as_str()), std::iter::once(raw(value)).chain(writers).collect())
        }
        Ty::Param(p) => call_path(&param_value("write", p.as_str()), vec![raw(value)]),
        // An object in key order, as serde_json writes a `BTreeMap`.
        Ty::Map { value: inner, .. } => {
            let v = format!("v{depth}");
            let item = write_json(inner, &v, depth + 1);
            let param = if reads(&arrow("()", None, expr_body(item.clone())), &v) { v } else { format!("_{v}") };
            call_path("Json.map", vec![raw(value), arrow(&format!("({param})"), None, expr_body(item))])
        }
        Ty::Set(_) => call_path("Json.set", vec![raw(value)]),
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
            let item = write_json(inner, &v, depth + 1);
            // `Vec<()>` writes `null` for each item without reading it.
            let param = if reads(&arrow("()", None, expr_body(item.clone())), &v) { v } else { format!("_{v}") };
            let each = arrow(&format!("({param})"), None, expr_body(item));
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

/// Whether the printed `js` names `name` as an identifier.
fn reads(js: &Js, name: &str) -> bool {
    let text = doc::print(&js.doc(), usize::MAX / 2, 0);
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(ident) && !text[at + name.len()..].chars().next().is_some_and(ident)
    })
}
