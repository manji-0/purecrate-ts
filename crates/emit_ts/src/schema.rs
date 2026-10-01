//! Wire schemas for one chosen library. The shape is serde's default JSON.
//! Numeric fields use the shared `purecrate-*` adapter, so the result is the
//! domain brand (`I32`, not a bare `number`). `toJson` writes the same shape
//! back, as serde_json writes the Rust value, with no schema library.

use purecrate_ir::{Crate, Item, Serde, Struct, Ty, VariantFields};

use crate::closed_ctor;

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
    let mut out = String::from(super::HEADER);
    out.push('\n');
    out.push_str(&header(schema));
    for item in krate.exported() {
        if serde(item).any() {
            let name = item.name().as_str();
            let value = matches!(item, Item::Struct(s) if (s.newtype_inner().is_some() && !s.closed) || s.wire_from.is_some());
            if matches!(item, Item::Struct(s) if s.closed && s.wire_from.is_some()) {
                out.push_str(&format!(
                    "import {{ {name} as {name}$value, type {name} as {name}$ }} from \"./{}.ts\";\n",
                    item.file_stem()
                ));
            } else if matches!(item, Item::Struct(s) if s.closed) {
                out.push_str(&format!(
                    "import {{ {ctor}, type {name} as {name}$ }} from \"./{}.ts\";\n",
                    item.file_stem(),
                    ctor = closed_ctor(name)
                ));
            } else if value {
                out.push_str(&format!(
                    "import {{ {name} as {name}$value, type {name} as {name}$ }} from \"./{}.ts\";\n",
                    item.file_stem()
                ));
            } else {
                out.push_str(&format!(
                    "import type {{ {name} as {name}$ }} from \"./{}.ts\";\n",
                    item.file_stem()
                ));
            }
        }
    }
    let wired: Vec<&Item> = krate.exported().filter(|i| serde(i).de).collect();
    out.push_str(&refusal_imports(krate, &wired));
    if schema == WireSchema::Arktype {
        // Arktype compiles each shape on first use (`memo`), so any order works.
        for item in &wired {
            let printed = match item {
                Item::Struct(s) => ark_struct(s, &refusal(krate, s)),
                Item::Enum(e) => ark_enum(e),
                _ => continue,
            };
            out.push_str(&documented(item, &printed));
        }
    } else {
        for (item, recursive) in wire_order(&wired) {
            let printed = match item {
                Item::Struct(s) => struct_schema(schema, s, recursive, &refusal(krate, s)),
                Item::Enum(e) => enum_schema(schema, e.name.as_str(), &e.variants, recursive),
                _ => continue,
            };
            out.push_str(&documented(item, &printed));
        }
    }
    out.push_str(&from_json(schema, &wired));
    out.push_str(&to_json(krate));
    crate::tidy::wrap(&super::imports::prune_unused(&out), crate::WIDTH)
}

/// `printed` with the type's `///` comment as JSDoc on its schema, so a
/// hover on the schema reads what the type is.
fn documented(item: &Item, printed: &str) -> String {
    let doc = match item {
        Item::Struct(s) => &s.doc,
        Item::Enum(e) => &e.doc,
        _ => return printed.to_string(),
    };
    let head = format!("export const {}:", item.name().as_str());
    match printed.find(&head) {
        Some(at) if doc.is_some() => format!("{}{}{}", &printed[..at], crate::items::jsdoc(doc, ""), &printed[at..]),
        _ => printed.to_string(),
    }
}

