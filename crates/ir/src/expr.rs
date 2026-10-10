use crate::name::Name;
use crate::ty::{FloatTy, IntTy, Ty, Wrapper};

/// Numeric literals carry their Rust type once known: from the source suffix,
/// or filled in by `check::accept`. An integer without one prints as a JS
/// `number`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lit {
    Bool(bool),
    /// `byte` when the source wrote it as a byte literal (`b'@'`), which
    /// emit notes beside the number; `hex` when it wrote `0x..`, which emit
    /// keeps.
    Int {
        value: i128,
        ty: Option<IntTy>,
        byte: bool,
        hex: bool,
    },
    Float {
        digits: String,
        ty: Option<FloatTy>,
    },
    Str(String),
    Char(char),
    Unit,
    Null,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    /// `& | ^` and the shifts, on integers only. `check::accept` rewrites
    /// each into a `Callee::Int` call; none is printed as a JS operator.
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinOp {
    /// `& | ^`: both operands and the result have one integer type.
    pub fn is_bitwise(self) -> bool {
        matches!(self, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor)
    }

    /// `<< >>`: the result has the left operand's type; the right operand
    /// is any integer.
    pub fn is_shift(self) -> bool {
        matches!(self, BinOp::Shl | BinOp::Shr)
    }
}

/// `!` is logical on `bool` and bitwise on an integer, as in Rust.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

/// Integer arithmetic with Rust debug-build semantics: truncating division,
/// and a throw wherever Rust would panic (overflow, zero divisor, a shift
/// amount outside `0..bits`). Bitwise results wrap to the width; they never
/// panic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Neg,
    And,
    Or,
    Xor,
    /// `!x`.
    Not,
    /// The amount is the second argument, of any integer type.
    Shl,
    Shr,
    /// The integer methods (`x.min(y)`, `x.checked_add(y)`, ...), each the
    /// exact result checked, wrapped, clamped, or tested against the range
    /// as std does (design/01 §7). `Pow` and its forms take a `u32`
    /// exponent; `Checked*` return `Option`.
    Method(IntMethod),
}

/// A method of `f32` / `f64` from the allow-list (design/01 §5.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatMethod {
    /// Half away from zero, as Rust rounds (`Math.round` rounds half up).
    Round,
    Floor,
    Ceil,
    Trunc,
    Abs,
    IsNan,
    IsFinite,
    IsInfinite,
}

impl FloatMethod {
    pub const ALL: [FloatMethod; 8] = [
        FloatMethod::Round,
        FloatMethod::Floor,
        FloatMethod::Ceil,
        FloatMethod::Trunc,
        FloatMethod::Abs,
        FloatMethod::IsNan,
        FloatMethod::IsFinite,
        FloatMethod::IsInfinite,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FloatMethod::Round => "round",
            FloatMethod::Floor => "floor",
            FloatMethod::Ceil => "ceil",
            FloatMethod::Trunc => "trunc",
            FloatMethod::Abs => "abs",
            FloatMethod::IsNan => "is_nan",
            FloatMethod::IsFinite => "is_finite",
            FloatMethod::IsInfinite => "is_infinite",
        }
    }

    /// A test, of type `bool`, rather than a float of the receiver's type.
    pub fn is_test(self) -> bool {
        matches!(self, FloatMethod::IsNan | FloatMethod::IsFinite | FloatMethod::IsInfinite)
    }
}

/// `f64::NAN`, `f64::INFINITY`, `f64::NEG_INFINITY` (and `f32`'s).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloatConst {
    Nan,
    Infinity,
    NegInfinity,
}

