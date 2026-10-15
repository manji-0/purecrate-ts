use purecrate_ir::{Name, Prim, Reason, Ty, Wrapper};
use syn::spanned::Spanned;
use syn::{GenericArgument, PathArguments, Type};

use crate::item::{path_idents, snippet, ParseError};

pub fn lower_type(ty: &Type) -> Result<Ty, ParseError> {
    lower_type_node(ty).map_err(|e| e.or_at(ty.span()))
}

/// The name a hole `_` lowers to inside [`lower_type_with_holes`].
pub const HOLE: &str = "_";

thread_local! {
    static HOLES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The type parameters in scope (`impl<T>`, then `fn f<U>`).
    static PARAMS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The crate's `const` items, by name, with their values: an array's
    /// length may name one (`[u8; LEN]`).
    static CONSTS: std::cell::RefCell<std::collections::HashMap<String, syn::Expr>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Records the crate's `const` items, whose values an array length
/// (`[T; N]`) may read. Called once per crate, before any type is lowered.
pub fn scan_consts<'a>(items: impl IntoIterator<Item = &'a syn::Item>) {
    let found = items
        .into_iter()
        .filter_map(|i| match i {
            syn::Item::Const(c) => Some((c.ident.to_string(), (*c.expr).clone())),
            _ => None,
        })
        .collect();
    CONSTS.with(|c| *c.borrow_mut() = found);
}

/// The length of `[T; N]`: an integer literal, a crate `const`, or
/// arithmetic of those, folded as rustc folds it (design/01 §7.27).
pub(crate) fn array_len(len: &syn::Expr) -> Result<usize, ParseError> {
    fn fold(e: &syn::Expr, depth: usize) -> Option<u128> {
        if depth > 64 {
            return None;
        }
        match e {
            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(i), .. }) => i.base10_parse::<u128>().ok(),
            syn::Expr::Paren(p) => fold(&p.expr, depth),
            syn::Expr::Group(g) => fold(&g.expr, depth),
            syn::Expr::Block(b) if b.block.stmts.len() == 1 => match &b.block.stmts[0] {
                syn::Stmt::Expr(e, None) => fold(e, depth),
                _ => None,
            },
            syn::Expr::Path(p) if p.qself.is_none() => {
                let name = p.path.get_ident()?.to_string();
                let value = CONSTS.with(|c| c.borrow().get(&name).cloned())?;
                fold(&value, depth + 1)
            }
            syn::Expr::Binary(b) => {
                let (l, r) = (fold(&b.left, depth)?, fold(&b.right, depth)?);
                match b.op {
                    syn::BinOp::Add(_) => l.checked_add(r),
                    syn::BinOp::Sub(_) => l.checked_sub(r),
                    syn::BinOp::Mul(_) => l.checked_mul(r),
                    syn::BinOp::Div(_) => l.checked_div(r),
                    syn::BinOp::Rem(_) => l.checked_rem(r),
                    _ => None,
                }
            }
            _ => None,
        }
    }
    fold(len, 0).and_then(|n| usize::try_from(n).ok()).filter(|n| *n <= 9_007_199_254_740_991).ok_or_else(|| {
        ParseError::new(
            Reason::ArrayType,
            format!(
                "the length of an array is an integer literal or a crate `const` of literals in v0, found `{}`; a const generic `N` is not in v0 (use a `Vec`)",
                snippet(len)
            ),
        )
    })
}

/// Type parameters in scope while it lives: a name among them lowers to
/// `Ty::Param`.
pub struct ParamScope(usize);

impl ParamScope {
    pub fn enter(names: &[Name]) -> ParamScope {
        PARAMS.with(|p| {
            let mut p = p.borrow_mut();
            let depth = p.len();
            p.extend(names.iter().map(|n| n.as_str().to_string()));
            ParamScope(depth)
        })
    }
}

impl Drop for ParamScope {
    fn drop(&mut self) {
        PARAMS.with(|p| p.borrow_mut().truncate(self.0));
    }
}