/// `fromJson.T(text)`: serde_json's text of a `T` read into the domain
/// value, `toJson.T`'s inverse. The text goes through `parseJson`, so a
/// 64-bit integer past 2^53 stays exact; a malformed text or a value the
/// schema refuses throws, as `JSON.parse` and the library's `parse` do.
fn from_json(schema: WireSchema, wired: &[&Item]) -> String {
    if wired.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n/**\n * Each type read from the JSON text serde_json writes, through `parseJson`;\n \
         * throws on malformed text or a value the schema refuses.\n */\nexport const fromJson = {\n",
    );
    for item in wired {
        let name = item.name().as_str();
        let read = match schema {
            WireSchema::Zod => format!("{name}.parse(parseJson(text))"),
            WireSchema::Valibot => format!("v.parse({name}, parseJson(text))"),
            WireSchema::Arktype => format!("{name}.assert(parseJson(text))"),
        };
        out.push_str(&format!("  {name}: (text: string): {name}$ => {read},\n"));
    }
    out.push_str("} as const;\n");
    out
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

/// `head a, b tail` on one line when it is short and each item is one
/// line, else one item per line; `pad` puts spaces inside the single-line
/// form (`{ a, b }`).
fn list(head: &str, items: &[String], tail: &str, pad: bool) -> String {
    list_within(90, head, items, tail, pad)
}

/// `list`, where the single-line form must fit in `width` columns.
fn list_within(width: usize, head: &str, items: &[String], tail: &str, pad: bool) -> String {
    let sp = if pad { " " } else { "" };
    let one = format!("{head}{sp}{}{sp}{tail}", items.join(", "));
    if one.len() <= width && !one.contains('\n') {
        return one;
    }
    let body = items
        .iter()
        .map(|i| format!("  {},", i.replace('\n', "\n  ")))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{head}\n{body}\n{tail}")
}

/// `base.a().b()` on one line when short, else one call per line.
fn chain(base: &str, calls: &[String]) -> String {
    let one = format!("{base}{}", calls.concat());
    if one.len() <= 90 && !one.contains('\n') {
        return one;
    }
    let calls: String = calls.iter().map(|c| format!("\n  {}", c.replace('\n', "\n  "))).collect();
    format!("{base}{calls}")
}

/// `({ kind: "V", a: .. })`, the domain value a variant arm builds.
fn variant_value(variant: &str, rest: &str) -> String {
    let mut fields = vec![format!("kind: \"{variant}\"")];
    fields.extend(rest.split(", ").filter(|f| !f.is_empty()).map(str::to_string));
    list("({", &fields, "})", true)
}

/// `export const T: <type> = <schema>;`, the schema behind `lazy` only for
/// a recursive type.
fn declare(schema: WireSchema, name: &str, expr: &str, recursive: bool) -> String {
    let (ty, lazy) = match schema {
        WireSchema::Zod => (format!("z.ZodType<{name}$, unknown>"), "z.lazy"),
        WireSchema::Valibot => (format!("v.GenericSchema<unknown, {name}$>"), "v.lazy"),
        WireSchema::Arktype => unreachable!("arktype declarations are printed by `ark_*`"),
    };
    if recursive {
        format!("\nexport const {name}: {ty} = {lazy}(() => {expr});\n")
    } else {
        format!("\nexport const {name}: {ty} = {expr};\n")
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
    let has_text = krate.items.iter().any(|i| matches!(i, Item::Fn(f) if f.owner.as_ref() == Some(&err) && f.name.as_str() == "toString"));
    if has_text {
        Refusal::Text(err.as_str().to_string())
    } else if krate.items.iter().any(|i| matches!(i, Item::Enum(en) if en.name == err)) {
        Refusal::Variant
    } else {
        Refusal::Type
    }
}

/// `import { E as E$text }` for each error type whose text names a refusal.
fn refusal_imports(krate: &Crate, wired: &[&Item]) -> String {
    let mut seen = std::collections::BTreeSet::new();
    for item in wired {
        if let Item::Struct(s) = item {
            if let Refusal::Text(err) = refusal(krate, s) {
                seen.insert(err);
            }
        }
    }
    seen.into_iter()
        .map(|err| format!("import {{ {err} as {err}$text }} from \"./{}.ts\";\n", purecrate_ir::Name::new(err.clone()).file_stem()))
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
    format!("import {{ Json, parseJson }} from \"purecrate\";\n{own}")
}

/// Zod and valibot. The domain value is built field by field: a schema's
/// inferred object type makes a field that may be `undefined` optional
/// (`unit?: undefined`), which the domain type does not accept.
fn struct_schema(schema: WireSchema, s: &Struct, recursive: bool, refused: &Refusal) -> String {
    let name = s.name.as_str();
    if let Some(from) = &s.wire_from {
        return try_from_schema(schema, s, from, recursive, refused);
    }
    if let Some(inner) = s.newtype_inner() {
        let value = schema_ty(schema, inner);
        // `x`, not `v`, which names valibot's namespace.
        let build = newtype_build(s, "x");
        let expr = match schema {
            WireSchema::Zod => format!("{value}.transform((x): {name}$ => {build})"),
            WireSchema::Valibot => format!("v.pipe({value}, v.transform((x): {name}$ => {build}))"),
            WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
        };
        return declare(schema, name, &expr, recursive);
    }
    let fields = object_fields(schema, &s.fields);
    let build = record_build(s, &fill(schema, &s.fields, "x"));
    let expr = match schema {
        // The object opens the declaration line, whose head takes room.
        WireSchema::Zod => chain(
            &list_within(100usize.saturating_sub(name.len() * 2 + 45), "z.object({", &fields, "})", true),
            &[format!(".transform((x): {name}$ => {build})")],
        ),
        WireSchema::Valibot => format!(
            "v.pipe(\n  {},\n  v.transform((x): {name}$ => {build}),\n)",
            list("v.object({", &fields, "})", true).replace('\n', "\n  ")
        ),
        WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
    };
    declare(schema, name, &expr, recursive)
}

/// `name: schema` for each field of a struct or struct variant.
fn object_fields(schema: WireSchema, fields: &[purecrate_ir::Field]) -> Vec<String> {
    fields
        .iter()
        .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(schema, &f.ty, true)))
        .collect()
}

