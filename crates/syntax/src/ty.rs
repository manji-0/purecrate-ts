use purecrate_ir::{Name, Prim, Ty};
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
        Type::Array(_) | Type::Slice(_) => Err(ParseError::new("arrays and slices are not in v0")),
        Type::Reference(_) => Err(ParseError::new("references are not allowed on the public surface")),
        Type::Paren(p) => lower_type(&p.elem),
        other => Err(ParseError::new(format!("unsupported type {}", snippet(other)))),
    }
}

const FORBIDDEN_CONTAINERS: [&str; 9] = [
    "Box", "Rc", "Arc", "Cell", "RefCell", "Mutex", "HashMap", "BTreeMap", "HashSet",
];

fn lower_path(path: &syn::Path) -> Result<Ty, ParseError> {
    if path.segments.len() != 1 || path.leading_colon.is_some() {
        return Err(ParseError::new(format!(
            "qualified type path {} is not in v0; use a crate-local name",
            snippet(path)
        )));
    }
    let last = &path.segments[0];
    let name = last.ident.to_string();
    if FORBIDDEN_CONTAINERS.contains(&name.as_str()) {
        return Err(ParseError::new(format!("`{name}` is not allowed in v0")));
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
        "f32" => Ok(Ty::Prim(Prim::F32)),
        "f64" => Ok(Ty::Prim(Prim::F64)),
        "String" | "str" => Ok(Ty::Prim(Prim::String)),
        "Option" => Ok(Ty::option(first_generic(&last.arguments)?)),
        "Vec" => Ok(Ty::Vec(Box::new(first_generic(&last.arguments)?))),
        "Result" => {
            let args = generics(&last.arguments)?;
            if args.len() != 2 {
                return Err(ParseError::new("Result needs two type arguments"));
            }
            let mut args = args;
            let err = args.pop().unwrap();
            let ok = args.pop().unwrap();
            Ok(Ty::result(ok, err))
        }
        _ if last.arguments.is_empty() => Ok(Ty::Named(Name::new(name))),
        _ => Err(ParseError::new(format!(
            "user generics are not in v0: {name}"
        ))),
    }
}

fn first_generic(args: &PathArguments) -> Result<Ty, ParseError> {
    let mut gs = generics(args)?;
    if gs.len() != 1 {
        return Err(ParseError::new("expected one type argument"));
    }
    Ok(gs.remove(0))
}

fn generics(args: &PathArguments) -> Result<Vec<Ty>, ParseError> {
    match args {
        PathArguments::None => Ok(Vec::new()),
        PathArguments::AngleBracketed(a) => a
            .args
            .iter()
            .map(|g| match g {
                GenericArgument::Type(t) => lower_type(t),
                _ => Err(ParseError::new("only type generics are allowed")),
            })
            .collect(),
        PathArguments::Parenthesized(_) => {
            Err(ParseError::new("Fn traits are not allowed"))
        }
    }
}