impl FloatConst {
    pub fn of_name(name: &str) -> Option<FloatConst> {
        match name {
            "NAN" => Some(FloatConst::Nan),
            "INFINITY" => Some(FloatConst::Infinity),
            "NEG_INFINITY" => Some(FloatConst::NegInfinity),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntMethod {
    Min,
    Max,
    /// Signed only; panics on `MIN` as negation does.
    Abs,
    Pow,
    CheckedAdd,
    CheckedSub,
    CheckedMul,
    CheckedDiv,
    CheckedRem,
    CheckedNeg,
    CheckedPow,
    SaturatingAdd,
    SaturatingSub,
    SaturatingMul,
    SaturatingPow,
    WrappingAdd,
    WrappingSub,
    WrappingMul,
    WrappingDiv,
    WrappingRem,
    WrappingNeg,
    WrappingPow,
}

impl IntMethod {
    pub const ALL: [IntMethod; 22] = [
        IntMethod::Min,
        IntMethod::Max,
        IntMethod::Abs,
        IntMethod::Pow,
        IntMethod::CheckedAdd,
        IntMethod::CheckedSub,
        IntMethod::CheckedMul,
        IntMethod::CheckedDiv,
        IntMethod::CheckedRem,
        IntMethod::CheckedNeg,
        IntMethod::CheckedPow,
        IntMethod::SaturatingAdd,
        IntMethod::SaturatingSub,
        IntMethod::SaturatingMul,
        IntMethod::SaturatingPow,
        IntMethod::WrappingAdd,
        IntMethod::WrappingSub,
        IntMethod::WrappingMul,
        IntMethod::WrappingDiv,
        IntMethod::WrappingRem,
        IntMethod::WrappingNeg,
        IntMethod::WrappingPow,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    /// The Rust method.
    pub fn name(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Max => "max",
            Self::Abs => "abs",
            Self::Pow => "pow",
            Self::CheckedAdd => "checked_add",
            Self::CheckedSub => "checked_sub",
            Self::CheckedMul => "checked_mul",
            Self::CheckedDiv => "checked_div",
            Self::CheckedRem => "checked_rem",
            Self::CheckedNeg => "checked_neg",
            Self::CheckedPow => "checked_pow",
            Self::SaturatingAdd => "saturating_add",
            Self::SaturatingSub => "saturating_sub",
            Self::SaturatingMul => "saturating_mul",
            Self::SaturatingPow => "saturating_pow",
            Self::WrappingAdd => "wrapping_add",
            Self::WrappingSub => "wrapping_sub",
            Self::WrappingMul => "wrapping_mul",
            Self::WrappingDiv => "wrapping_div",
            Self::WrappingRem => "wrapping_rem",
            Self::WrappingNeg => "wrapping_neg",
            Self::WrappingPow => "wrapping_pow",
        }
    }

    /// The runtime's name: `Int.<ty>.<ts_name>`.
    pub fn ts_name(self) -> &'static str {
        match self {
            Self::Min => "min",
            Self::Max => "max",
            Self::Abs => "abs",
            Self::Pow => "pow",
            Self::CheckedAdd => "checkedAdd",
            Self::CheckedSub => "checkedSub",
            Self::CheckedMul => "checkedMul",
            Self::CheckedDiv => "checkedDiv",
            Self::CheckedRem => "checkedRem",
            Self::CheckedNeg => "checkedNeg",
            Self::CheckedPow => "checkedPow",
            Self::SaturatingAdd => "saturatingAdd",
            Self::SaturatingSub => "saturatingSub",
            Self::SaturatingMul => "saturatingMul",
            Self::SaturatingPow => "saturatingPow",
            Self::WrappingAdd => "wrappingAdd",
            Self::WrappingSub => "wrappingSub",
            Self::WrappingMul => "wrappingMul",
            Self::WrappingDiv => "wrappingDiv",
            Self::WrappingRem => "wrappingRem",
            Self::WrappingNeg => "wrappingNeg",
            Self::WrappingPow => "wrappingPow",
        }
    }

    /// Arguments, the receiver included.
    pub fn arity(self) -> usize {
        match self {
            Self::Abs | Self::CheckedNeg | Self::WrappingNeg => 1,
            _ => 2,
        }
    }

    /// The second argument is a `u32` exponent.
    pub fn takes_exponent(self) -> bool {
        matches!(self, Self::Pow | Self::CheckedPow | Self::SaturatingPow | Self::WrappingPow)
    }

    /// The result is `Option` of the receiver's type.
    pub fn is_checked(self) -> bool {
        matches!(
            self,
            Self::CheckedAdd
                | Self::CheckedSub
                | Self::CheckedMul
                | Self::CheckedDiv
                | Self::CheckedRem
                | Self::CheckedNeg
                | Self::CheckedPow
        )
    }
}

impl IntOp {
    pub fn arity(self) -> usize {
        match self {
            IntOp::Neg | IntOp::Not => 1,
            IntOp::Method(m) => m.arity(),
            _ => 2,
        }
    }

    pub fn is_shift(self) -> bool {
        matches!(self, IntOp::Shl | IntOp::Shr)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            IntOp::Add => "add",
            IntOp::Sub => "sub",
            IntOp::Mul => "mul",
            IntOp::Div => "div",
            IntOp::Rem => "rem",
            IntOp::Neg => "neg",
            IntOp::And => "and",
            IntOp::Or => "or",
            IntOp::Xor => "xor",
            IntOp::Not => "not",
            IntOp::Shl => "shl",
            IntOp::Shr => "shr",
            IntOp::Method(m) => m.ts_name(),
        }
    }
}

/// `str` methods whose result does not depend on UTF-8 versus UTF-16 for a
/// well-formed string, or that go through the runtime's `Str` when it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrMethod {
    /// UTF-8 byte count, as `usize`. Prints as `Str.len(s)`.
    Len,
    IsEmpty,
    /// Takes a `&str` needle. A prefix, suffix or substring of UTF-8 bytes
    /// on char boundaries is one of UTF-16 units too, and both needles are
    /// well-formed, so the JS methods agree.
    StartsWith,
    EndsWith,
    Contains,
    /// The same `string`; lets a `String` be matched against literals.
    AsStr,
    /// Take a `&str`; `Option<&str>`. A prefix or suffix on char boundaries
    /// has the same extent in UTF-8 bytes and UTF-16 units, so the rest is
    /// the same string. Print as `Str.stripPrefix` / `Str.stripSuffix`.
    StripPrefix,
    StripSuffix,
    /// Takes a `char` or a `&str`; `Option<(&str, &str)>`, the text around
    /// the first match. The first match in UTF-8 bytes is the first in
    /// UTF-16 units, both sides are on char boundaries, and an empty needle
    /// matches at 0 in both. Prints as `Str.splitOnce`.
    SplitOnce,
    /// Takes a `&str`. Equal once ASCII `A`..=`Z` are folded to lower case,
    /// every other byte as it is: folding maps ASCII to ASCII and leaves the
    /// rest, so comparing folded UTF-16 units agrees with folded UTF-8
    /// bytes. Prints as `Str.eqIgnoreAsciiCase`.
    EqIgnoreAsciiCase,
}

impl StrMethod {
    pub const ALL: [StrMethod; 10] = [
        StrMethod::Len,
        StrMethod::IsEmpty,
        StrMethod::StartsWith,
        StrMethod::EndsWith,
        StrMethod::Contains,
        StrMethod::AsStr,
        StrMethod::StripPrefix,
        StrMethod::StripSuffix,
        StrMethod::SplitOnce,
        StrMethod::EqIgnoreAsciiCase,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Len => "len",
            Self::IsEmpty => "is_empty",
            Self::StartsWith => "starts_with",
            Self::EndsWith => "ends_with",
            Self::Contains => "contains",
            Self::AsStr => "as_str",
            Self::StripPrefix => "strip_prefix",
            Self::StripSuffix => "strip_suffix",
            Self::SplitOnce => "split_once",
            Self::EqIgnoreAsciiCase => "eq_ignore_ascii_case",
        }
    }

    /// Arguments after the receiver.
    pub fn needles(self) -> usize {
        match self {
            Self::Len | Self::IsEmpty | Self::AsStr => 0,
            Self::StartsWith
            | Self::EndsWith
            | Self::Contains
            | Self::StripPrefix
            | Self::StripSuffix
            | Self::SplitOnce
            | Self::EqIgnoreAsciiCase => 1,
        }
    }
}

/// `char` methods that look only at ASCII or at the code point, so no
/// Unicode table is involved (design/01 §6). Each prints as `Char.<ts>(c,
/// ..)`; the receiver is the first argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharMethod {
    IsAscii,
    IsAsciiAlphabetic,
    IsAsciiAlphanumeric,
    IsAsciiControl,
    IsAsciiDigit,
    IsAsciiGraphic,
    IsAsciiHexdigit,
    IsAsciiLowercase,
    IsAsciiPunctuation,
    IsAsciiUppercase,
    /// Space, tab, LF, FF, CR. Not VT, unlike `is_whitespace` and JS `\s`.
    IsAsciiWhitespace,
    ToAsciiLowercase,
    ToAsciiUppercase,
    /// Takes `&char`.
    EqIgnoreAsciiCase,
    /// UTF-8 bytes, as `usize`.
    LenUtf8,
    /// Takes a `u32` radix; panics outside 2..=36 as Rust does.
    IsDigit,
    /// Takes a `u32` radix; `Option<u32>`.
    ToDigit,
}

