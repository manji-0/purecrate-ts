//! Wire schemas for one chosen library. The shape is serde's default JSON.
//! Numeric fields use the shared `purecrate-*` adapter, so the result is the
//! domain brand (`I32`, not a bare `number`).

use purecrate_ir::{Crate, Item, Struct, Ty, VariantFields, NEWTYPE_FIELD};

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
            let value = matches!(item, Item::Struct(s) if s.newtype_inner().is_some());
            if value {
                out.push_str(&format!(
                    "import {{ {name} as {name}Value, type {name} as {name}$ }} from \"./{}.ts\";\n",
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
    if schema == WireSchema::Arktype {
        out.push_str(&ark_schemas(krate));
        return out;
    }
    for item in krate.exported() {
        match item {
            Item::Struct(s) => out.push_str(&struct_schema(schema, s)),
            Item::Enum(e) => out.push_str(&enum_schema(schema, e.name.as_str(), &e.variants)),
            Item::Alias(_) | Item::Fn(_) => {}
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
import { bool, f32, f64, i16, i32, i64, i8, nullable, str, u16, u32, u64, u8, unit, usize } from \"purecrate-arktype\";
"
        .into(),
    }
}

fn struct_schema(schema: WireSchema, s: &Struct) -> String {
    let name = s.name.as_str();
    if let Some(inner) = s.newtype_inner() {
        let value = schema_ty(schema, inner);
        return match schema {
            WireSchema::Zod => format!(
                "\nexport const {name}: z.ZodType<{name}$, z.ZodTypeDef, unknown> = z.lazy(() => {value}.transform((v): {name}$ => {name}Value.of(v)));\n"
            ),
            WireSchema::Valibot => format!(
                "\nexport const {name}: v.GenericSchema<unknown, {name}$> = v.lazy(() => v.pipe({value}, v.transform((v): {name}$ => {name}Value.of(v))));\n"
            ),
            WireSchema::Arktype => format!(
                "\nexport const {name} = type.lazy(() => {value}.pipe((v) => {name}Value.of(v)));\n"
            ),
        };
    }
    let fields = s
        .fields
        .iter()
        .filter(|f| f.name.as_str() != NEWTYPE_FIELD)
        .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(schema, &f.ty, true)))
        .collect::<Vec<_>>()
        .join(", ");
    match schema {
        WireSchema::Zod => format!(
            "\nexport const {name}: z.ZodType<{name}$, z.ZodTypeDef, unknown> = z.lazy(() => z.object({{ {fields} }}).transform((v): {name}$ => v));\n"
        ),
        WireSchema::Valibot => {
            let fill = valibot_fill(s);
            format!(
                "\nexport const {name}: v.GenericSchema<unknown, {name}$> = v.lazy(() => v.pipe(v.object({{ {fields} }}), v.transform({fill})));\n"
            )
        }
        WireSchema::Arktype => format!(
            "\nexport const {name} = type.lazy(() => type({{ {fields} }}).pipe((v): {name} => v));\n"
        ),
    }
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
        WireSchema::Arktype => format!(
            "\nexport const {name} = type.lazy(() => type.or({arms}));\n"
        ),
    }
}

