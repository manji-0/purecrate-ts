//! Wire schemas for one chosen library. The shape is serde's default JSON.
//! Numeric fields use the shared `purecrate-*` adapter, so the result is the
//! domain brand (`I32`, not a bare `number`).

use purecrate_ir::{Crate, Item, Struct, Ty, VariantFields, NEWTYPE_FIELD};

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
            Self::Zod => "3.25.76",
            Self::Valibot => "1.1.0",
            Self::Arktype => "2.1.22",
        }
    }
}

pub fn emit_wire(krate: &Crate, schema: WireSchema) -> String {
    let mut out = String::from(super::HEADER);
    out.push('\n');
    out.push_str(&header(schema));
    for item in krate.exported() {
        if matches!(item, Item::Struct(_) | Item::Enum(_)) {
            let name = item.name().as_str();
            let value = matches!(item, Item::Struct(s) if s.newtype_inner().is_some() && !s.closed);
            if matches!(item, Item::Struct(s) if s.closed) {
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
    for item in krate.exported() {
        match (item, schema) {
            (Item::Struct(s), WireSchema::Arktype) => out.push_str(&ark_struct(s)),
            (Item::Enum(e), WireSchema::Arktype) => out.push_str(&ark_enum(e)),
            (Item::Struct(s), _) => out.push_str(&struct_schema(schema, s)),
            (Item::Enum(e), _) => out.push_str(&enum_schema(schema, e.name.as_str(), &e.variants)),
            (Item::Alias(_) | Item::Fn(_), _) => {}
        }
    }
    out
}

fn header(schema: WireSchema) -> String {
    match schema {
        WireSchema::Zod => "\
import { z } from \"zod\";
import { bool, f32, f64, i16, i32, i64, i8, nullable, str, u16, u32, u64, u8, unit, usize } from \"purecrate-zod\";
"
        .into(),
        WireSchema::Valibot => "\
import * as v from \"valibot\";
import { bool, f32, f64, i16, i32, i64, i8, nullable, str, u16, u32, u64, u8, unit, usize } from \"purecrate-valibot\";
"
        .into(),
        WireSchema::Arktype => "\
import { type } from \"arktype\";
import { bool, f32, f64, i16, i32, i64, i8, memo, nullable, str, u16, u32, u64, u8, unit, usize, type Wire } from \"purecrate-arktype\";
"
        .into(),
    }
}

/// Zod and valibot. The domain value is built field by field: a schema's
/// inferred object type makes a field that may be `undefined` optional
/// (`unit?: undefined`), which the domain type does not accept.
fn struct_schema(schema: WireSchema, s: &Struct) -> String {
    let name = s.name.as_str();
    if let Some(inner) = s.newtype_inner() {
        let value = schema_ty(schema, inner);
        let build = newtype_build(s, "v");
        return match schema {
            WireSchema::Zod => format!(
                "\nexport const {name}: z.ZodType<{name}$, z.ZodTypeDef, unknown> = z.lazy(() => {value}.transform((v): {name}$ => {build}));\n"
            ),
            WireSchema::Valibot => format!(
                "\nexport const {name}: v.GenericSchema<unknown, {name}$> = v.lazy(() => v.pipe({value}, v.transform((v): {name}$ => {build})));\n"
            ),
            WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
        };
    }
    let fields = s
        .fields
        .iter()
        .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(schema, &f.ty, true)))
        .collect::<Vec<_>>()
        .join(", ");
    let build = record_build(s, &fill(schema, &s.fields, "v"));
    match schema {
        WireSchema::Zod => format!(
            "\nexport const {name}: z.ZodType<{name}$, z.ZodTypeDef, unknown> = z.lazy(() => z.object({{ {fields} }}).transform((v): {name}$ => {build}));\n"
        ),
        WireSchema::Valibot => format!(
            "\nexport const {name}: v.GenericSchema<unknown, {name}$> = v.lazy(() => v.pipe(v.object({{ {fields} }}), v.transform((v): {name}$ => {build})));\n"
        ),
        WireSchema::Arktype => unreachable!("arktype structs are printed by `ark_struct`"),
    }
}

/// The domain value of a newtype from its parsed inner value. A closed
/// newtype has no `of`; serde's derive builds it from the shape alone, and so
/// does the schema, through the package-internal constructor (design/05 §7.7).
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

fn enum_schema(schema: WireSchema, name: &str, variants: &[purecrate_ir::Variant]) -> String {
    let arms = variants
        .iter()
        .map(|v| variant_arm(schema, name, v.name.as_str(), &v.fields))
        .collect::<Vec<_>>()
        .join(", ");
    match schema {
        WireSchema::Zod => format!(
            "\nexport const {name}: z.ZodType<{name}$, z.ZodTypeDef, unknown> = z.lazy(() => z.union([{arms}]));\n"
        ),
        WireSchema::Valibot => format!(
            "\nexport const {name}: v.GenericSchema<unknown, {name}$> = v.lazy(() => v.union([{arms}]));\n"
        ),
        WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
    }
}

/// serde's externally tagged enum: `{"Variant": ..}` has exactly one key, so
/// the wrapper rejects any other key. A unit variant is `"Variant"`, and
/// serde_json also reads `{"Variant": null}`. The fields inside a struct variant are a
/// struct's, and unknown ones are ignored as serde does by default.
fn variant_arm(schema: WireSchema, enum_name: &str, variant: &str, fields: &VariantFields) -> String {
    match fields {
        VariantFields::Unit => match schema {
            WireSchema::Zod => format!(
                "z.union([z.literal(\"{variant}\"), z.object({{ {variant}: z.null() }}).strict()]).transform((): {enum_name}$ => ({{ kind: \"{variant}\" }}))"
            ),
            WireSchema::Valibot => format!(
                "v.pipe(v.union([v.literal(\"{variant}\"), v.strictObject({{ {variant}: v.null() }})]), v.transform((): {enum_name}$ => ({{ kind: \"{variant}\" }})))"
            ),
            WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
        },
        VariantFields::Tuple(tys) => {
            let (json, content) = tuple_json(schema, variant, tys);
            match schema {
                WireSchema::Zod => format!(
                    "z.object({{ {variant}: {json} }}).strict().transform((v): {enum_name}$ => ({{ kind: \"{variant}\", content: {content} }}))"
                ),
                WireSchema::Valibot => format!(
                    "v.pipe(v.strictObject({{ {variant}: {json} }}), v.transform((v): {enum_name}$ => ({{ kind: \"{variant}\", content: {content} }})))"
                ),
                WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
            }
        }
        VariantFields::Struct(fs) => {
            let inner = fs
                .iter()
                .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(schema, &f.ty, true)))
                .collect::<Vec<_>>()
                .join(", ");
            let copies = fill(schema, fs, &format!("v.{variant}"));
            match schema {
                WireSchema::Zod => format!(
                    "z.object({{ {variant}: z.object({{ {inner} }}) }}).strict().transform((v): {enum_name}$ => ({{ kind: \"{variant}\", {copies} }}))"
                ),
                WireSchema::Valibot => format!(
                    "v.pipe(v.strictObject({{ {variant}: v.object({{ {inner} }}) }}), v.transform((v): {enum_name}$ => ({{ kind: \"{variant}\", {copies} }})))"
                ),
                WireSchema::Arktype => unreachable!("arktype enums are printed by `ark_enum`"),
            }
        }
    }
}