impl CharMethod {
    pub const ALL: [CharMethod; 17] = [
        CharMethod::IsAscii,
        CharMethod::IsAsciiAlphabetic,
        CharMethod::IsAsciiAlphanumeric,
        CharMethod::IsAsciiControl,
        CharMethod::IsAsciiDigit,
        CharMethod::IsAsciiGraphic,
        CharMethod::IsAsciiHexdigit,
        CharMethod::IsAsciiLowercase,
        CharMethod::IsAsciiPunctuation,
        CharMethod::IsAsciiUppercase,
        CharMethod::IsAsciiWhitespace,
        CharMethod::ToAsciiLowercase,
        CharMethod::ToAsciiUppercase,
        CharMethod::EqIgnoreAsciiCase,
        CharMethod::LenUtf8,
        CharMethod::IsDigit,
        CharMethod::ToDigit,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::IsAscii => "is_ascii",
            Self::IsAsciiAlphabetic => "is_ascii_alphabetic",
            Self::IsAsciiAlphanumeric => "is_ascii_alphanumeric",
            Self::IsAsciiControl => "is_ascii_control",
            Self::IsAsciiDigit => "is_ascii_digit",
            Self::IsAsciiGraphic => "is_ascii_graphic",
            Self::IsAsciiHexdigit => "is_ascii_hexdigit",
            Self::IsAsciiLowercase => "is_ascii_lowercase",
            Self::IsAsciiPunctuation => "is_ascii_punctuation",
            Self::IsAsciiUppercase => "is_ascii_uppercase",
            Self::IsAsciiWhitespace => "is_ascii_whitespace",
            Self::ToAsciiLowercase => "to_ascii_lowercase",
            Self::ToAsciiUppercase => "to_ascii_uppercase",
            Self::EqIgnoreAsciiCase => "eq_ignore_ascii_case",
            Self::LenUtf8 => "len_utf8",
            Self::IsDigit => "is_digit",
            Self::ToDigit => "to_digit",
        }
    }

    /// Arguments after the receiver.
    pub fn args(self) -> usize {
        match self {
            Self::EqIgnoreAsciiCase | Self::IsDigit | Self::ToDigit => 1,
            _ => 0,
        }
    }

    /// The runtime's name: `Char.<ts_name>`.
    pub fn ts_name(self) -> &'static str {
        match self {
            Self::IsAscii => "isAscii",
            Self::IsAsciiAlphabetic => "isAsciiAlphabetic",
            Self::IsAsciiAlphanumeric => "isAsciiAlphanumeric",
            Self::IsAsciiControl => "isAsciiControl",
            Self::IsAsciiDigit => "isAsciiDigit",
            Self::IsAsciiGraphic => "isAsciiGraphic",
            Self::IsAsciiHexdigit => "isAsciiHexdigit",
            Self::IsAsciiLowercase => "isAsciiLowercase",
            Self::IsAsciiPunctuation => "isAsciiPunctuation",
            Self::IsAsciiUppercase => "isAsciiUppercase",
            Self::IsAsciiWhitespace => "isAsciiWhitespace",
            Self::ToAsciiLowercase => "toAsciiLowercase",
            Self::ToAsciiUppercase => "toAsciiUppercase",
            Self::EqIgnoreAsciiCase => "eqIgnoreAsciiCase",
            Self::LenUtf8 => "lenUtf8",
            Self::IsDigit => "isDigit",
            Self::ToDigit => "toDigit",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Callee {
    Fn(Name),
    /// `e as T` on a fieldless enum: each variant's discriminant as `to`,
    /// looked up by `kind`.
    Discriminant {
        to: IntTy,
        table: Vec<(Name, i128)>,
        /// The enum, whose `kind`s the table's keys are.
        of: Name,
    },
    /// A local binding holding a closure. `check::accept` rewrites
    /// `Callee::Fn` to this when a binding shadows the item.
    Local(Name),
    Int {
        ty: IntTy,
        op: IntOp,
    },
    /// Round an f64 result to f32 (`Math.fround`). The emitted call is `F32`.
    Fround,
    /// Cast a JS number that is already the right width (`(x) as F64`).
    AsFloat(FloatTy),
    /// A float method from the allow-list; the receiver is the argument.
    Float {
        ty: FloatTy,
        m: FloatMethod,
    },
    /// `f64::NAN` and the infinities, of `ty`.
    FloatConst {
        ty: FloatTy,
        c: FloatConst,
    },
    /// `f64::from(x)` / `f32::from(x)` as written; `check::accept` rewrites
    /// it to `IntToFloat` or `FloatToFloat` by the argument's type.
    FloatFrom(FloatTy),
    /// `x as T` from a float to an integer: toward zero, saturating at
    /// `T`'s bounds, NaN to 0, as Rust's `as` does. Prints as
    /// `Int.<t>.castFloat(x)`.
    FloatToInt {
        from: FloatTy,
        to: IntTy,
    },
    /// An integer to a float, by `as` or `from`: the nearest value, ties to
    /// even, rounded once, as Rust does.
    IntToFloat {
        from: IntTy,
        to: FloatTy,
    },
    /// `x as f32` from an `f64` (rounded, `Math.fround`), or `f64::from(x)`
    /// from an `f32` (exact).
    FloatToFloat {
        to: FloatTy,
    },
    Method {
        ty: Name,
        name: Name,
    },
    Variant {
        ty: Name,
        variant: Name,
    },
    StructNew(Name),
    ResultOk,
    ResultErr,
    OptionSome,
    OptionNone,
    /// `Vec::len`. The argument is the vector. Prints as `.length`.
    VecLen,
    /// `Vec::is_empty`. Prints as `.length === 0`.
    VecIsEmpty,
    /// `v.push(x)` on a local `let mut v: Vec<T>`, of type `()`: the one
    /// write to an array (design/01 §7.14). Prints as `v.push(x)`.
    VecPush,
    /// `v.insert(i, x)` on a local `let mut v: Vec<T>`, of type `()`: like
    /// `push`, a write to the local's own array, panicking past its end.
    /// Prints as `Slice.insert(v, i, x)`.
    VecInsert,
    /// `v.remove(i)` on a local `let mut v: Vec<T>`, of type `T`: a write to
    /// the local's own array, panicking at or past its end. Prints as
    /// `Slice.remove(v, i)`.
    VecRemove,
    /// `v[i] = x` on a local `let mut v: Vec<T>`, of type `()`: a write to
    /// the local's own array, panicking at or past its end (JS would grow
    /// it). `v[i] op= x` is `v[i] = v[i] op x`. Rust evaluates `x` before
    /// the place, so `emit` binds it first where both may panic. Prints as
    /// `Slice.set(v, i, x)`.
    VecSet,
    /// `String::new()`: the empty `String`. Prints as `""`.
    StringNew,
    /// A `pub` function's argument that may hold a string, checked on
    /// entry: a JS string may hold a lone surrogate, which no Rust `str`
    /// does, so it panics there rather than meeting the encoding later
    /// (design/01 §6). `emit` writes it, never `check`. Prints as
    /// `Str.wellFormed(x)`.
    StrWellFormed,
    /// `a` with `b` (a `char` or a `&str`) after it, a new `String`:
    /// `check::accept` writes `s.push(c)` and `s.push_str(t)` on a local `let
    /// mut s: String` as `s = StrConcat(s, c)`, an assignment like any other
    /// (design/01 §7.14). UTF-8 and UTF-16 both concatenate by code point.
    /// Prints as `a + b`.
    StrConcat,
    /// A `Vec<char>`'s chars as one `String`, what `collect::<String>()`
    /// gives over a sequence of `char`s. Prints as `v.join("")`.
    StrFromChars,
    /// `Option::is_some` / `is_none`. `Option<T>` is `T | null` (no nested
    /// `Option`), so each is one comparison with `null`.
    OptionIsSome,
    OptionIsNone,
    /// `str::as_bytes`, on a `String` or `&str`: the UTF-8 bytes as a
    /// `&[u8]`. Prints as `Str.bytes(s)` (design/01 §6).
    StrBytes,
    /// `s.split(c)` with a `char` separator, only as the source of a `for`:
    /// the pieces as `&str`, empty ones included. One code point cuts a
    /// well-formed string at the same places in UTF-8 and UTF-16, and JS
    /// `split` keeps the empty pieces Rust keeps. Prints as `s.split(c)`.
    StrSplit,
    /// `Ord` on two strings, by code point as Rust's UTF-8 bytes compare:
    /// -1, 0, or 1, as an `i32`. `check::accept` writes it for `<`, `<=`,
    /// `>`, `>=` on `String`/`&str`, compared with 0: JS orders strings by
    /// UTF-16 unit, which puts U+E000..=U+FFFF above the supplementary
    /// planes. Prints as `Str.cmp(a, b)`.
    StrCmp,
    /// `a.cmp(&b)`, giving std's `Ordering`: `a`, then `b`, each evaluated
    /// once. `text` for `char`, `String`/`&str`, and `Uuid`, ordered by code
    /// point (`Ord.cmpStr`); else an integer or `bool`, ordered by JS `<`
    /// (`Ord.cmp`).
    OrdCmp {
        text: bool,
    },
    /// `a.cmp(&b)` on two `Vec`s of what `OrdCmp` orders (`text` as
    /// there): element by element, then the shorter first, as std's
    /// lexicographic order. Prints as `Ord.cmpList(a, b, Ord.cmp)`.
    OrdCmpList {
        text: bool,
    },
    /// `o.then(p)`: `p` when `o` is `Equal`, else `o`; both evaluated, `o`
    /// first, as Rust evaluates a call's arguments. Prints as `Ord.then`.
    OrdThen,
    /// `all`, `any`, `position`, `count`, or `sum` on what `over` walks:
    /// the source, then (but for `count` and `sum`) the predicate, a
    /// closure of one parameter. Prints as `Iter.<method>`.
    Consume {
        method: Consume,
        over: Over,
    },
    /// `collect()` of what `over` walks (a `StrSplit`'s pieces, a `Vec`'s
    /// items, a string's chars or bytes, or the stages `IterMap` and
    /// `IterFilter` over one of those), then `f` if the last stage is
    /// `map(f)`, a closure of one parameter. With `result`, the target is
    /// `Result<Vec<T>, E>` and `f` returns `Result<T, E>`: `f` runs on the
    /// items in order and stops at the first `Err`, which is the result, as
    /// std's `FromIterator` for `Result` does. Always a new array (design/01
    /// §7.14). Prints as the array method where one stage runs, else
    /// `Array.from` or `Iter.tryCollect`.
    Collect {
        result: bool,
        over: Over,
    },
    /// `.map(f)` on what `over` walks: lazy, each item through `f` when the
    /// consumer asks for it (design/01 §7.13). The arguments are the source
    /// and `f`. Typed as a `Vec` of `f`'s results; only a consumer or
    /// another stage reads it. Prints as `Iter.map`.
    IterMap {
        over: Over,
    },
    /// `.filter(p)` on what `over` walks, lazy as `IterMap`. Prints as
    /// `Iter.filter`.
    IterFilter {
        over: Over,
    },
    /// `String::from(s)`. Prints as `s`: JS strings are already owned values.
    StringFrom,
    /// `&x[a..b]`, `&x[a..]`, `&x[..b]`, or `&x[..]` on a string (byte
    /// positions) or a `Vec` or slice. The arguments are `x`, then `a` if
    /// `start`, then `b` if `end`. Out-of-range positions, a reversed range,
    /// and (on a string) a position inside a character panic with Rust's
    /// messages, checked in Rust's order. `of` is set by `check::accept`.
    Slice {
        of: Option<SliceOf>,
        start: bool,
        end: bool,
    },
    /// A `str` method from the allow-list (design/01 §6). The first argument
    /// is the receiver, a `String` or `&str`.
    Str(StrMethod),
    /// `to::from(x)` where std has a lossless `From` (`IntTy::widens_to`).
    /// `from` is the argument's type, set by `check::accept`.
    IntFrom {
        from: Option<IntTy>,
        to: IntTy,
    },
    /// `x as to` between integer types where std has no `From`: the value
    /// modulo 2^bits of `to`, read as `to` reads it, as Rust's `as` wraps
    /// (to `usize`, Rust's 64 bits, then the 2^53 check every `usize`
    /// makes). Prints as `Int.<to>.cast(x)`.
    IntCast {
        from: IntTy,
        to: IntTy,
    },
    /// `u32::from(c)` / `u64::from(c)`: the code point. `check::accept`
    /// rewrites an `IntFrom` on a `char` to this, and wraps both sides of a
    /// `char` ordering in it: JS orders strings by UTF-16 unit, which puts
    /// U+E000..=U+FFFF above the supplementary planes.
    CharCode(IntTy),
    /// `char::from(b)` for a `u8`.
    CharFromU8,
    /// `char::from_u32(n)`: `None` for a surrogate or past U+10FFFF.
    CharFromU32,
    /// A `char` method from the allow-list; the receiver is the first argument.
    Char(CharMethod),
    /// `Uuid::parse_str(s)` / `Uuid::try_parse(s)`: the four forms `uuid`
    /// accepts, any case, to the canonical form. Prints as `Uuid.parseStr`.
    UuidParse,
    /// `Uuid::nil()`.
    UuidNil,
    /// `s.parse::<T>()` into an integer type: Rust's `from_str_radix(s, 10)`
    /// (an optional `+`, a `-` for a signed type, then ASCII digits, in
    /// range). Prints as `Int.<t>.parse(s)`.
    StrParse(IntTy),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fields {
    Unit,
    Positional(Vec<Expr>),
    Named(Vec<(Name, Expr)>),
}

impl Fields {
    fn is_inlinable(&self) -> bool {
        match self {
            Fields::Unit => true,
            Fields::Positional(xs) => xs.iter().all(Expr::is_inlinable),
            Fields::Named(xs) => xs.iter().all(|(_, x)| x.is_inlinable()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    Wildcard,
    Var(Name),
    Lit(Lit),
    Variant {
        ty: Name,
        variant: Name,
        bind: VariantBind,
    },
    OptionSome(Box<Pattern>),
    OptionNone,
    ResultOk(Box<Pattern>),
    ResultErr(Box<Pattern>),
    /// `A | B`: variants of one enum, binding no names. A trailing `_` arm
    /// on an enum becomes one after checking, naming every variant the
    /// other arms leave (`check::accept`).
    Or(Vec<Pattern>),
    /// `lo..=hi` (`inclusive`) or `lo..hi` on an integer. With `Lit`, only in
    /// a `match` on an integer or a `&str`, which must end in `_`.
    Range {
        lo: Lit,
        hi: Lit,
        inclusive: bool,
    },
    /// `(p, q)`: an arm of a `match` on a tuple, such as `match (state,
    /// event)`. Each element is `_`, a binding, or a pattern a `match` arm
    /// may have; tuples do not nest. `check::accept` splits the `match` into
    /// one `match` per element (`check::tuple`), so none reaches emit.
    Tuple(Vec<Pattern>),
}

impl Pattern {
    /// Can fail to match: anything but `_`, a name, or a tuple of those.
    pub fn refutable(&self) -> bool {
        match self {
            Pattern::Wildcard | Pattern::Var(_) => false,
            Pattern::Tuple(ps) => ps.iter().any(Pattern::refutable),
            _ => true,
        }
    }

    /// Tests something below its own case: a variant field, or the payload
    /// of `Some`, `Ok`, or `Err`, that can fail to match
    /// (`Some(Event::Pay { .. })`, `Checked { verified: false, .. }`). A
    /// `match` with such an arm is lowered into a decision tree
    /// (`check::tuple`), so none reaches emit.
    pub fn nests(&self) -> bool {
        match self {
            Pattern::Variant { bind, .. } => match bind {
                VariantBind::Unit => false,
                VariantBind::Tuple(ps) => ps.iter().any(Pattern::refutable),
                VariantBind::Struct(ps) => ps.iter().any(|(_, p)| p.refutable()),
            },
            Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => p.refutable(),
            Pattern::Or(ps) | Pattern::Tuple(ps) => ps.iter().any(Pattern::nests),
            Pattern::Wildcard | Pattern::Var(_) | Pattern::Lit(_) | Pattern::OptionNone | Pattern::Range { .. } => {
                false
            }
        }
    }

    /// A tuple pattern, or `|` of them: an arm of a `match` on a tuple.
    pub fn is_tuple_case(&self) -> bool {
        match self {
            Pattern::Tuple(_) => true,
            Pattern::Or(alts) => alts.iter().any(|a| matches!(a, Pattern::Tuple(_))),
            _ => false,
        }
    }

    /// The patterns an arm tests values with: a tuple's elements (of every
    /// alternative, for `|`), or the pattern itself.
    pub fn columns(&self) -> Vec<&Pattern> {
        match self {
            Pattern::Tuple(ps) => ps.iter().collect(),
            Pattern::Or(alts) if self.is_tuple_case() => alts.iter().flat_map(Pattern::columns).collect(),
            other => vec![other],
        }
    }

    /// An integer literal or range, or `|` of them: an arm of a `match` on
    /// an integer.
    pub fn is_int_case(&self) -> bool {
        match self {
            Pattern::Lit(Lit::Int { .. }) | Pattern::Range { lo: Lit::Int { .. }, .. } => true,
            Pattern::Or(alts) => alts.iter().all(Pattern::is_int_case),
            _ => false,
        }
    }

    /// A `char` literal or range, or `|` of them: an arm of a `match` on a
    /// `char`.
    pub fn is_char_case(&self) -> bool {
        match self {
            Pattern::Lit(Lit::Char(_)) | Pattern::Range { lo: Lit::Char(_), .. } => true,
            Pattern::Or(alts) => alts.iter().all(Pattern::is_char_case),
            _ => false,
        }
    }

    /// A string literal, or `|` of them: an arm of a `match` on a `&str`.
    pub fn is_str_case(&self) -> bool {
        match self {
            Pattern::Lit(Lit::Str(_)) => true,
            Pattern::Or(alts) => alts.iter().all(Pattern::is_str_case),
            _ => false,
        }
    }

    /// `true` or `false`, or `|` of them: an arm of a `match` on a `bool`.
    pub fn is_bool_case(&self) -> bool {
        match self {
            Pattern::Lit(Lit::Bool(_)) => true,
            Pattern::Or(alts) => alts.iter().all(Pattern::is_bool_case),
            _ => false,
        }
    }

    /// An arm tried by value in order, as an `if` chain: integer, `char`,
    /// string, or `bool`.
    pub fn is_lit_case(&self) -> bool {
        self.is_int_case() || self.is_char_case() || self.is_str_case() || self.is_bool_case()
    }

    /// Names the pattern binds, left to right.
    pub fn bindings(&self) -> Vec<&Name> {
        let mut out = Vec::new();
        self.collect_bindings(&mut out);
        out
    }

    fn collect_bindings<'a>(&'a self, out: &mut Vec<&'a Name>) {
        match self {
            Pattern::Var(n) => out.push(n),
            p => p.children().into_iter().for_each(|p| p.collect_bindings(out)),
        }
    }

    /// The patterns directly inside this one, left to right.
    pub fn children(&self) -> Vec<&Pattern> {
        match self {
            Pattern::Variant { bind: VariantBind::Tuple(ps), .. } | Pattern::Or(ps) | Pattern::Tuple(ps) => {
                ps.iter().collect()
            }
            Pattern::Variant { bind: VariantBind::Struct(ps), .. } => ps.iter().map(|(_, p)| p).collect(),
            Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => vec![p],
            Pattern::Variant { bind: VariantBind::Unit, .. }
            | Pattern::Wildcard
            | Pattern::Var(_)
            | Pattern::Lit(_)
            | Pattern::Range { .. }
            | Pattern::OptionNone => Vec::new(),
        }
    }

    /// `children`, mutably and in the same order.
    pub fn children_mut(&mut self) -> Vec<&mut Pattern> {
        match self {
            Pattern::Variant { bind: VariantBind::Tuple(ps), .. } | Pattern::Or(ps) | Pattern::Tuple(ps) => {
                ps.iter_mut().collect()
            }
            Pattern::Variant { bind: VariantBind::Struct(ps), .. } => ps.iter_mut().map(|(_, p)| p).collect(),
            Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => vec![p],
            Pattern::Variant { bind: VariantBind::Unit, .. }
            | Pattern::Wildcard
            | Pattern::Var(_)
            | Pattern::Lit(_)
            | Pattern::Range { .. }
            | Pattern::OptionNone => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariantBind {
    Unit,
    Tuple(Vec<Pattern>),
    Struct(Vec<(Name, Pattern)>),
}

/// Closure parameter. `ty` is filled by `check::accept` when not written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosureParam {
    pub name: Name,
    pub ty: Option<Ty>,
}

/// What `Callee::Slice` cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceOf {
    /// A `String` or `&str`, at UTF-8 byte positions. Prints as `Str.slice`.
    Str,
    /// A `Vec` or slice.
    Items,
}

/// A consuming iterator method (`Callee::Consume`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consume {
    All,
    Any,
    Position,
    Count,
    /// Adds the items of this integer type from zero, panicking on overflow.
    Sum(IntTy),
}

impl Consume {
    pub fn ts_name(self) -> &'static str {
        match self {
            Consume::All => "all",
            Consume::Any => "any",
            Consume::Position => "position",
            Consume::Count => "count",
            Consume::Sum(_) => "sum",
        }
    }
}

/// What a `for` walks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Over {
    /// `s.chars()`: each Unicode scalar value of a `String` or `&str`, as a
    /// `char` (design/01 §6).
    Chars,
    /// `s.bytes()`: each UTF-8 byte of a `String` or `&str`, as a `u8`.
    Bytes,
    /// `xs`, `&xs`, or `xs.iter()` on a `Vec` or slice: each element.
    Items,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pattern: Pattern,
    /// `p if guard =>`: tried only when `pattern` matched, with its
    /// bindings in scope. `check::accept` lowers every guard into the
    /// decision tree of `check::tuple`, so none reaches emit.
    pub guard: Option<Expr>,
    pub body: Expr,
}

impl Arm {
    pub fn new(pattern: Pattern, body: Expr) -> Self {
        Arm { pattern, guard: None, body }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Lit(Lit),
    Var(Name),
    Let {
        name: Name,
        mutable: bool,
        ty: Option<Ty>,
        value: Box<Expr>,
        then: Box<Expr>,
    },
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<Arm>,
    },
    Call {
        callee: Callee,
        args: Vec<Expr>,
    },
    /// `receiver.name(args)`. `check::accept` resolves it by the receiver's
    /// type into a `Callee::Method` call with the receiver first.
    MethodCall {
        receiver: Box<Expr>,
        name: Name,
        args: Vec<Expr>,
    },
    /// `S { fields, ..base }`. `base` is evaluated after `fields`, as in Rust.
    /// It is `None` when every field is written out. Enum variants have no base:
    /// Rust rejects functional record update on them.
    Construct {
        ty: Name,
        variant: Option<Name>,
        fields: Fields,
        base: Option<Box<Expr>>,
    },
    Field {
        base: Box<Expr>,
        name: Name,
    },
    /// `base[index]` where `base` is a `Vec`. Evaluated base, then index.
    /// Out of range throws Rust's slice-index panic message.
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// `expr as to`. Only a fieldless enum to an integer that holds every
    /// discriminant is accepted; `check::accept` rewrites it into a
    /// `Callee::Discriminant` call, so none reaches emit.
    Cast {
        expr: Box<Expr>,
        to: Ty,
    },
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    /// `|params| body`. Captured bindings are never `let mut`, so capturing
    /// by value (Rust `move`) and by reference (JS) agree. `?` and `return`
    /// in `body` leave the closure; `ret` is filled by `check::accept`.
    Closure {
        params: Vec<ClosureParam>,
        ret: Option<Ty>,
        body: Box<Expr>,
    },
    /// Arithmetic here is JS arithmetic, which matches Rust only for `f64`.
    /// `check::accept` rewrites integer and `f32` arithmetic into
    /// `Callee::Int` and `Callee::Fround` calls.
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnOp,
        expr: Box<Expr>,
    },
    Return(Box<Expr>),
    /// `name = value`, of type `()`. `name` is a `let mut` binding.
    Assign {
        name: Name,
        value: Box<Expr>,
    },
    /// `for var in start..end { body }`, of type `()`. `start` and `end`
    /// are one integer type and are evaluated once, in that order, before
    /// the first iteration. `var` is not `mut`; `body` runs for its effect.
    /// `ty` is the bounds' type, filled by `check::accept`.
    For {
        var: Name,
        ty: Option<IntTy>,
        start: Box<Expr>,
        end: Box<Expr>,
        body: Box<Expr>,
    },
    /// `for var in <source> { body }` over what `over` says, of type `()`.
    /// `source` is evaluated once; `var` takes each item in order.
    ForEach {
        var: Name,
        over: Over,
        source: Box<Expr>,
        body: Box<Expr>,
    },
    /// `while cond { body }`, of type `()`. `cond` runs before every pass.
    While {
        cond: Box<Expr>,
        body: Box<Expr>,
    },
    /// `break` and `continue` of the innermost loop, without a label or a
    /// value; of type `!`, and only where a statement may stand.
    Break,
    Continue,
    /// `first; then`: `first` runs for its effect, its value is dropped.
    Seq {
        first: Box<Expr>,
        then: Box<Expr>,
    },
    /// `expr?`. `on` is filled by `check::accept`, which also lifts every
    /// `Try` into the value of its own `Let`.
    Try {
        expr: Box<Expr>,
        on: Option<TryOn>,
    },
    /// `Box::new` / `Arc::new`. The value is `expr`.
    /// Emit prints `wrapper`'s comment and then `expr`.
    Ignored {
        wrapper: Wrapper,
        expr: Box<Expr>,
    },
    Unreachable,
    /// The `//` lines written directly above a statement or a block's tail,
    /// one string per line without the `//`. It stands as `first` of a
    /// `Seq` whose `then` is that statement; of type `()` and no effect.
    /// Emit prints it where `then` is printed as a statement.
    Comment(Vec<String>),
    /// Where `expr` starts in the source: a statement, a block's tail, or a
    /// `match` arm. Only the parser's spanned output has it, for diagnostics;
    /// `check::accept` removes it, so later passes never see it.
    At {
        at: Pos,
        expr: Box<Expr>,
    },
}

/// A 1-based line and column in the source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryOn {
    Result,
    Option,
}