/// The message of the issue a failed `try_from` raises: the type, and the
/// error's text or variant (`refusal`).
fn try_from_message(name: &str, refused: &Refusal) -> String {
    match refused {
        Refusal::Text(err) => format!("`{name}: ${{{err}$text.toString(r.error)}}`"),
        Refusal::Variant => format!("`{name}: ${{r.error.kind}}`"),
        Refusal::Type => format!("\"{name}\""),
    }
}

/// `#[serde(try_from = "T")]`: read `T`, then `S.tryFrom`; `Err` fails the
/// read, as serde fails deserialization (design/04 §5). The value comes
/// only from the checked constructor, so a closed type keeps its invariant.
fn try_from_schema(schema: WireSchema, s: &Struct, from: &Ty, recursive: bool, refused: &Refusal) -> String {
    let name = s.name.as_str();
    let from = schema_ty(schema, from);
    let message = try_from_message(name, refused);
    match schema {
        WireSchema::Zod => declare(
            schema,
            name,
            &format!(
                "{from}.transform((x, ctx): {name}$ => {{\n\
                 \x20 const r = {name}$value.tryFrom(x);\n\
                 \x20 if (r.kind === \"Err\") {{\n\
                 \x20   ctx.addIssue({{ code: \"custom\", message: {message}, input: x, params: {{ error: r.error }} }});\n\
                 \x20   return z.NEVER;\n\
                 \x20 }}\n\
                 \x20 return r.value;\n\
                 }})"
            ),
            recursive,
        ),
        WireSchema::Valibot => declare(
            schema,
            name,
            &format!(
                "v.pipe({from}, v.rawTransform(({{ dataset, addIssue, NEVER }}): {name}$ => {{\n\
                 \x20 const r = {name}$value.tryFrom(dataset.value);\n\
                 \x20 if (r.kind === \"Err\") {{\n\
                 \x20   addIssue({{ message: {message} }});\n\
                 \x20   return NEVER;\n\
                 \x20 }}\n\
                 \x20 return r.value;\n\
                 }}))"
            ),
            recursive,
        ),
        WireSchema::Arktype => format!(
            "\nconst {name}$wire = memo(() => {from});\n\
             export const {name}: Wire<{name}$> = type(\"unknown\").pipe((v, ctx): {name}$ => {{\n\
             \x20 const parsed = {name}$wire()(v);\n\
             \x20 if (parsed instanceof type.errors) return fail(ctx, parsed);\n\
             \x20 const r = {name}$value.tryFrom(parsed);\n\
             \x20 if (r.kind === \"Err\") return ctx.error({message}) as never;\n\
             \x20 return r.value;\n\
             }});\n"
        ),
    }
}

/// The domain value of a newtype from its parsed inner value. A closed
/// newtype has no `of`; serde's derive builds it from the shape alone, and so
/// does the schema, through the package-internal constructor (design/04 §5).
fn newtype_build(s: &Struct, from: &str) -> String {
    let name = s.name.as_str();
    if s.closed {
        format!("{}({from})", closed_ctor(name))
    } else {
        format!("{name}$value.of({from})")
    }
}