/// Arktype: every schema is a morph from `unknown`, annotated `Wire<T>` so
/// recursion does not make TypeScript infer `any`. The JSON shape is compiled
/// on first use (`memo`), so a definition may refer to a schema declared
/// later in the file. Variants are tried one at a time: arktype rejects an
/// unordered union of two object morphs.
fn ark_struct(s: &Struct) -> String {
    let name = s.name.as_str();
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
         \x20 if (parsed instanceof type.errors) return ctx.error(\"{name}\") as never;\n\
         \x20 return {build};\n\
         }});\n"
    )
}

fn ark_enum(en: &purecrate_ir::Enum) -> String {
    let name = en.name.as_str();
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
                    "  if (v === \"{name}\" || !({arm}()(v) instanceof type.errors)) return {{ kind: \"{name}\" }};\n"
                ),
            );
        }
        VariantFields::Tuple(tys) => {
            let (json, content) = tuple_json(WireSchema::Arktype, name, tys);
            (json, format!("content: {}", content.replace("v.", "parsed.")))
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
            "  {{\n    const parsed = {arm}()(v);\n    if (!(parsed instanceof type.errors)) return {{ kind: \"{name}\", {value} }};\n  }}\n"
        ),
    )
}

/// A one-element tuple variant is serde's newtype variant: `{"Add": 4}`, not
/// `{"Add": [4]}`. The domain value is still a one-element tuple.
fn tuple_json(schema: WireSchema, variant: &str, tys: &[Ty]) -> (String, String) {
    if tys.len() == 1 {
        return (schema_ty(schema, &tys[0]), format!("[v.{variant}]"));
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
    (json, format!("v.{variant}"))
}

fn schema_ty(schema: WireSchema, ty: &Ty) -> String {
    schema_ty_in(schema, ty, false)
}

fn schema_ty_in(schema: WireSchema, ty: &Ty, struct_field: bool) -> String {
    match ty {
        Ty::Prim(p) => match p {
            purecrate_ir::Prim::Bool => "bool".into(),
            purecrate_ir::Prim::String | purecrate_ir::Prim::Str => "str".into(),
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
                WireSchema::Zod => {
                    format!("nullable({inner}).optional().transform((v) => v ?? null)")
                }
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
