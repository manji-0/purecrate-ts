use purecrate_ir::{Name, Prim, Reason, Ty};
use syn::spanned::Spanned;
use syn::{GenericArgument, PathArguments, Type};

use crate::item::{snippet, ParseError};

pub fn lower_type(ty: &Type) -> Result<Ty, ParseError> {
    lower_type_node(ty).map_err(|e| e.or_at(ty.span()))
}

fn lower_type_node(ty: &Type) -> Result<Ty, ParseError> {
    match ty {
        Type::Path(p) if p.qself.is_none() => lower_path(&p.path),
        Type::Tuple(t) if t.elems.is_empty() => Ok(Ty::Prim(Prim::Unit)),
        Type::Tuple(t) => {
            let elems = t
                .elems
                .iter()
                .map(lower_type)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Ty::Tuple(elems))
        }
        Type::Array(_) | Type::Slice(_) => Err(ParseError::new(Reason::ArrayType, "arrays and slices are not in v0")),
        // Nothing emitted mutates through a shared reference, so `&T` and `T`
        // are the same TS value. `&[T]` reads a sequence like `&str` reads a string.
        Type::Reference(r) if r.mutability.is_some() => Err(ParseError::new(
            Reason::RefType,
            "`&mut` references are not in v0: return the new value instead",
        )),
        Type::Reference(r) => match &*r.elem {
            Type::Slice(s) => Ok(Ty::Vec(Box::new(lower_type(&s.elem)?))),
            elem => lower_type(elem),
        },
        Type::Paren(p) => lower_type(&p.elem),
        Type::BareFn(_) | Type::ImplTrait(_) | Type::TraitObject(_) => Err(ParseError::new(
            Reason::FnType,
            format!("function and trait types are not in v0: {}", snippet(ty)),
        )),
        other => Err(ParseError::new(Reason::UnsupportedType, format!("unsupported type {}", snippet(other)))),
    }
}

/// Primitives with no chosen TS form yet.
const UNSUPPORTED_PRIMS: [&str; 4] = ["isize", "u128", "i128", "char"];

const FORBIDDEN_CONTAINERS: [&str; 9] = [
    "Box", "Rc", "Arc", "Cell", "RefCell", "Mutex", "HashMap", "BTreeMap", "HashSet",
];

fn lower_path(path: &syn::Path) -> Result<Ty, ParseError> {
    if path.segments.len() != 1 || path.leading_colon.is_some() {
        return Err(ParseError::new(Reason::QualifiedPath, format!(
            "qualified type path {} is not in v0; use a crate-local name",
            snippet(path)
        ))
        .detail(
            path.segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        ));
    }
    let last = &path.segments[0];
    let name = last.ident.to_string();
    if name == "Self" {
        return Err(ParseError::new(Reason::SelfType, "`Self` outside an inherent impl is not in v0"));
    }
    if UNSUPPORTED_PRIMS.contains(&name.as_str()) {
        return Err(
            ParseError::new(Reason::DisallowedType, format!("`{name}` has no TS counterpart in v0")).detail(name),
        );
    }
    if FORBIDDEN_CONTAINERS.contains(&name.as_str()) {
        return Err(ParseError::new(Reason::DisallowedType, format!("`{name}` is not allowed in v0")).detail(name));
    }
    match name.as_str() {
        "bool" => Ok(Ty::Prim(Prim::Bool)),
        "i8" => Ok(Ty::Prim(Prim::I8)),
        "i16" => Ok(Ty::Prim(Prim::I16)),
        "i32" => Ok(Ty::Prim(Prim::I32)),
        "i64" => Ok(Ty::Prim(Prim::I64)),
        "u8" => Ok(Ty::Prim(Prim::U8)),
        "u16" => Ok(Ty::Prim(Prim::U16)),
        "u32" => Ok(Ty::Prim(Prim::U32)),
        "u64" => Ok(Ty::Prim(Prim::U64)),
        "usize" => Ok(Ty::Prim(Prim::Usize)),
        "f32" => Ok(Ty::Prim(Prim::F32)),
        "f64" => Ok(Ty::Prim(Prim::F64)),
        "String" | "str" => Ok(Ty::Prim(Prim::String)),
        "Option" => Ok(Ty::option(first_generic(&last.arguments)?)),
        "Vec" => Ok(Ty::Vec(Box::new(first_generic(&last.arguments)?))),
        "Result" => {
            let args = generics(&last.arguments)?;
            if args.len() != 2 {
                return Err(ParseError::new(Reason::TypeArity, "Result needs two type arguments"));
            }
            let mut args = args;
            let err = args.pop().unwrap();
            let ok = args.pop().unwrap();
            Ok(Ty::result(ok, err))
        }
        _ if generics(&last.arguments)?.is_empty() => Ok(Ty::Named(Name::new(name))),
        _ => Err(ParseError::new(Reason::Generics, format!(
            "user generics are not in v0: {name}"
        ))),
    }
}

fn first_generic(args: &PathArguments) -> Result<Ty, ParseError> {
    let mut gs = generics(args)?;
    if gs.len() != 1 {
        return Err(ParseError::new(Reason::TypeArity, "expected one type argument"));
    }
    Ok(gs.remove(0))
}

fn generics(args: &PathArguments) -> Result<Vec<Ty>, ParseError> {
    match args {
        PathArguments::None => Ok(Vec::new()),
        PathArguments::AngleBracketed(a) => a
            .args
            .iter()
            .filter(|g| !matches!(g, GenericArgument::Lifetime(_)))
            .map(|g| match g {
                GenericArgument::Type(t) => lower_type(t),
                _ => Err(ParseError::new(Reason::Generics, "only type generics are allowed")),
            })
            .collect(),
        PathArguments::Parenthesized(_) => {
            Err(ParseError::new(Reason::FnType, "Fn traits are not allowed"))
        }
    }
}