fn variant_arm(schema: WireSchema, enum_name: &str, variant: &str, fields: &VariantFields) -> String {
    match fields {
        VariantFields::Unit => match schema {
            WireSchema::Zod => format!(
                "z.literal(\"{variant}\").transform((): {enum_name}$ => ({{ kind: \"{variant}\" }}))"
            ),
            WireSchema::Valibot => format!(
                "v.pipe(v.literal(\"{variant}\"), v.transform((): {enum_name}$ => ({{ kind: \"{variant}\" }})))"
            ),
            WireSchema::Arktype => format!(
                "type(\"'{}'\").pipe((): {enum_name} => ({{ kind: \"{variant}\" }}))",
                variant
            ),
        },
        VariantFields::Tuple(tys) => {
            let (json, content) = tuple_json(schema, variant, tys);
            match schema {
                WireSchema::Zod => format!(
                    "z.object({{ {variant}: {json} }}).transform((v): {enum_name}$ => ({{ kind: \"{variant}\", content: {content} }}))"
                ),
                WireSchema::Valibot => format!(
                    "v.pipe(v.object({{ {variant}: {json} }}), v.transform((v): {enum_name}$ => ({{ kind: \"{variant}\", content: {content} }})))"
                ),
                WireSchema::Arktype => String::new(),
            }
        }
        VariantFields::Struct(fs) => {
            let inner = fs
                .iter()
                .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(schema, &f.ty, true)))
                .collect::<Vec<_>>()
                .join(", ");
            let copies = fs
                .iter()
                .map(|f| {
                    let n = f.name.as_str();
                    if schema == WireSchema::Valibot && matches!(f.ty, Ty::Option(_)) {
                        format!("{n}: v.{variant}.{n} ?? null")
                    } else {
                        format!("{n}: v.{variant}.{n}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            match schema {
                WireSchema::Zod => format!(
                    "z.object({{ {variant}: z.object({{ {inner} }}) }}).transform((v): {enum_name}$ => ({{ kind: \"{variant}\", {copies} }}))"
                ),
                WireSchema::Valibot => format!(
                    "v.pipe(v.object({{ {variant}: v.object({{ {inner} }}) }}), v.transform((v): {enum_name}$ => ({{ kind: \"{variant}\", {copies} }})))"
                ),
                WireSchema::Arktype => format!(
                    "type({{ {variant}: {{ {inner} }} }}).pipe((v): {enum_name} => ({{ kind: \"{variant}\", {copies} }}))"
                ),
            }
        }
    }
}

/// Arktype rejects an unordered union of two object morphs, so each variant is
/// tried on its own. Structs share one module so a recursive alias can refer to
/// itself by name.
fn ark_schemas(krate: &Crate) -> String {
    let mut entries = Vec::new();
    let mut exports = Vec::new();
    let mut enums = String::new();
    for item in krate.exported() {
        match item {
            Item::Struct(s) => {
                entries.push(ark_struct_entry(s));
                let name = s.name.as_str();
                exports.push(format!(
                    "export const {name} = wire.{name}.pipe((v): {name}$ => v);\n"
                ));
            }
            Item::Enum(e) => enums.push_str(&ark_enum(e)),
            Item::Alias(_) | Item::Fn(_) => {}
        }
    }
    let mut out = String::new();
    if !entries.is_empty() {
        out.push_str("\nconst wire = type.module({\n");
        out.push_str(&entries.join("\n"));
        out.push_str("\n});\n\n");
        for export in exports {
            out.push_str(&export);
        }
        out.push('\n');
    }
    out.push_str(&enums);
    out
}

fn ark_struct_entry(s: &Struct) -> String {
    let name = s.name.as_str();
    if let Some(inner) = s.newtype_inner() {
        let value = schema_ty(WireSchema::Arktype, inner);
        return format!("  {name}: {value}.pipe((v): {name}$ => {name}Value.of(v)),");
    }
    let fields = s
        .fields
        .iter()
        .filter(|f| f.name.as_str() != NEWTYPE_FIELD)
        .map(|f| format!("{}: {}", f.name.as_str(), ark_field(&f.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("  {name}: {{ {fields} }},")
}

fn ark_field(ty: &Ty) -> String {
    if let Some(referent) = ark_scope_ref(ty) {
        return format!("\"{referent}\"");
    }
    schema_ty_in(WireSchema::Arktype, ty, true)
}

fn ark_scope_ref(ty: &Ty) -> Option<String> {
    match ty {
        Ty::Named(name) => Some(name.as_str().to_string()),
        Ty::Option(inner) => Some(format!("{} | null = null", ark_scope_ref(inner)?)),
        Ty::Vec(inner) => Some(format!("{}[]", ark_scope_ref(inner)?)),
        _ => None,
    }
}

fn ark_enum(en: &purecrate_ir::Enum) -> String {
    let name = en.name.as_str();
    let mut body = String::new();
    for variant in &en.variants {
        body.push_str(&ark_variant(variant));
    }
    format!(
        "export const {name} = type(\"unknown\").pipe((v, ctx): {name}$ => {{\n{body}  return ctx.error(\"{name}\") as never;\n}});\n\n"
    )
}

fn ark_variant(variant: &purecrate_ir::Variant) -> String {
    let name = variant.name.as_str();
    match &variant.fields {
        VariantFields::Unit => {
            format!("  if (v === \"{name}\") return {{ kind: \"{name}\" }};\n")
        }
        VariantFields::Tuple(tys) => {
            let (json, content) = tuple_json(WireSchema::Arktype, name, tys);
            let content = content.replace("v.", "parsed.");
            format!(
                "  {{\n    const parsed = type({{ {name}: {json} }})(v);\n    if (!(parsed instanceof type.errors)) return {{ kind: \"{name}\", content: {content} }};\n  }}\n"
            )
        }
        VariantFields::Struct(fields) => {
            let inner = fields
                .iter()
                .map(|f| format!("{}: {}", f.name.as_str(), schema_ty_in(WireSchema::Arktype, &f.ty, true)))
                .collect::<Vec<_>>()
                .join(", ");
            let copies = fields
                .iter()
                .map(|f| {
                    let field = f.name.as_str();
                    format!("{field}: parsed.{name}.{field}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "  {{\n    const parsed = type({{ {name}: {{ {inner} }} }})(v);\n    if (!(parsed instanceof type.errors)) return {{ kind: \"{name}\", {copies} }};\n  }}\n"
            )
        }
    }
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

fn domain_ty(ty: &Ty) -> String {
    match ty {
        Ty::Prim(p) => match p {
            purecrate_ir::Prim::Bool => "boolean".into(),
            purecrate_ir::Prim::String => "string".into(),
            purecrate_ir::Prim::Unit => "undefined".into(),
            other => other
                .int()
                .map(|t| t.ts_name().to_string())
                .or_else(|| other.float().map(|t| t.ts_name().to_string()))
                .unwrap_or_else(|| "number".into()),
        },
        Ty::Option(inner) => format!("{} | null", domain_ty(inner)),
        Ty::Vec(inner) => format!("ReadonlyArray<{}>", domain_ty(inner)),
        Ty::Tuple(elems) => {
            let inner = elems.iter().map(domain_ty).collect::<Vec<_>>().join(", ");
            format!("readonly [{inner}]")
        }
        Ty::Named(name) => format!("{}$", name.as_str()),
        Ty::Result { ok, err } => format!("Result<{}, {}>", domain_ty(ok), domain_ty(err)),
        Ty::Fn { .. } | Ty::Never => "never".into(),
    }
}

fn valibot_fill(s: &Struct) -> String {
    let name = s.name.as_str();
    let body = s
        .fields
        .iter()
        .filter(|f| f.name.as_str() != NEWTYPE_FIELD)
        .map(|f| {
            let field = f.name.as_str();
            if matches!(f.ty, Ty::Option(_)) {
                format!("{field}: v.{field} ?? null")
            } else {
                format!("{field}: v.{field}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("(v): {name}$ => ({{ {body} }})")
}

fn schema_ty(schema: WireSchema, ty: &Ty) -> String {
    schema_ty_in(schema, ty, false)
}

fn schema_ty_in(schema: WireSchema, ty: &Ty, struct_field: bool) -> String {
    match ty {
        Ty::Prim(p) => match p {
            purecrate_ir::Prim::Bool => "bool".into(),
            purecrate_ir::Prim::String => "str".into(),
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
        Ty::Fn { .. } | Ty::Never => match schema {
            WireSchema::Zod => "z.never()".into(),
            WireSchema::Valibot => "v.never()".into(),
            WireSchema::Arktype => "type.never".into(),
        },
    }
}
