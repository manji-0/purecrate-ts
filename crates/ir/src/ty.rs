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
    /// 64-bit Rust `usize`, printed as `number` and checked only up to
    /// 2^53−1. Past that, Rust debug does not panic and the generated code
    /// does. A stated non-equivalence (design/01 §3).
    Usize,
    F32,
    F64,
    String,
    /// `str` behind a borrow, and the type of a string literal. Same TS
    /// `string` as `String`, but rustc does not put one where `String` goes.
    Str,
    /// A Unicode scalar value: a branded one-code-point `string` (design/01 §6).
    Char,
    /// `uuid::Uuid`: a branded `string` in the lowercase hyphenated form,
    /// which serde writes and which orders as the 16 bytes do (design/01 §6).
    Uuid,
    /// `uuid::Error`, what `Uuid::parse_str` fails with: an opaque value.
    UuidError,
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
    Usize,
}

impl IntTy {
    pub const ALL: [IntTy; 9] = [
        IntTy::I8,
        IntTy::I16,
        IntTy::I32,
        IntTy::I64,
        IntTy::U8,
        IntTy::U16,
        IntTy::U32,
        IntTy::U64,
        IntTy::Usize,
    ];

    /// TypeScript brand name. Distinct from `as_str`, which is the Rust name.
    pub fn ts_name(self) -> &'static str {
        match self {
            IntTy::I8 => "I8",
            IntTy::I16 => "I16",
            IntTy::I32 => "I32",
            IntTy::I64 => "I64",
            IntTy::U8 => "U8",
            IntTy::U16 => "U16",
            IntTy::U32 => "U32",
            IntTy::U64 => "U64",
            IntTy::Usize => "Usize",
        }
    }

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
            IntTy::Usize => "usize",
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
            // `Number.MAX_SAFE_INTEGER`, not `usize::MAX`.
            IntTy::Usize => (0, 9_007_199_254_740_991),
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, IntTy::I8 | IntTy::I16 | IntTy::I32 | IntTy::I64)
    }

    /// Width in Rust; `usize` is 64 bits, though `bounds` stops at 2^53−1.
    pub fn bits(self) -> u32 {
        match self {
            IntTy::I8 | IntTy::U8 => 8,
            IntTy::I16 | IntTy::U16 => 16,
            IntTy::I32 | IntTy::U32 => 32,
            IntTy::I64 | IntTy::U64 | IntTy::Usize => 64,
        }
    }

    /// Printed as TS `bigint` rather than `number`.
    pub fn is_big(self) -> bool {
        matches!(self, IntTy::I64 | IntTy::U64)
    }

    pub fn of_suffix(suffix: &str) -> Option<IntTy> {
        IntTy::ALL.into_iter().find(|t| t.as_str() == suffix)
    }

    /// std implements `From<self> for to`. Every such conversion keeps the
    /// value. `usize` takes only `u8` and `u16`, as in std.
    pub fn widens_to(self, to: IntTy) -> bool {
        use IntTy::*;
        self == to
            || matches!(
                (self, to),
                (U8, U16 | U32 | U64 | Usize | I16 | I32 | I64)
                    | (U16, U32 | U64 | Usize | I32 | I64)
                    | (U32, U64 | I64)
                    | (I8, I16 | I32 | I64)
                    | (I16, I32 | I64)
                    | (I32, I64)
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FloatTy {
    F32,
    F64,
}

impl FloatTy {
    pub fn ts_name(self) -> &'static str {
        match self {
            FloatTy::F32 => "F32",
            FloatTy::F64 => "F64",
        }
    }
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
            IntTy::Usize => Prim::Usize,
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

/// A Rust wrapper the generated TS drops. The value is the inner type.
/// `Rc`, `Cell`, and `RefCell` stay rejected: erasing them would change
/// what a single-threaded program can observe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Wrapper {
    Box,
    Arc,
    Mutex,
}

impl Wrapper {
    pub fn rust_name(self) -> &'static str {
        match self {
            Wrapper::Box => "Box",
            Wrapper::Arc => "Arc",
            Wrapper::Mutex => "Mutex",
        }
    }

}

/// Value type. No references. `Fn` is the type of a closure; the parser
/// never produces it, so it cannot reach a signature or a field.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Prim(Prim),
    Option(Box<Ty>),
    Result { ok: Box<Ty>, err: Box<Ty> },
    Vec(Box<Ty>),
    Tuple(Vec<Ty>),
    Named(Name),
    Fn { params: Vec<Ty>, ret: Box<Ty> },
/// `Box` / `Arc`. Equal to `inner` for checking. Emit keeps the comment.
    Ignored { wrapper: Wrapper, inner: Box<Ty> },
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

    pub fn ignored(wrapper: Wrapper, inner: Ty) -> Self {
        Ty::Ignored {
            wrapper,
            inner: Box::new(inner),
        }
    }

    /// Drops `Box` / `Arc` layers. Checking sees the inner type.
    pub fn peel(&self) -> &Ty {
        match self {
            Ty::Ignored { inner, .. } => inner.peel(),
            other => other,
        }
    }
}
