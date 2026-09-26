use crate::name::Name;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prim {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    String,
    Unit,
}

/// Value type at the crate boundary. No references.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Prim(Prim),
    Option(Box<Ty>),
    Result { ok: Box<Ty>, err: Box<Ty> },
    Vec(Box<Ty>),
    Tuple(Vec<Ty>),
    Named(Name),
    Never,
}

impl Ty {
    pub fn bool() -> Self {
        Ty::Prim(Prim::Bool)
    }

    pub fn i32() -> Self {
        Ty::Prim(Prim::I32)
    }

    pub fn named(name: impl Into<String>) -> Self {
        Ty::Named(Name::new(name))
    }

    pub fn option(inner: Ty) -> Self {
        Ty::Option(Box::new(inner))
    }

    pub fn result(ok: Ty, err: Ty) -> Self {
        Ty::Result {
            ok: Box::new(ok),
            err: Box::new(err),
        }
    }
}
