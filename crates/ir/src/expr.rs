use crate::name::Name;
use crate::ty::{FloatTy, IntTy, Ty, Wrapper};

/// Numeric literals carry their Rust type once known: from the source suffix,
/// or filled in by `check::accept`. An integer without one prints as a JS
/// `number`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lit {
    Bool(bool),
    Int { value: i128, ty: Option<IntTy> },
    Float { digits: String, ty: Option<FloatTy> },
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
}

impl IntOp {
    pub fn arity(self) -> usize {
        match self {
            IntOp::Neg | IntOp::Not => 1,
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
}

impl StrMethod {
    pub const ALL: [StrMethod; 6] = [
        StrMethod::Len,
        StrMethod::IsEmpty,
        StrMethod::StartsWith,
        StrMethod::EndsWith,
        StrMethod::Contains,
        StrMethod::AsStr,
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
        }
    }

    /// Arguments after the receiver.
    pub fn needles(self) -> usize {
        match self {
            Self::Len | Self::IsEmpty | Self::AsStr => 0,
            Self::StartsWith | Self::EndsWith | Self::Contains => 1,
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
    /// A local binding holding a closure. `check::accept` rewrites
    /// `Callee::Fn` to this when a binding shadows the item.
    Local(Name),
    Int { ty: IntTy, op: IntOp },
    /// Round an f64 result to f32 (`Math.fround`). The emitted call is `F32`.
    Fround,
    /// Cast a JS number that is already the right width (`(x) as F64`).
    AsFloat(FloatTy),
    Method { ty: Name, name: Name },
    Variant { ty: Name, variant: Name },
    StructNew(Name),
    ResultOk,
    ResultErr,
    OptionSome,
    OptionNone,
    /// `Vec::len`. The argument is the vector. Prints as `.length`.
    VecLen,
    /// `Vec::is_empty`. Prints as `.length === 0`.
    VecIsEmpty,
    /// `Option::is_some` / `is_none`. `Option<T>` is `T | null` (no nested
    /// `Option`), so each is one comparison with `null`.
    OptionIsSome,
    OptionIsNone,
    /// `str::as_bytes`, on a `String` or `&str`: the UTF-8 bytes as a
    /// `&[u8]`. Prints as `Str.bytes(s)` (design/01 §6).
    StrBytes,
    /// `String::from(s)`. Prints as `s`: JS strings are already owned values.
    StringFrom,
    /// A `str` method from the allow-list (design/01 §6). The first argument
    /// is the receiver, a `String` or `&str`.
    Str(StrMethod),
    /// `to::from(x)` where std has a lossless `From` (`IntTy::widens_to`).
    /// `from` is the argument's type, set by `check::accept`.
    IntFrom { from: Option<IntTy>, to: IntTy },
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fields {
    Unit,
    Positional(Vec<Expr>),
    Named(Vec<(Name, Expr)>),
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
    Range { lo: Lit, hi: Lit, inclusive: bool },
    /// `(p, q)`: an arm of a `match` on a tuple, such as `match (state,
    /// event)`. Each element is `_`, a binding, or a pattern a `match` arm
    /// may have; tuples do not nest. `check::accept` splits the `match` into
    /// one `match` per element (`check::tuple`), so none reaches emit.
    Tuple(Vec<Pattern>),
}

impl Pattern {
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

    /// An arm tried by value in order, as an `if` chain: integer, `char`,
    /// or string.
    pub fn is_lit_case(&self) -> bool {
        self.is_int_case() || self.is_char_case() || self.is_str_case()
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
            Pattern::Variant { bind, .. } => match bind {
                VariantBind::Unit => {}
                VariantBind::Tuple(ps) => ps.iter().for_each(|p| p.collect_bindings(out)),
                VariantBind::Struct(ps) => ps.iter().for_each(|(_, p)| p.collect_bindings(out)),
            },
            Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => {
                p.collect_bindings(out)
            }
            Pattern::Or(ps) | Pattern::Tuple(ps) => ps.iter().for_each(|p| p.collect_bindings(out)),
            Pattern::Wildcard | Pattern::Lit(_) | Pattern::Range { .. } | Pattern::OptionNone => {}
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: Expr,
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
    /// `for var in string.chars() { body }`, of type `()`. `string` is a
    /// `String` or `&str`, evaluated once; `var` is each Unicode scalar value
    /// as a `char`, in order (design/01 §6).
    ForChars {
        var: Name,
        string: Box<Expr>,
        body: Box<Expr>,
    },
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
    /// `Box::new` / `Arc::new` / `Mutex::new`. The value is `expr`.
    /// Emit prints `wrapper`'s comment and then `expr`.
    Ignored {
        wrapper: Wrapper,
        expr: Box<Expr>,
    },
    Unreachable,
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
    /// Direct subexpressions in evaluation order.
    pub fn children(&self) -> Vec<&Expr> {
        match self {
            Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable => Vec::new(),
            Expr::Let { value, then, .. } => vec![value, then],
            Expr::If { cond, then, else_ } => vec![cond, then, else_],
            Expr::Match { scrutinee, arms } => std::iter::once(&**scrutinee)
                .chain(arms.iter().map(|a| &a.body))
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
            },
            Expr::Field { base, .. }
            | Expr::Unary { expr: base, .. }
            | Expr::Return(base)
            | Expr::Try { expr: base, .. } => vec![base],
            Expr::Index { base, index } => vec![base, index],
            Expr::Binary { left, right, .. } => vec![left, right],
            Expr::Assign { value, .. } => vec![value],
            Expr::Seq { first, then } => vec![first, then],
            Expr::For { start, end, body, .. } => vec![start, end, body],
            Expr::ForChars { string, body, .. } => vec![string, body],
            Expr::Closure { body, .. } => vec![body],
            Expr::Ignored { expr, .. } | Expr::At { expr, .. } => vec![expr],
        }
    }

    /// `children`, mutably and in the same order.
    pub fn children_mut(&mut self) -> Vec<&mut Expr> {
        match self {
            Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable => Vec::new(),
            Expr::Let { value, then, .. } => vec![value, then],
            Expr::If { cond, then, else_ } => vec![cond, then, else_],
            Expr::Match { scrutinee, arms } => std::iter::once(&mut **scrutinee)
                .chain(arms.iter_mut().map(|a| &mut a.body))
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
            },
            Expr::Field { base, .. }
            | Expr::Unary { expr: base, .. }
            | Expr::Return(base)
            | Expr::Try { expr: base, .. } => vec![base],
            Expr::Index { base, index } => vec![base, index],
            Expr::Binary { left, right, .. } => vec![left, right],
            Expr::Assign { value, .. } => vec![value],
            Expr::Seq { first, then } => vec![first, then],
            Expr::For { start, end, body, .. } => vec![start, end, body],
            Expr::ForChars { string, body, .. } => vec![string, body],
            Expr::Closure { body, .. } => vec![body],
            Expr::Ignored { expr, .. } | Expr::At { expr, .. } => vec![expr],
        }
    }

    /// Subexpressions evaluated every time this one is, before it produces a
    /// value: a `?` there can be hoisted in front without changing meaning.
    /// Excludes branches and the right side of `&&`/`||`.
    pub fn strict_children(&self) -> Vec<&Expr> {
        match self {
            Expr::Binary {
                op: BinOp::And | BinOp::Or,
                left,
                ..
            } => vec![left],
            Expr::If { .. }
            | Expr::Match { .. }
            | Expr::Let { .. }
            | Expr::Return(_)
            | Expr::Assign { .. }
            | Expr::Seq { .. }
            | Expr::Closure { .. } => Vec::new(),
            // The body runs zero or more times; the bounds always run.
            Expr::For { start, end, .. } => vec![start, end],
            Expr::ForChars { string, .. } => vec![string],
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
            Expr::Match { .. }
            | Expr::Let { .. }
            | Expr::Return(_)
            | Expr::Try { .. }
            | Expr::Assign { .. }
            | Expr::For { .. }
            | Expr::ForChars { .. }
            | Expr::Seq { .. } => true,
            Expr::If { then, else_, .. } => [then, else_]
                .into_iter()
                .any(|b| b.needs_statements() || b.lifts()),
            Expr::Ignored { expr, .. } | Expr::At { expr, .. } => expr.needs_statements(),
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
        Expr::Lit(Lit::Int {
            value: n.into(),
            ty: None,
        })
    }
}