impl Expr {
    /// Types written in this node itself, not in its subexpressions: a
    /// `let` annotation, a closure's parameter and return types, the target
    /// of `as`. With `children`, what a pass that collects names walks.
    pub fn own_types(&self) -> Vec<&Ty> {
        match self {
            Expr::Let { ty, .. } => ty.iter().collect(),
            Expr::Closure { params, ret, .. } => params.iter().filter_map(|p| p.ty.as_ref()).chain(ret).collect(),
            Expr::Cast { to, .. } => vec![to],
            Expr::Lit(_)
            | Expr::Var(_)
            | Expr::If { .. }
            | Expr::Match { .. }
            | Expr::Call { .. }
            | Expr::MethodCall { .. }
            | Expr::Construct { .. }
            | Expr::Field { .. }
            | Expr::Index { .. }
            | Expr::Tuple(_)
            | Expr::Array(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::Assign { .. }
            | Expr::Return(_)
            | Expr::Try { .. }
            | Expr::Seq { .. }
            | Expr::For { .. }
            | Expr::ForEach { .. }
            | Expr::While { .. }
            | Expr::Break
            | Expr::Continue
            | Expr::Unreachable
            | Expr::Comment(_)
            | Expr::Ignored { .. }
            | Expr::At { .. } => Vec::new(),
        }
    }

    /// Direct subexpressions in evaluation order.
    pub fn children(&self) -> Vec<&Expr> {
        match self {
            Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable | Expr::Comment(_) | Expr::Break | Expr::Continue => {
                Vec::new()
            }
            Expr::Let { value, then, .. } => vec![value, then],
            Expr::If { cond, then, else_ } => vec![cond, then, else_],
            Expr::Match { scrutinee, arms } => std::iter::once(&**scrutinee)
                .chain(arms.iter().flat_map(|a| a.guard.iter().chain(std::iter::once(&a.body))))
                .collect(),
            Expr::Call { args, .. } | Expr::Tuple(args) | Expr::Array(args) => args.iter().collect(),
            Expr::MethodCall { receiver, args, .. } => std::iter::once(&**receiver).chain(args).collect(),
            Expr::Construct { fields, base, .. } => {
                let mut out = match fields {
                    Fields::Unit => Vec::new(),
                    Fields::Positional(xs) => xs.iter().collect(),
                    Fields::Named(xs) => xs.iter().map(|(_, x)| x).collect(),
                };
                if let Some(b) = base {
                    out.push(b);
                }
                out
            }
            Expr::Field { base, .. }
            | Expr::Unary { expr: base, .. }
            | Expr::Return(base)
            | Expr::Try { expr: base, .. } => vec![base],
            Expr::Index { base, index } => vec![base, index],
            Expr::Cast { expr, .. } => vec![expr],
            Expr::Binary { left, right, .. } => vec![left, right],
            Expr::Assign { value, .. } => vec![value],
            Expr::Seq { first, then } => vec![first, then],
            Expr::For { start, end, body, .. } => vec![start, end, body],
            Expr::ForEach { source: string, body, .. } => vec![string, body],
            Expr::While { cond, body } => vec![cond, body],
            Expr::Closure { body, .. } => vec![body],
            Expr::Ignored { expr, .. } | Expr::At { expr, .. } => vec![expr],
        }
    }

    /// `children`, mutably and in the same order.
    pub fn children_mut(&mut self) -> Vec<&mut Expr> {
        match self {
            Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable | Expr::Comment(_) | Expr::Break | Expr::Continue => {
                Vec::new()
            }
            Expr::Let { value, then, .. } => vec![value, then],
            Expr::If { cond, then, else_ } => vec![cond, then, else_],
            Expr::Match { scrutinee, arms } => std::iter::once(&mut **scrutinee)
                .chain(arms.iter_mut().flat_map(|a| a.guard.iter_mut().chain(std::iter::once(&mut a.body))))
                .collect(),
            Expr::Call { args, .. } | Expr::Tuple(args) | Expr::Array(args) => args.iter_mut().collect(),
            Expr::MethodCall { receiver, args, .. } => std::iter::once(&mut **receiver).chain(args).collect(),
            Expr::Construct { fields, base, .. } => {
                let mut out = match fields {
                    Fields::Unit => Vec::new(),
                    Fields::Positional(xs) => xs.iter_mut().collect(),
                    Fields::Named(xs) => xs.iter_mut().map(|(_, x)| x).collect(),
                };
                if let Some(b) = base {
                    out.push(b);
                }
                out
            }
            Expr::Field { base, .. }
            | Expr::Unary { expr: base, .. }
            | Expr::Return(base)
            | Expr::Try { expr: base, .. } => vec![base],
            Expr::Index { base, index } => vec![base, index],
            Expr::Cast { expr, .. } => vec![expr],
            Expr::Binary { left, right, .. } => vec![left, right],
            Expr::Assign { value, .. } => vec![value],
            Expr::Seq { first, then } => vec![first, then],
            Expr::For { start, end, body, .. } => vec![start, end, body],
            Expr::ForEach { source: string, body, .. } => vec![string, body],
            Expr::While { cond, body } => vec![cond, body],
            Expr::Closure { body, .. } => vec![body],
            Expr::Ignored { expr, .. } | Expr::At { expr, .. } => vec![expr],
        }
    }

    /// Searches this expression in preorder: `f` answers for a node
    /// (`Some`) or leaves it to the node's children (`None`), and the search
    /// is true when some answer is.
    pub fn search(&self, mut f: impl FnMut(&Expr) -> Option<bool>) -> bool {
        fn go(e: &Expr, f: &mut dyn FnMut(&Expr) -> Option<bool>) -> bool {
            f(e).unwrap_or_else(|| e.children().into_iter().any(|c| go(c, f)))
        }
        go(self, &mut f)
    }

    /// Whether `f` holds for this expression or one inside it.
    pub fn any(&self, mut f: impl FnMut(&Expr) -> bool) -> bool {
        self.search(|e| f(e).then_some(true))
    }

    /// Calls `f` on this expression and every one inside it, in preorder.
    pub fn walk(&self, mut f: impl FnMut(&Expr)) {
        self.search(|e| {
            f(e);
            None
        });
    }

    /// Contains a `?` or `return` that leaves the function or closure this
    /// is the body of (not one nested in it).
    pub fn exits(&self) -> bool {
        self.search(|e| match e {
            Expr::Try { .. } | Expr::Return(_) => Some(true),
            Expr::Closure { .. } => Some(false),
            _ => None,
        })
    }

    /// Whether this expression or one inside it reads `name`.
    pub fn reads(&self, name: &Name) -> bool {
        self.any(|e| matches!(e, Expr::Var(n) if n == name))
    }

    /// Whether this expression or one inside it assigns `name`.
    pub fn assigns(&self, name: &Name) -> bool {
        self.any(|e| matches!(e, Expr::Assign { name: n, .. } if n == name))
    }

    /// Names this node itself binds, not its subexpressions: a `let`, a
    /// loop variable, an arm's pattern, a closure's parameters.
    pub fn own_bindings(&self) -> Vec<&Name> {
        match self {
            Expr::Let { name, .. } | Expr::For { var: name, .. } | Expr::ForEach { var: name, .. } => vec![name],
            Expr::Match { arms, .. } => arms.iter().flat_map(|a| a.pattern.bindings()).collect(),
            Expr::Closure { params, .. } => params.iter().map(|p| &p.name).collect(),
            _ => Vec::new(),
        }
    }

    /// The local a `v.push(x)`, `v.insert(i, x)`, `v.remove(i)`, or `v[i] =
    /// x` writes.
    pub fn grown(&self) -> Option<&Name> {
        match self {
            Expr::Call { callee: Callee::VecPush | Callee::VecInsert | Callee::VecRemove | Callee::VecSet, args } => {
                match args.first() {
                    Some(Expr::Var(n)) => Some(n),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Subexpressions evaluated every time this one is, before it produces a
    /// value: a `?` there can be hoisted in front without changing meaning.
    /// Excludes branches and the right side of `&&`/`||`.
    pub fn strict_children(&self) -> Vec<&Expr> {
        match self {
            Expr::Binary { op: BinOp::And | BinOp::Or, left, .. } => vec![left],
            Expr::If { .. }
            | Expr::Match { .. }
            | Expr::Let { .. }
            | Expr::Return(_)
            | Expr::Assign { .. }
            | Expr::Seq { .. }
            | Expr::Closure { .. } => Vec::new(),
            // The body runs zero or more times; the bounds always run.
            Expr::For { start, end, .. } => vec![start, end],
            Expr::ForEach { source: string, .. } => vec![string],
            // The condition runs at least once; the body maybe not.
            Expr::While { cond, .. } => vec![cond],
            _ => self.children(),
        }
    }

    /// Contains a `?` that the lifting pass hoists out of this expression.
    pub fn lifts(&self) -> bool {
        matches!(self, Expr::Try { .. }) || self.strict_children().into_iter().any(Expr::lifts)
    }

    /// Printed as JS statements rather than a JS expression.
    pub fn needs_statements(&self) -> bool {
        match self {
            // A comment above a value is left out where the value is printed
            // as an expression.
            Expr::Seq { first, then } if matches!(**first, Expr::Comment(_)) => then.needs_statements(),
            Expr::Match { .. }
            | Expr::Let { .. }
            | Expr::Return(_)
            | Expr::Try { .. }
            | Expr::Assign { .. }
            | Expr::For { .. }
            | Expr::ForEach { .. }
            | Expr::While { .. }
            | Expr::Break
            | Expr::Continue
            | Expr::Seq { .. } => true,
            Expr::If { then, else_, .. } => [then, else_].into_iter().any(|b| b.needs_statements() || b.lifts()),
            Expr::Ignored { expr, .. } | Expr::At { expr, .. } => expr.needs_statements(),
            _ => false,
        }
    }

    /// Evaluating it has no effect and cannot panic: a name, a literal, a
    /// field of one of those, or a variant / `Some` / `Ok` / `Err` of those.
    /// `unwrap_or` / `ok_or` may print it in the arm instead of a temporary.
    pub fn is_inlinable(&self) -> bool {
        match self.unpositioned() {
            Expr::Var(_) | Expr::Lit(_) | Expr::Closure { .. } => true,
            Expr::Field { base, .. } => base.is_inlinable(),
            Expr::Tuple(xs) | Expr::Array(xs) => xs.iter().all(Self::is_inlinable),
            Expr::Construct { fields, base, .. } => {
                base.as_ref().is_none_or(|b| b.is_inlinable()) && fields.is_inlinable()
            }
            Expr::Call { callee: Callee::OptionNone, .. } => true,
            Expr::Call { callee: Callee::OptionSome | Callee::ResultOk | Callee::ResultErr, args } => {
                args.iter().all(Self::is_inlinable)
            }
            Expr::Cast { expr, .. } | Expr::Ignored { expr, .. } => expr.is_inlinable(),
            _ => false,
        }
    }

    /// Removes every `At`, keeping what it wraps.
    pub fn strip_positions(&mut self) {
        while let Expr::At { expr, .. } = self {
            *self = std::mem::replace(&mut **expr, Expr::Unreachable);
        }
        self.children_mut().into_iter().for_each(Expr::strip_positions);
    }

    /// `At` wrappers removed from the outside of this expression only.
    pub fn unpositioned(&self) -> &Expr {
        match self {
            Expr::At { expr, .. } => expr.unpositioned(),
            other => other,
        }
    }

    pub fn var(name: impl Into<String>) -> Self {
        Expr::Var(Name::new(name))
    }

    pub fn int(n: i64) -> Self {
        Expr::Lit(Lit::Int { value: n.into(), ty: None, byte: false, hex: false })
    }
}
