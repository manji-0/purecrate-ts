//! The operators of a printed TS expression, as a tree. The printer builds
//! one where it writes an operator (`emit_tx`), and parentheses come from
//! its shape: an operand is parenthesized only where its operator binds
//! looser than its parent's, or on the side that would regroup. Negating a
//! test (`!(a === b)` as `a !== b`, De Morgan on `&&`) works on the tree
//! too. Before, both read the printed text back (`tidy::top_prec`), which
//! missed operators a call or a `match` printed (`is_some` as `!== null`).
//!
//! Everything that binds tighter than every operator here (a name, a call,
//! a member, a literal, anything the printer already parenthesized) is an
//! `Atom`, kept as text.

use crate::tidy::{
    needs_paren, Assoc, Side, PREC_ADD, PREC_AND, PREC_ATOMIC, PREC_COALESCE, PREC_EQ, PREC_MUL, PREC_OR, PREC_REL,
    PREC_TERNARY, PREC_UNARY,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Mul,
    Div,
    Rem,
    Add,
    Sub,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Coalesce,
}

impl Op {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Op::Mul => "*",
            Op::Div => "/",
            Op::Rem => "%",
            Op::Add => "+",
            Op::Sub => "-",
            Op::Lt => "<",
            Op::Le => "<=",
            Op::Gt => ">",
            Op::Ge => ">=",
            Op::Eq => "===",
            Op::Ne => "!==",
            Op::And => "&&",
            Op::Or => "||",
            Op::Coalesce => "??",
        }
    }

    pub(crate) fn prec(self) -> u8 {
        match self {
            Op::Mul | Op::Div | Op::Rem => PREC_MUL,
            Op::Add | Op::Sub => PREC_ADD,
            Op::Lt | Op::Le | Op::Gt | Op::Ge => PREC_REL,
            Op::Eq | Op::Ne => PREC_EQ,
            Op::And => PREC_AND,
            Op::Or => PREC_OR,
            Op::Coalesce => PREC_COALESCE,
        }
    }
}

/// How a `?:` sets its parts apart. `Fold` is the `?:` a `match` or an
/// `if` of expressions becomes, which also parenthesizes a `??` in any
/// part, as oxfmt prints it; `Plain` is an `if` printed as written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CondStyle {
    Plain,
    Fold,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tx {
    Atom(String),
    Bin(Op, Box<Tx>, Box<Tx>),
    Not(Box<Tx>),
    Neg(Box<Tx>),
    Cond(CondStyle, Box<Tx>, Box<Tx>, Box<Tx>),
}

impl Tx {
    pub(crate) fn atom(s: impl Into<String>) -> Tx {
        Tx::Atom(s.into())
    }

    pub(crate) fn bin(op: Op, l: Tx, r: Tx) -> Tx {
        Tx::Bin(op, Box::new(l), Box::new(r))
    }

    pub(crate) fn prec(&self) -> u8 {
        match self {
            Tx::Atom(_) => PREC_ATOMIC,
            Tx::Bin(op, ..) => op.prec(),
            Tx::Not(_) | Tx::Neg(_) => PREC_UNARY,
            Tx::Cond(..) => PREC_TERNARY,
        }
    }

    pub(crate) fn is_cond(&self) -> bool {
        matches!(self, Tx::Cond(..))
    }

    /// `!self`: a comparison turned round (`a !== b` for `a === b`), else
    /// `!` in front.
    pub(crate) fn not(self) -> Tx {
        match self {
            Tx::Bin(Op::Eq, l, r) => Tx::Bin(Op::Ne, l, r),
            Tx::Bin(Op::Ne, l, r) => Tx::Bin(Op::Eq, l, r),
            other => Tx::Not(Box::new(other)),
        }
    }

    /// `!self` with an `&&` chain as the `||` of each side turned round.
    pub(crate) fn not_all(self) -> Tx {
        match self {
            Tx::Bin(Op::And, l, r) => Tx::bin(Op::Or, l.not_all(), r.not_all()),
            other => other.not(),
        }
    }