/// `{ a: v.a }`, or through the constructor when the struct is closed.
fn record_build(s: &Struct, fields: &str) -> String {
    if s.closed {
        format!("{}({{ {fields} }})", closed_ctor(s.name.as_str()))
    } else {
        format!("({{ {fields} }})")
    }
}

/// `a: v.a, b: v.b ?? null`: the fields of a parsed JSON object copied into
/// the domain value. Valibot leaves a missing optional field `undefined`.
fn fill(schema: WireSchema, fields: &[purecrate_ir::Field], from: &str) -> String {
    fields
        .iter()
        .map(|f| {
            let n = f.name.as_str();
            if schema == WireSchema::Valibot && matches!(f.ty, Ty::Option(_)) {
                format!("{n}: {from}.{n} ?? null")
            } else {
                format!("{n}: {from}.{n}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The quoted names of a fieldless enum's variants, or `None` when one has
/// fields: the adapters' `unitEnum` reads it whole.
fn unit_names(variants: &[purecrate_ir::Variant]) -> Option<Vec<String>> {
    variants
        .iter()
        .map(|v| matches!(v.fields, VariantFields::Unit).then(|| format!("\"{}\"", v.name.as_str())))
        .collect()
}

fn enum_schema(schema: WireSchema, name: &str, variants: &[purecrate_ir::Variant], recursive: bool) -> String {
    if let Some(names) = unit_names(variants) {
        return declare(schema, name, &list("unitEnum([", &names, "])", false), recursive);
    }
    let arms: Vec<String> = variants
        .iter()
        .map(|v| variant_arm(schema, name, v.name.as_str(), &v.fields))
        .collect();
    let expr = match schema {
        WireSchema::Zod => list("z.union([", &arms, "])", false),
        WireSchema::Valibot => list("v.union([", &arms, "])", false),
        WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
    };
    declare(schema, name, &expr, recursive)
}

/// serde's externally tagged enum: `{"Variant": ..}` has exactly one key, so
/// the wrapper rejects any other key. A unit variant is `"Variant"`, and
/// serde_json also reads `{"Variant": null}`. The fields inside a struct variant are a
/// struct's, and unknown ones are ignored as serde does by default.
fn variant_arm(schema: WireSchema, enum_name: &str, variant: &str, fields: &VariantFields) -> String {
    let transform = |param: &str, rest: &str| format!("({param}): {enum_name}$ => {}", variant_value(variant, rest));
    let (wrapper, rest) = match fields {
        VariantFields::Unit => {
            let value = transform("", "");
            return match schema {
                WireSchema::Zod => chain(&format!("unitVariant(\"{variant}\")"), &[format!(".transform({value})")]),
                WireSchema::Valibot => list("v.pipe(", &[format!("unitVariant(\"{variant}\")"), format!("v.transform({value})")], ")", false),
                WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
            };
        }
        VariantFields::Tuple(tys) => {
            let (json, content) = tuple_json(schema, variant, tys);
            (json, format!("content: {content}"))
        }
        VariantFields::Struct(fs) => {
            let fields = object_fields(schema, fs);
            let inner = match schema {
                WireSchema::Zod => list("z.object({", &fields, "})", true),
                _ => list("v.object({", &fields, "})", true),
            };
            (inner, fill(schema, fs, &format!("x.{variant}")))
        }
    };
    let value = transform("x", &rest);
    match schema {
        WireSchema::Zod => chain(
            &format!("z.object({{ {variant}: {wrapper} }})"),
            &[".strict()".to_string(), format!(".transform({value})")],
        ),
        WireSchema::Valibot => list(
            "v.pipe(",
            &[format!("v.strictObject({{ {variant}: {wrapper} }})"), format!("v.transform({value})")],
            ")",
            false,
        ),
        WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
    }
}

/// Arktype: every schema is a morph from `unknown`, annotated `Wire<T>` so
/// recursion does not make TypeScript infer `any`. The JSON shape is compiled
/// on first use (`memo`), so a definition may refer to a schema declared
/// later in the file. Variants are tried one at a time: arktype rejects an
/// unordered union of two object morphs.
fn ark_struct(s: &Struct, refused: &Refusal) -> String {
    let name = s.name.as_str();
    if let Some(from) = &s.wire_from {
        return try_from_schema(WireSchema::Arktype, s, from, false, refused);
    }
    let (shape, build) = match s.newtype_inner() {
        Some(inner) => (schema_ty(WireSchema::Arktype, inner), newtype_build(s, "parsed")),
        None => {
            let fields = s
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(WireSchema::Arktype, &f.ty, true)))
                .collect::<Vec<_>>()
                .join(", ");
            (format!("type({{ {fields} }})"), record_build(s, &fill(WireSchema::Arktype, &s.fields, "parsed")))
        }
    };
    format!(
        "\nconst {name}$wire = memo(() => {shape});\n\
         export const {name}: Wire<{name}$> = type(\"unknown\").pipe((v, ctx): {name}$ => {{\n\
         \x20 const parsed = {name}$wire()(v);\n\
         \x20 if (parsed instanceof type.errors) return fail(ctx, parsed);\n\
         \x20 return {build};\n\
         }});\n"
    )
}

fn ark_enum(en: &purecrate_ir::Enum) -> String {
    let name = en.name.as_str();
    if let Some(names) = unit_names(&en.variants) {
        let names = list(&format!("unitEnum(\"{name}\", ["), &names, "])", false);
        return format!("\nexport const {name}: Wire<{name}$> = {names};\n");
    }
    let mut arms = String::new();
    let mut body = String::new();
    for variant in &en.variants {
        let (arm, test) = ark_variant(name, variant);
        arms.push_str(&arm);
        body.push_str(&test);
    }
    format!(
        "\n{arms}export const {name}: Wire<{name}$> = type(\"unknown\").pipe((v, ctx): {name}$ => {{\n{body}  return ctx.error(\"{name}\") as never;\n}});\n"
    )
}

/// The memoized JSON shape of one variant, and the test that tries it.
fn ark_variant(en: &str, variant: &purecrate_ir::Variant) -> (String, String) {
    let name = variant.name.as_str();
    let arm = format!("{en}$arm${name}");
    let (shape, value) = match &variant.fields {
        VariantFields::Unit => {
            return (
                format!("const {arm} = memo(() => type({{ \"+\": \"reject\", {name}: \"null\" }}));\n"),
                format!(
                    "  if (v === \"{name}\") return {{ kind: \"{name}\" }};\n\
                     \x20 {{\n\
                     \x20   const parsed = {arm}()(v);\n\
                     \x20   if (!(parsed instanceof type.errors)) return {{ kind: \"{name}\" }};\n\
                     \x20   if (keyed(v, \"{name}\")) return fail(ctx, parsed);\n\
                     \x20 }}\n"
                ),
            );
        }
        VariantFields::Tuple(tys) => {
            let (json, content) = tuple_json(WireSchema::Arktype, name, tys);
            (json, format!("content: {}", content.replace("x.", "parsed.")))
        }
        VariantFields::Struct(fields) => {
            let inner = fields
                .iter()
                .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(WireSchema::Arktype, &f.ty, true)))
                .collect::<Vec<_>>()
                .join(", ");
            (format!("{{ {inner} }}"), fill(WireSchema::Arktype, fields, &format!("parsed.{name}")))
        }
    };
    (
        format!("const {arm} = memo(() => type({{ \"+\": \"reject\", {name}: {shape} }}));\n"),
        format!(
            "  {{\n    const parsed = {arm}()(v);\n    if (!(parsed instanceof type.errors)) return {{ kind: \"{name}\", {value} }};\n    if (keyed(v, \"{name}\")) return fail(ctx, parsed);\n  }}\n"
        ),
    )
}

/// A one-element tuple variant is serde's newtype variant: `{"Add": 4}`, not
/// `{"Add": [4]}`. The domain value is still a one-element tuple.
fn tuple_json(schema: WireSchema, variant: &str, tys: &[Ty]) -> (String, String) {
    if tys.len() == 1 {
        return (schema_ty(schema, &tys[0]), format!("[x.{variant}]"));
    }
    let inner = tys
        .iter()
        .map(|ty| schema_ty(schema, ty))
        .collect::<Vec<_>>()
        .join(", ");
    let json = match schema {
        WireSchema::Zod => format!("z.tuple([{inner}])"),
        WireSchema::Valibot => format!("v.tuple([{inner}])"),
        WireSchema::Arktype => format!("[{inner}]"),
    };
    (json, format!("x.{variant}"))
}

fn schema_ty(schema: WireSchema, ty: &Ty) -> String {
    schema_ty_in(schema, ty, false)
}

fn schema_ty_in(schema: WireSchema, ty: &Ty, struct_field: bool) -> String {
    match ty {
        Ty::Prim(p) => match p {
            purecrate_ir::Prim::Bool => "bool".into(),
            purecrate_ir::Prim::String | purecrate_ir::Prim::Str => "str".into(),
            purecrate_ir::Prim::Char => "char".into(),
            purecrate_ir::Prim::Uuid => "uuid".into(),
            purecrate_ir::Prim::UuidError => "uuidError".into(),
            purecrate_ir::Prim::Unit => "unit".into(),
            other => other
                .int()
                .map(|t| t.as_str().to_string())
                .or_else(|| other.float().map(|t| match t {
                    purecrate_ir::FloatTy::F32 => "f32".into(),
                    purecrate_ir::FloatTy::F64 => "f64".into(),
                }))
                .unwrap_or_else(|| "str".into()),
        },
        Ty::Option(inner) => {
            let inner = schema_ty(schema, inner);
            if !struct_field {
                return format!("nullable({inner})");
            }
            // A missing struct field is `None`, matching serde. The domain value is `T | null`.
            match schema {
                WireSchema::Zod => format!("optionalField({inner})"),
                WireSchema::Valibot => format!("v.optional(nullable({inner}))"),
                WireSchema::Arktype => format!("nullable({inner}).default(null)"),
            }
        }
        Ty::Vec(inner) => match schema {
            WireSchema::Zod => format!("z.array({})", schema_ty(schema, inner)),
            WireSchema::Valibot => format!("v.array({})", schema_ty(schema, inner)),
            WireSchema::Arktype => format!("({}).array()", schema_ty(schema, inner)),
        },
        Ty::Tuple(elems) => {
            let inner = elems.iter().map(|t| schema_ty(schema, t)).collect::<Vec<_>>().join(", ");
            match schema {
                WireSchema::Zod => format!("z.tuple([{inner}])"),
                WireSchema::Valibot => format!("v.tuple([{inner}])"),
                WireSchema::Arktype => format!("type([{inner}])"),
            }
        }
        Ty::Result { .. } => match schema {
            WireSchema::Zod => "z.never()".into(),
            WireSchema::Valibot => "v.never()".into(),
            WireSchema::Arktype => "type.never".into(),
        },
        Ty::Named(n) => n.as_str().to_string(),
        Ty::Ignored { inner, .. } => schema_ty_in(schema, inner, struct_field),
        Ty::Fn { .. } | Ty::Never => match schema {
            WireSchema::Zod => "z.never()".into(),
            WireSchema::Valibot => "v.never()".into(),
            WireSchema::Arktype => "type.never".into(),
        },
    }
}

/// `toJson.T(x)`: the JSON text serde_json writes for the Rust value of `x`
/// (design/04 §6). Struct fields in declaration order; a newtype is its
/// content; enums are externally tagged, a one-field tuple variant as the
/// field itself.
fn to_json(krate: &Crate) -> String {
    let mut out = String::from(
        "\n/** Each type written as serde_json writes the Rust value. */\nexport const toJson = {\n",
    );
    for item in krate.exported().filter(|i| serde(i).ser) {
        match item {
            Item::Struct(s) => {
                let name = s.name.as_str();
                let body = match s.newtype_inner() {
                    Some(inner) => write_json(inner, "x", 0),
                    None => object_json(s.fields.iter().map(|f| (f.name.as_str(), &f.ty)), "x."),
                };
                out.push_str(&format!("  {name}: (x: {name}$): string => {body},\n"));
            }
            // A variant name is a Rust identifier: nothing in it needs escaping.
            Item::Enum(e) if e.variants.iter().all(|v| matches!(v.fields, VariantFields::Unit)) => {
                let name = e.name.as_str();
                out.push_str(&format!("  {name}: (x: {name}$): string => `\"${{x.kind}}\"`,\n"));
            }
            Item::Enum(e) => {
                let name = e.name.as_str();
                out.push_str(&format!("  {name}: (x: {name}$): string => {{\n    switch (x.kind) {{\n"));
                for v in &e.variants {
                    let var = v.name.as_str();
                    let value = match &v.fields {
                        VariantFields::Unit => format!("\"\\\"{var}\\\"\""),
                        VariantFields::Tuple(tys) if tys.len() == 1 => {
                            format!("Json.object([[\"{var}\", {}]])", write_json(&tys[0], "x.content[0]", 0))
                        }
                        VariantFields::Tuple(tys) => {
                            let elems = tys
                                .iter()
                                .enumerate()
                                .map(|(i, t)| write_json(t, &format!("x.content[{i}]"), 0))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!("Json.object([[\"{var}\", Json.tuple([{elems}])]])")
                        }
                        VariantFields::Struct(fields) => {
                            let inner = object_json(fields.iter().map(|f| (f.name.as_str(), &f.ty)), "x.");
                            format!("Json.object([[\"{var}\", {inner}]])")
                        }
                    };
                    out.push_str(&format!("      case \"{var}\":\n        return {value};\n"));
                }
                out.push_str("    }\n  },\n");
            }
            Item::Alias(_) | Item::Fn(_) | Item::Const(_) => {}
        }
    }
    out.push_str("} as const;\n");
    out
}

/// `{"a":…,"b":…}` for fields read from `<prefix><name>`, through
/// `Json.object`: a call whose pairs a long line breaks one per line.
fn object_json<'a>(fields: impl Iterator<Item = (&'a str, &'a Ty)>, prefix: &str) -> String {
    let pairs = fields
        .map(|(name, ty)| format!("[\"{name}\", {}]", write_json(ty, &format!("{prefix}{name}"), 0)))
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        return "\"{}\"".into();
    }
    format!("Json.object([{}])", pairs.join(", "))
}