/// `name` is a type parameter in scope (`T`, or a trait's `Self`).
pub fn is_param(name: &str) -> bool {
    PARAMS.with(|p| p.borrow().iter().any(|n| n == name))
}

/// `ty` where `_` may stand for a part the checker fills in
/// (`collect::<Result<Vec<_>, _>>()`), as `Ty::Named("_")`.
pub fn lower_type_with_holes(ty: &Type) -> Result<Ty, ParseError> {
    let before = HOLES.with(|h| h.replace(true));
    let out = lower_type(ty);
    HOLES.with(|h| h.set(before));
    out
}

fn lower_type_node(ty: &Type) -> Result<Ty, ParseError> {
    match ty {
        Type::Path(p) if p.qself.is_none() => lower_path(&p.path),
        Type::Tuple(t) if t.elems.is_empty() => Ok(Ty::Prim(Prim::Unit)),
        Type::Tuple(t) => {
            let elems = t.elems.iter().map(lower_type).collect::<Result<Vec<_>, _>>()?;
            Ok(Ty::Tuple(elems))
        }
        // `[T; N]`: a `ReadonlyArray<T>` of `N` elements (design/01 §7.27).
        Type::Array(a) => Ok(Ty::Array { elem: Box::new(lower_type(&a.elem)?), len: array_len(&a.len)? }),
        Type::Slice(_) => Err(ParseError::new(Reason::ArrayType, "a bare slice `[T]` is not in v0; borrow it, `&[T]`")),
        // Nothing emitted mutates through a shared reference, so `&T` and `T`
        // are the same TS value. `&[T]` reads a sequence like `&str` reads a string.
        Type::Reference(r) if r.mutability.is_some() => {
            Err(ParseError::new(Reason::RefType, "`&mut` references are not in v0: return the new value instead"))
        }
        Type::Reference(r) => match &*r.elem {
            Type::Slice(s) => Ok(Ty::Vec(Box::new(lower_type(&s.elem)?))),
            elem => lower_type(elem),
        },
        Type::Paren(p) => lower_type(&p.elem),
        Type::Infer(_) if HOLES.with(|h| h.get()) => Ok(Ty::Named(Name::new(HOLE))),
        // A value carries no tag that names its type, so nothing could pick
        // the implementation at run time (design/01 §7.25).
        Type::TraitObject(t) if t.bounds.iter().any(|b| matches!(b, syn::TypeParamBound::Trait(p) if p.path.segments.iter().all(|s| s.arguments.is_empty()))) => {
            Err(ParseError::new(
                Reason::FnType,
                format!(
                    "a trait object {} is not in v0: a value has no tag to dispatch on; take a type parameter with the bound (`fn f<T: Tr>(x: &T)`) or `impl Tr` as a parameter's type (design/01 §7.25)",
                    snippet(ty)
                ),
            ))
        }
        Type::BareFn(_) | Type::ImplTrait(_) | Type::TraitObject(_) => {
            Err(ParseError::new(Reason::FnType, format!("function and trait types are not in v0: {}", snippet(ty))))
        }
        other => Err(ParseError::new(Reason::UnsupportedType, format!("unsupported type {}", snippet(other)))),
    }
}

/// Primitives with no chosen TS form yet.
const UNSUPPORTED_PRIMS: [&str; 1] = ["isize"];

const FORBIDDEN_CONTAINERS: [&str; 5] = ["Rc", "Cell", "RefCell", "HashMap", "HashSet"];

