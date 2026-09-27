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
    Construct {
        ty: Name,
        variant: Option<Name>,
        fields: Fields,
    },
    Field {
        base: Box<Expr>,
        name: Name,
    },
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
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
    Unreachable,
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
            Expr::Construct { fields, .. } => match fields {
                Fields::Unit => Vec::new(),
                Fields::Positional(xs) => xs.iter().collect(),
                Fields::Named(xs) => xs.iter().map(|(_, x)| x).collect(),
            },
            Expr::Field { base, .. } | Expr::Unary { expr: base, .. } | Expr::Return(base) => {
                vec![base]
            }
            Expr::Binary { left, right, .. } => vec![left, right],
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