/// A TS expression for the JSON text of `value`, of type `ty`. `depth`
/// names the parameters of nested array writers apart.
fn write_json(ty: &Ty, value: &str, depth: usize) -> String {
    use purecrate_ir::{FloatTy, Prim};
    match ty {
        Ty::Prim(p) => match p {
            Prim::Bool => format!("Json.bool({value})"),
            Prim::String | Prim::Str | Prim::Char | Prim::Uuid => format!("Json.str({value})"),
            // Rust cannot serialize one either.
            Prim::UuidError => "((): never => { throw new globalThis.Error(\"uuid::Error has no JSON form\"); })()".into(),
            Prim::Unit => "\"null\"".into(),
            other => match other.float() {
                Some(FloatTy::F32) => format!("Json.f32({value})"),
                Some(FloatTy::F64) => format!("Json.f64({value})"),
                None => format!("Json.int({value})"),
            },
        },
        Ty::Option(inner) => format!("({value} === null ? \"null\" : {})", write_json(inner, value, depth)),
        Ty::Vec(inner) => {
            let v = format!("v{depth}");
            format!("Json.array({value}, ({v}) => {})", write_json(inner, &v, depth + 1))
        }
        Ty::Tuple(elems) => {
            let parts = elems
                .iter()
                .enumerate()
                .map(|(i, t)| write_json(t, &format!("{value}[{i}]"), depth))
                .collect::<Vec<_>>()
                .join(", ");
            format!("Json.tuple([{parts}])")
        }
        Ty::Result { ok, err } => format!(
            "({value}.kind === \"Ok\" ? `{{\"Ok\":{}}}` : `{{\"Err\":{}}}`)",
            splice(&write_json(ok, &format!("{value}.value"), depth)),
            splice(&write_json(err, &format!("{value}.error"), depth))
        ),
        Ty::Named(n) => format!("toJson.{}({value})", n.as_str()),
        Ty::Ignored { inner, .. } => write_json(inner, value, depth),
        Ty::Fn { .. } | Ty::Never => "\"null\"".into(),
    }
}

/// `expr` placed inside a template literal: a constant string or a nested
/// template is inlined, anything else becomes `${expr}`.
fn splice(expr: &str) -> String {
    match expr {
        "\"null\"" => "null".into(),
        "\"{}\"" => "{}".into(),
        _ if expr.len() > 1 && expr.starts_with('`') && expr.ends_with('`') => expr[1..expr.len() - 1].into(),
        _ => format!("${{{expr}}}"),
    }
}
