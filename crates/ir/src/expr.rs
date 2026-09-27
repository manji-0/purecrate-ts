use crate::name::Name;
use crate::ty::{FloatTy, IntTy, Ty};

/// Numeric literals carry their Rust type once known: from the source suffix,
/// or filled in by `check::accept`. An integer without one prints as a JS
/// `number`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Lit {
    Bool(bool),
    Int { value: i128, ty: Option<IntTy> },
    Float { digits: String, ty: Option<FloatTy> },
    Str(String),
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

/// Integer arithmetic with Rust debug-build semantics: truncating division,
/// and a throw wherever Rust would panic (overflow, zero divisor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Neg,
}

impl IntOp {
    pub fn arity(self) -> usize {
        match self {
            IntOp::Neg => 1,
            _ => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            IntOp::Add => "add",
            IntOp::Sub => "sub",
            IntOp::Mul => "mul",
            IntOp::Div => "div",
            IntOp::Rem => "rem",
            IntOp::Neg => "neg",
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
    /// Round an f64 result to f32 (`Math.fround`).
    Fround,
    Method { ty: Name, name: Name },
    Variant { ty: Name, variant: Name },
    StructNew(Name),
    ResultOk,
    ResultErr,
    OptionSome,
    OptionNone,
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
}

impl Pattern {
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
            Pattern::Wildcard | Pattern::Lit(_) | Pattern::OptionNone => {}
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
    Unreachable,
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
            Expr::Binary { left, right, .. } => vec![left, right],
            Expr::Assign { value, .. } => vec![value],
            Expr::Seq { first, then } => vec![first, then],
            Expr::Closure { body, .. } => vec![body],
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
            Expr::Binary { left, right, .. } => vec![left, right],
            Expr::Assign { value, .. } => vec![value],
            Expr::Seq { first, then } => vec![first, then],
            Expr::Closure { body, .. } => vec![body],
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
            | Expr::Seq { .. } => true,
            Expr::If { then, else_, .. } => [then, else_]
                .into_iter()
                .any(|b| b.needs_statements() || b.lifts()),
            _ => false,
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
