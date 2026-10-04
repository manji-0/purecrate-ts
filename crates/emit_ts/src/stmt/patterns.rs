//! A pattern's test and the bindings it reads, as TS.

use super::*;
use crate::tx::{Op, Tx};

/// The test for an integer, `char`, `bool`, or string arm; `None` for `_`.
pub(crate) fn lit_tx(pattern: &Pattern, subject: &str) -> Option<Tx> {
    let at = || Tx::atom(subject);
    let range = |lo: String, hi: String, inclusive: bool, read: &dyn std::ops::Fn() -> Tx| {
        Tx::bin(
            Op::And,
            Tx::bin(Op::Ge, read(), Tx::atom(lo)),
            Tx::bin(if inclusive { Op::Le } else { Op::Lt }, read(), Tx::atom(hi)),
        )
    };
    match pattern {
        // A `bool` reads as itself: `x`, `!x`.
        Pattern::Lit(Lit::Bool(true)) => Some(at()),
        Pattern::Lit(Lit::Bool(false)) => Some(Tx::Not(Box::new(at()))),
        Pattern::Lit(lit) => Some(Tx::bin(Op::Eq, at(), Tx::atom(bare_lit(lit)))),
        // By code point: JS orders strings by UTF-16 unit.
        Pattern::Range { lo: Lit::Char(lo), hi: Lit::Char(hi), inclusive } => {
            Some(range(u32::from(*lo).to_string(), u32::from(*hi).to_string(), *inclusive, &|| {
                Tx::atom(format!("Char.code({subject})"))
            }))
        }
        Pattern::Range { lo, hi, inclusive } => Some(range(bare_lit(lo), bare_lit(hi), *inclusive, &at)),
        // A range is an `&&`, parenthesized under the `||` by the tree.
        Pattern::Or(alts) => alts.iter().filter_map(|a| lit_tx(a, subject)).reduce(|a, b| Tx::bin(Op::Or, a, b)),
        _ => None,
    }
}

/// `Option` is `T | null` and `Result` is `kind`-tagged, so both narrow
/// with a single test.
pub(crate) fn two_way_tx(pattern: &Pattern, subject: &str) -> Option<Tx> {
    let at = || Tx::atom(subject);
    match pattern {
        Pattern::OptionSome(_) => Some(Tx::bin(Op::Ne, at(), Tx::atom("null"))),
        Pattern::OptionNone => Some(Tx::bin(Op::Eq, at(), Tx::atom("null"))),
        Pattern::ResultOk(_) => Some(Tx::bin(Op::Eq, Tx::atom(format!("{subject}.kind")), Tx::atom("\"Ok\""))),
        Pattern::ResultErr(_) => Some(Tx::bin(Op::Eq, Tx::atom(format!("{subject}.kind")), Tx::atom("\"Err\""))),
        _ => None,
    }
}

pub(crate) fn two_way_test(pattern: &Pattern, subject: &str) -> Option<String> {
    two_way_tx(pattern, subject).map(|t| t.print())
}

/// An arm's binding and body. `Some(m)` on a variable is the variable
/// itself, narrowed: the body reads it in place of `m` (`if (method !==
/// null)` then `method`, not `const m = method`) where nothing in the body
/// binds or assigns the variable or captures it in a closure, which TS may
/// not narrow. A field or a `Result` payload is read once, into the name.
pub(super) fn read_in_place(
    pattern: &Pattern,
    scrutinee: &Expr,
    body: &Expr,
    subject: &str,
    pad: &str,
) -> (String, Expr) {
    if let (Pattern::OptionSome(inner), Expr::Var(var)) = (pattern, scrutinee) {
        if let Pattern::Var(n) = &**inner {
            if !touches(body, var) {
                return (String::new(), subst(body, n, scrutinee));
            }
        }
    }
    (two_way_prelude(pattern, subject, pad), body.clone())
}

pub(crate) fn two_way_prelude(pattern: &Pattern, subject: &str, pad: &str) -> String {
    let (inner, read) = match pattern {
        Pattern::OptionSome(p) => (p, subject.to_string()),
        Pattern::ResultOk(p) => (p, format!("{subject}.value")),
        Pattern::ResultErr(p) => (p, format!("{subject}.error")),
        _ => return String::new(),
    };
    match &**inner {
        Pattern::Var(n) => format!("{pad}const {} = {read};\n", n.as_str()),
        Pattern::Tuple(elems) => tuple_names(elems, &read, pad),
        _ => String::new(),
    }
}

/// `const a = read[0]` for each name in a tuple of names and `_`.
fn tuple_names(elems: &[Pattern], read: &str, pad: &str) -> String {
    elems
        .iter()
        .enumerate()
        .filter_map(|(i, p)| match p {
            Pattern::Var(n) => Some(format!("{pad}const {} = {read}[{i}];\n", n.as_str())),
            _ => None,
        })
        .collect()
}

/// Field `i` of a tuple variant of `n` fields: `.value` when it is the
/// only one, else `.content[i]`.
pub(crate) fn tuple_variant_field(subject: &str, i: usize, n: usize) -> String {
    if n == 1 {
        format!("{subject}.value")
    } else {
        format!("{subject}.content[{i}]")
    }
}

pub(crate) fn bind_prelude(bind: &VariantBind, subject: &str, pad: &str) -> String {
    match bind {
        VariantBind::Unit => String::new(),
        VariantBind::Tuple(pats) => pats
            .iter()
            .enumerate()
            .filter_map(|(i, p)| match p {
                Pattern::Var(n) => Some(format!(
                    "{pad}const {name} = {};\n",
                    tuple_variant_field(subject, i, pats.len()),
                    name = n.as_str()
                )),
                Pattern::Tuple(elems) => Some(tuple_names(elems, &tuple_variant_field(subject, i, pats.len()), pad)),
                _ => None,
            })
            .collect(),
        VariantBind::Struct(pairs) => pairs
            .iter()
            .filter_map(|(field, p)| match p {
                Pattern::Var(n) => {
                    Some(format!("{pad}const {name} = {subject}.{f};\n", name = n.as_str(), f = field.as_str()))
                }
                _ => None,
            })
            .collect(),
    }
}
