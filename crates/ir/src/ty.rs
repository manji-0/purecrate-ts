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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IntTy {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntTy {
    pub const ALL: [IntTy; 8] = [
        IntTy::I8,
        IntTy::I16,
        IntTy::I32,
        IntTy::I64,
        IntTy::U8,
        IntTy::U16,
        IntTy::U32,
        IntTy::U64,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            IntTy::I8 => "i8",
            IntTy::I16 => "i16",
            IntTy::I32 => "i32",
            IntTy::I64 => "i64",
            IntTy::U8 => "u8",
            IntTy::U16 => "u16",
            IntTy::U32 => "u32",
            IntTy::U64 => "u64",
        }
    }

    pub fn bounds(self) -> (i128, i128) {
        match self {
            IntTy::I8 => (i8::MIN.into(), i8::MAX.into()),
            IntTy::I16 => (i16::MIN.into(), i16::MAX.into()),
            IntTy::I32 => (i32::MIN.into(), i32::MAX.into()),
            IntTy::I64 => (i64::MIN.into(), i64::MAX.into()),
            IntTy::U8 => (0, u8::MAX.into()),
            IntTy::U16 => (0, u16::MAX.into()),
            IntTy::U32 => (0, u32::MAX.into()),
            IntTy::U64 => (0, u64::MAX.into()),
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64)
    }

    /// Printed as TS `bigint` rather than `number`.
    pub fn is_big(self) -> bool {
        matches!(self, IntTy::I64 | IntTy::U64)
    }

    pub fn of_suffix(suffix: &str) -> Option<IntTy> {
        IntTy::ALL.into_iter().find(|t| t.as_str() == suffix)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FloatTy {
    F32,
    F64,
}

impl From<IntTy> for Prim {
    fn from(t: IntTy) -> Prim {
        match t {
            IntTy::I8 => Prim::I8,
            IntTy::I16 => Prim::I16,
            IntTy::I32 => Prim::I32,
            IntTy::I64 => Prim::I64,
            IntTy::U8 => Prim::U8,
            IntTy::U16 => Prim::U16,
            IntTy::U32 => Prim::U32,
            IntTy::U64 => Prim::U64,
        }
    }
}

impl From<FloatTy> for Prim {
    fn from(t: FloatTy) -> Prim {
        match t {
            FloatTy::F32 => Prim::F32,
            FloatTy::F64 => Prim::F64,
        }
    }
}

impl Prim {
    pub fn int(self) -> Option<IntTy> {
        IntTy::ALL.into_iter().find(|t| Prim::from(*t) == self)
    }

    pub fn float(self) -> Option<FloatTy> {
        match self {
            Prim::F32 => Some(FloatTy::F32),
            Prim::F64 => Some(FloatTy::F64),
            _ => None,
        }
    }
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