    /// `self` as an operand of an operator of `parent` precedence.
    pub(crate) fn operand(&self, parent: u8, assoc: Assoc, side: Side) -> String {
        let s = self.print();
        // An equality inside an equality is parenthesized, as oxfmt prints
        // `(a !== null) === b`.
        let nested_eq = parent == PREC_EQ && self.prec() == PREC_EQ;
        if nested_eq || needs_paren(self.prec(), parent, assoc, side) {
            format!("({s})")
        } else {
            s
        }
    }

    pub(crate) fn print(&self) -> String {
        match self {
            Tx::Atom(s) => s.clone(),
            Tx::Bin(op, l, r) => format!(
                "{} {} {}",
                l.operand(op.prec(), Assoc::Left, Side::Left),
                op.text(),
                r.operand(op.prec(), Assoc::Left, Side::Right)
            ),
            Tx::Not(x) | Tx::Neg(x) => {
                let o = if matches!(self, Tx::Not(_)) { "!" } else { "-" };
                let inner = x.operand(PREC_UNARY, Assoc::Right, Side::Right);
                if inner.starts_with(o) {
                    format!("{o}({inner})")
                } else {
                    format!("{o}{inner}")
                }
            }
            Tx::Cond(style, c, t, e) => {
                let part = |x: &Tx, side| {
                    let s = x.operand(PREC_TERNARY, Assoc::Right, side);
                    if *style == CondStyle::Fold && x.prec() == PREC_COALESCE {
                        format!("({s})")
                    } else {
                        s
                    }
                };
                format!("{} ? {} : {}", part(c, Side::Left), part(t, Side::Left), part(e, Side::Right))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> Tx {
        Tx::atom(s)
    }

    #[test]
    fn a_call_printed_as_a_comparison_is_grouped_by_its_operator() {
        // `r.ok().is_some()` under a `?:`: the comparison is a node, so the
        // choice it holds is parenthesized (0.9.1's bug, by construction).
        let choice = Tx::Cond(CondStyle::Plain, Box::new(a("c")), Box::new(a("x")), Box::new(a("null")));
        let some = Tx::bin(Op::Ne, choice, a("null"));
        assert_eq!(some.print(), "(c ? x : null) !== null");
        assert_eq!(Tx::bin(Op::Eq, some.clone(), a("b")).print(), "((c ? x : null) !== null) === b");
        assert_eq!(some.not().print(), "(c ? x : null) === null");
    }

    #[test]
    fn coalesce_is_parenthesized_beside_logical_operators() {
        let d = Tx::bin(Op::Coalesce, a("o"), a("d"));
        assert_eq!(Tx::bin(Op::Or, d.clone(), a("b")).print(), "(o ?? d) || b");
        assert_eq!(Tx::bin(Op::Coalesce, Tx::bin(Op::And, a("a"), a("b")), a("d")).print(), "(a && b) ?? d");
        let fold = Tx::Cond(CondStyle::Fold, Box::new(a("c")), Box::new(d.clone()), Box::new(a("e")));
        assert_eq!(fold.print(), "c ? (o ?? d) : e");
    }

    #[test]
    fn negation_turns_comparisons_and_and_chains_round() {
        let t = Tx::bin(Op::And, Tx::bin(Op::Eq, a("a"), a("1")), Tx::bin(Op::And, a("b"), a("c")));
        assert_eq!(t.not_all().print(), "a !== 1 || !b || !c");
        assert_eq!(Tx::Not(Box::new(Tx::Not(Box::new(a("x"))))).print(), "!(!x)");
        assert_eq!(Tx::Neg(Box::new(Tx::bin(Op::Sub, a("a"), a("b")))).print(), "-(a - b)");
        assert_eq!(Tx::bin(Op::Sub, a("a"), Tx::bin(Op::Sub, a("b"), a("c"))).print(), "a - (b - c)");
    }
}