fn lower_path(path: &syn::Path) -> Result<Ty, ParseError> {
    // The `uuid` crate is the one outside dependency whose types are modeled
    // (design/01 §6). Its error is named through the crate, as in
    // `Result<Uuid, uuid::Error>`, since a bare `Error` is usually the crate's own.
    let segs = path_idents(path);
    let plain = path.segments.iter().all(|s| s.arguments.is_empty());
    match (plain, segs.iter().map(String::as_str).collect::<Vec<_>>().as_slice()) {
        (true, ["uuid", "Uuid"]) => return Ok(Ty::Prim(Prim::Uuid)),
        (true, ["uuid", "Error"]) => return Ok(Ty::Prim(Prim::UuidError)),
        (true, ["std", "num", "ParseIntError"] | ["num", "ParseIntError"] | ["ParseIntError"]) => {
            return Ok(Ty::Prim(Prim::ParseIntError))
        }
        _ => {}
    }
    if path.segments.len() != 1 || path.leading_colon.is_some() {
        return Err(ParseError::new(
            Reason::QualifiedPath,
            format!("qualified type path {} is not in v0; use a crate-local name", snippet(path)),
        )
        .detail(segs.join("::")));
    }
    let last = &path.segments[0];
    let name = last.ident.to_string();
    if last.arguments.is_empty() && PARAMS.with(|p| p.borrow().contains(&name)) {
        return Ok(Ty::Param(Name::new(name)));
    }
    if name == "Self" {
        return Err(ParseError::new(Reason::SelfType, "`Self` outside an inherent impl is not in v0"));
    }
    if UNSUPPORTED_PRIMS.contains(&name.as_str()) {
        return Err(
            ParseError::new(Reason::DisallowedType, format!("`{name}` has no TS counterpart in v0")).detail(name)
        );
    }
    if FORBIDDEN_CONTAINERS.contains(&name.as_str()) {
        let hint = match name.as_str() {
            // Rust leaves a hash map's order unspecified, so a function that
            // iterates one has no one result to match (design/07 §5).
            "HashMap" => "; use `BTreeMap<String, V>`, which iterates in key order",
            "HashSet" => "; use `BTreeSet<String>`, which iterates in key order",
            _ => "",
        };
        return Err(
            ParseError::new(Reason::DisallowedType, format!("`{name}` is not allowed in v0{hint}")).detail(name)
        );
    }
    if name == "Mutex" {
        return Err(ParseError::new(
            Reason::Mutex,
            "`Mutex` is shared mutable state; the subset is pure functions, so `lock` is refused and erasing a `Mutex` field would only hide that",
        )
        .detail("Mutex"));
    }
    // `Box` / `Arc` are erased to the inner type. Emit keeps a comment
    // that says why Rust has the wrapper and why single-threaded TS drops it.
    if let Some(wrapper) = Wrapper::of_rust_name(&name) {
        return Ok(Ty::ignored(wrapper, first_generic(&last.arguments)?));
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
        "i128" => Ok(Ty::Prim(Prim::I128)),
        "u128" => Ok(Ty::Prim(Prim::U128)),
        "usize" => Ok(Ty::Prim(Prim::Usize)),
        "f32" => Ok(Ty::Prim(Prim::F32)),
        "f64" => Ok(Ty::Prim(Prim::F64)),
        "String" => Ok(Ty::Prim(Prim::String)),
        "str" => Ok(Ty::Prim(Prim::Str)),
        "char" => Ok(Ty::Prim(Prim::Char)),
        "Uuid" if generics(&last.arguments)?.is_empty() => Ok(Ty::Prim(Prim::Uuid)),
        "Option" => Ok(Ty::option(first_generic(&last.arguments)?)),
        "Vec" => Ok(Ty::Vec(Box::new(first_generic(&last.arguments)?))),
        "BTreeMap" | "BTreeSet" => {
            let mut args = generics(&last.arguments)?;
            let want = if name == "BTreeMap" { 2 } else { 1 };
            if args.len() != want {
                return Err(ParseError::new(Reason::TypeArity, format!("`{name}` needs {want} type argument(s)")));
            }
            // Which keys are accepted needs the crate's types: `check`
            // (design/01 §7.26).
            let key = Box::new(args.remove(0));
            Ok(if name == "BTreeMap" { Ty::Map { key, value: Box::new(args.remove(0)) } } else { Ty::Set(key) })
        }
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
        _ => {
            let args = generics(&last.arguments)?;
            if args.is_empty() {
                Ok(Ty::Named(Name::new(name)))
            } else {
                Ok(Ty::App { name: Name::new(name), args })
            }
        }
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
        PathArguments::Parenthesized(_) => Err(ParseError::new(Reason::FnType, "Fn traits are not allowed")),
    }
}
