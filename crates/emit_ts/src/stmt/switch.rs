//! `match` as a `switch`, or as a value (`??`, a ternary) where it is one.

use super::*;
use crate::tx::{Op, Tx};

pub(crate) fn emit_switch(
    scrutinee: &Expr,
    arms: &[purecrate_ir::Arm],
    indent: usize,
    sink: Sink,
    tail: bool,
    out: &mut String,
) {
    // The matched value's temporary needs no block of its own: `plain`
    // numbers it where another `match` at the same depth has one.
    emit_switch_in(scrutinee, arms, indent, sink, tail || matches!(sink, Sink::Return), out);
}

/// `const tmp = scrutinee;` before a `match` on a value that is not a place.
pub(super) fn bind_scrutinee(tmp: &str, scrutinee: &Expr, arms: &[purecrate_ir::Arm], indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    let value = emit_expr(scrutinee, indent);
    // An annotated literal initializer narrows the union and makes the
    // other cases unreachable; a cast keeps the full union.
    let decl = match (scrutinee_ty(arms), peel(scrutinee)) {
        (Some(ty), Expr::Construct { .. }) => format!("{tmp} = {value} as {}", ty.as_str()),
        // A call says its type already.
        (Some(ty), v) if !declares_return(v) => format!("{tmp}: {} = {value}", ty.as_str()),
        // `match { let t: Result<A, B> = Err(b); t } { .. }` prints `t`'s
        // value: its annotation is the only place TS learns `A`.
        _ => match peeled_ty(scrutinee) {
            Some(ty) => format!("{tmp}: {} = {}", emit_ty(ty), crate::tidy::strip_outer(&value)),
            None => format!("{tmp} = {}", crate::tidy::strip_outer(&value)),
        },
    };
    out.push_str(&format!("{pad}const {decl};\n"));
}

/// `value` as one expression, after the statements (`prelude`) that bind
/// what it reads once: a `match` on a value that is not a place, each arm an
/// expression, binds the value and is a `?:` on that binding.
pub(crate) fn value_expr(value: &Expr, indent: usize) -> Option<(String, String)> {
    let value = peel_identity(value);
    if let Some(s) = as_expr(value, indent) {
        return Some((String::new(), s));
    }
    let Expr::Match { scrutinee, arms } = value else { return None };
    if is_place(scrutinee) {
        return None;
    }
    if let Some(s) = coalesced(value, indent) {
        return Some((String::new(), s));
    }
    let tmp = match_temp(arms, indent);
    let on_tmp = Expr::Match { scrutinee: Box::new(Expr::Var(Name::new(tmp.clone()))), arms: arms.clone() };
    let s = as_expr(&on_tmp, indent)?;
    let mut prelude = String::new();
    bind_scrutinee(&tmp, scrutinee, arms, indent, &mut prelude);
    Some((prelude, s))
}

/// A `match` on a call that reads the value once, `call ?? d` or the call
/// itself, so it needs no binding: the arms copy what `Some` holds.
pub(super) fn coalesced(value: &Expr, indent: usize) -> Option<String> {
    let Expr::Match { scrutinee, arms } = peel_identity(value) else { return None };
    if is_place(scrutinee) {
        return None;
    }
    let tmp = match_temp(arms, indent);
    let on_tmp = Expr::Match { scrutinee: Box::new(Expr::Var(Name::new(tmp.clone()))), arms: arms.clone() };
    let rest = match as_tx(&on_tmp, indent)? {
        Tx::Atom(s) if s == tmp => None,
        Tx::Bin(Op::Coalesce, l, r) if *l == Tx::atom(tmp.clone()) => Some(*r),
        _ => return None,
    };
    let Tx::Atom(call) = emit_tx(scrutinee, indent) else { return None };
    if rest.as_ref().is_some_and(|r| mentions_word(&r.print(), &tmp)) {
        return None;
    }
    let call = Tx::atom(crate::tidy::strip_outer(&call));
    Some(match rest {
        Some(rest) => Tx::bin(Op::Coalesce, call, rest).print(),
        None => call.print(),
    })
}

/// Whether `text` holds `word` as an identifier of its own.
fn mentions_word(text: &str, word: &str) -> bool {
    let part = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    text.match_indices(word).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(part) && !text[at + word.len()..].chars().next().is_some_and(part)
    })
}

/// `let t = v; let a = t[0]; let b = t[1]; ..` where nothing else reads
/// `t`: the names by element (`None` for one no name takes), and the rest.
pub(super) fn destructured<'e>(t: &Name, mut then: &'e Expr) -> Option<(Vec<Option<&'e Name>>, &'e Expr)> {
    let mut slots: Vec<Option<&Name>> = Vec::new();
    while let Expr::Let { name, mutable: false, value, then: rest, .. } = then {
        let Expr::Field { base, name: field } = value.as_ref() else { break };
        let Some(i) =
            field.as_str().strip_prefix('[').and_then(|f| f.strip_suffix(']')).and_then(|f| f.parse::<usize>().ok())
        else {
            break;
        };
        if !matches!(base.as_ref(), Expr::Var(n) if n == t) {
            break;
        }
        if slots.len() <= i {
            slots.resize(i + 1, None);
        }
        if slots[i].is_some() {
            break;
        }
        slots[i] = Some(name);
        then = rest;
    }
    (slots.iter().filter(|s| s.is_some()).count() >= 2 && !mentions(then, t)).then_some((slots, then))
}

/// `matches!(v, p)` or `!matches!(v, p)` on a value that is not a place,
/// each arm an expression (a guard's included): the value, the arms, and
/// whether it is negated.
pub(super) fn hoistable_test(cond: &Expr) -> Option<(&Expr, &[purecrate_ir::Arm], bool)> {
    let (inner, negated) = match peel_identity(cond) {
        Expr::Unary { op: purecrate_ir::UnOp::Not, expr } => (peel_identity(expr), true),
        other => (other, false),
    };
    match inner {
        Expr::Match { scrutinee, arms } if !is_place(scrutinee) && arms.iter().all(|a| !a.body.needs_statements()) => {
            Some((scrutinee, arms, negated))
        }
        _ => None,
    }
}

pub(crate) fn emit_switch_in(
    scrutinee: &Expr,
    arms: &[purecrate_ir::Arm],
    indent: usize,
    sink: Sink,
    tail: bool,
    out: &mut String,
) {
    assert!(arms.iter().all(|a| a.guard.is_none()), "guards are lowered in check::accept");
    let pad = "  ".repeat(indent);
    let pad1 = "  ".repeat(indent + 1);
    let subject = if is_place(scrutinee) {
        emit_expr(scrutinee, indent)
    } else {
        let tmp = match_temp(arms, indent);
        bind_scrutinee(&tmp, scrutinee, arms, indent, out);
        tmp
    };
    if let [a, b] = arms {
        if let (Some(test), Some(_)) = (two_way_tx(&a.pattern, &subject), two_way_tx(&b.pattern, &subject)) {
            let pad2 = "  ".repeat(indent + 1);
            // `Some(x) => a, None => return e`: the exit first, then `a` with
            // its payload read where the exit has narrowed it, as `let ..
            // else` reads. Only where `a` declares nothing the block's later
            // statements could meet.
            if let Some(a_body) = exit_first(a, b, scrutinee) {
                let b_test = two_way_tx(&b.pattern, &subject).expect("two-way");
                let (b_prelude, b_body) = read_in_place(&b.pattern, scrutinee, &b.body, &subject, &pad2);
                let branches = vec![
                    Branch { test: Some(b_test), prelude: b_prelude, body: &b_body },
                    Branch { test: None, prelude: String::new(), body: &a_body },
                ];
                emit_branches(&branches, indent, sink, tail, out);
                return;
            }
            let (a_prelude, a_body) = read_in_place(&a.pattern, scrutinee, &a.body, &subject, &pad2);
            let (b_prelude, b_body) = read_in_place(&b.pattern, scrutinee, &b.body, &subject, &pad2);
            let mut branches = vec![Branch { test: Some(test), prelude: a_prelude, body: &a_body }];
            if !(matches!(sink, Sink::Effect) && b.body == Expr::Lit(Lit::Unit)) {
                branches.push(Branch { test: None, prelude: b_prelude, body: &b_body });
            }
            emit_branches(&branches, indent, sink, tail, out);
            return;
        }
    }
    if arms.iter().any(|a| a.pattern.is_lit_case()) {
        emit_lit_chain(&subject, arms, indent, sink, tail, out);
        return;
    }
    // An enum of one variant: TS does not narrow a type that is not a union,
    // so a `switch` would leave its `default` reachable to `assertNever`.
    // The one arm is all there is.
    if let [arm] = arms {
        if let Pattern::Variant { bind, .. } = &arm.pattern {
            out.push_str(&bind_prelude(bind, &subject, &pad));
            emit_stmts(&arm.body, indent, sink, out);
            return;
        }
    }
    // One variant and the rest (`_`, or the others the decision tree lists):
    // an `if` on the variant, then the rest.
    if let Some((a, b)) = one_and_rest(arms) {
        if let Pattern::Variant { variant, bind, .. } = &a.pattern {
            let kind = format!("{subject}.kind");
            let lit = format!("\"{}\"", variant.as_str());
            let prelude = bind_prelude(bind, &subject, &pad1);
            // The rest one exit and the variant's arm statements: the exit
            // first, as a guard, then the arm where TS has narrowed it.
            let exits = (matches!(sink, Sink::Return) || ends_in_jump(&b.body)) && !b.body.needs_statements();
            // `true` on one side and `false` on the other is the test.
            if let (Expr::Lit(Lit::Bool(x)), Expr::Lit(Lit::Bool(y)), true, false) =
                (&a.body, &b.body, prelude.is_empty(), matches!(sink, Sink::Effect))
            {
                if x != y {
                    let op = if *x { "===" } else { "!==" };
                    return sink.finish(&format!("{kind} {op} {lit}"), &pad, out);
                }
            }
            let branches = if exits && (a.body.needs_statements() || !prelude.is_empty()) {
                vec![
                    Branch {
                        test: Some(Tx::bin(Op::Ne, Tx::atom(kind.clone()), Tx::atom(lit.clone()))),
                        prelude: String::new(),
                        body: &b.body,
                    },
                    Branch { test: None, prelude, body: &a.body },
                ]
            } else {
                vec![
                    Branch {
                        test: Some(Tx::bin(Op::Eq, Tx::atom(kind.clone()), Tx::atom(lit.clone()))),
                        prelude,
                        body: &a.body,
                    },
                    Branch { test: None, prelude: String::new(), body: &b.body },
                ]
            };
            emit_branches(&branches, indent, sink, tail, out);
            return;
        }
    }
    out.push_str(&format!("{pad}switch ({subject}.kind) {{\n"));
    let remainder = remainder_arm(arms);
    // `A | B` binds nothing: its cases share one body, as do the cases of
    // arms that bind nothing and do the same (`Succeeded` and `Canceled`).
    let mut groups: Vec<(Vec<&Name>, Option<&VariantBind>, &purecrate_ir::Arm)> = Vec::new();
    for arm in arms {
        if remainder.is_some_and(|r| std::ptr::eq(r, arm)) {
            continue;
        }
        let (variants, bind) = match &arm.pattern {
            Pattern::Variant { variant, bind, .. } => (vec![variant], Some(bind)),
            Pattern::Or(alts) => (
                alts.iter()
                    .filter_map(|alt| match alt {
                        Pattern::Variant { variant, .. } => Some(variant),
                        _ => None,
                    })
                    .collect(),
                None,
            ),
            _ => continue,
        };
        let shared = arm
            .pattern
            .bindings()
            .is_empty()
            .then(|| groups.iter_mut().find(|(_, _, g)| g.pattern.bindings().is_empty() && g.body == arm.body));
        match shared.flatten() {
            Some((vs, _, _)) => vs.extend(variants),
            None => groups.push((variants, bind, arm)),
        }
    }
    for (variants, bind, arm) in groups {
        if let Some((last, first)) = variants.split_last() {
            for v in first {
                out.push_str(&format!("{pad1}case \"{v}\":\n", v = v.as_str()));
            }
            out.push_str(&format!("{pad1}case \"{v}\":", v = last.as_str()));
            let prelude = bind.map(|bind| bind_prelude(bind, &subject, &"  ".repeat(indent + 2))).unwrap_or_default();
            out.push_str(&case_body(&prelude, &arm.body, indent, sink));
        }
    }
    if let Some(arm) = remainder {
        out.push_str(&format!("{pad1}default:{}", case_body("", &arm.body, indent, sink)));
        out.push_str(&format!("{pad}}}\n"));
    } else {
        out.push_str(&format!("{pad1}default:\n{pad1}  return assertNever({subject});\n{pad}}}\n"));
    }
}

/// What follows a `case ..:` of a `switch` at `indent`: the arm's
/// statements and its `break`, in a block only where they declare a name,
/// which would otherwise be seen by the other cases.
fn case_body(prelude: &str, body: &Expr, indent: usize, sink: Sink) -> String {
    let pad1 = "  ".repeat(indent + 1);
    let mut stmts = prelude.to_string();
    emit_stmts(body, indent + 2, sink, &mut stmts);
    if !matches!(sink, Sink::Return) && !ends_in_jump(body) {
        stmts.push_str(&format!("{pad1}  break;\n"));
    }
    if declares(&stmts, &"  ".repeat(indent + 2)) {
        format!(" {{\n{stmts}{pad1}}}\n")
    } else {
        format!("\n{stmts}")
    }
}

/// Whether printed statements at `pad` declare a name at their top.
pub(super) fn declares(stmts: &str, pad: &str) -> bool {
    stmts.lines().any(|l| {
        let l = l.strip_prefix(pad).unwrap_or(l);
        l.starts_with("const ") || l.starts_with("let ")
    })
}

/// The first arm's body with its payload read from `scrutinee`, where the
/// other arm binds nothing and jumps, `scrutinee` is a place the first arm
/// does not reassign, and the body declares nothing at its top.
fn exit_first(a: &purecrate_ir::Arm, b: &purecrate_ir::Arm, scrutinee: &Expr) -> Option<Expr> {
    if !ends_in_jump(&b.body) || !b.pattern.bindings().is_empty() || ends_in_jump(&a.body) {
        return None;
    }
    let mut root = scrutinee;
    while let Expr::Field { base, .. } = root {
        root = base;
    }
    let Expr::Var(var) = root else { return None };
    if touches(&a.body, var) || declares_at_top(&a.body) {
        return None;
    }
    let mut body = a.body.clone();
    match &a.pattern {
        Pattern::OptionSome(p) => crate::expr::bind_in(p, scrutinee.clone(), &mut body)?,
        Pattern::ResultOk(p) | Pattern::ResultErr(p) => {
            let field = if matches!(a.pattern, Pattern::ResultOk(_)) { "value" } else { "error" };
            let read = Expr::Field { base: Box::new(scrutinee.clone()), name: purecrate_ir::Name::new(field) };
            crate::expr::bind_in(p, read, &mut body)?
        }
        _ => return None,
    }
    Some(body)
}

/// The one `A | B | …` (or `_`) that binds nothing, when other arms name
/// variants: it is the Rust `_`, printed as `default` instead of listing
/// every remaining case and `assertNever`.
/// The arm of one variant and the arm of the rest that binds nothing, of a
/// two-arm `match` on an enum. The rest may come first where it is the
/// other variants, which the one does not meet.
fn one_and_rest(arms: &[purecrate_ir::Arm]) -> Option<(&purecrate_ir::Arm, &purecrate_ir::Arm)> {
    let [a, b] = arms else { return None };
    let others = |rest: &Pattern, one: &Name| match rest {
        Pattern::Or(alts) => alts.iter().all(|p| matches!(p, Pattern::Variant { variant, .. } if variant != one)),
        _ => false,
    };
    match (&a.pattern, &b.pattern) {
        (Pattern::Variant { .. }, Pattern::Wildcard | Pattern::Or(_)) if b.pattern.bindings().is_empty() => {
            Some((a, b))
        }
        (rest, Pattern::Variant { variant, .. }) if others(rest, variant) && rest.bindings().is_empty() => Some((b, a)),
        _ => None,
    }
}

fn remainder_arm(arms: &[purecrate_ir::Arm]) -> Option<&purecrate_ir::Arm> {
    let named = arms.iter().any(|a| matches!(a.pattern, Pattern::Variant { .. }));
    if !named {
        return None;
    }
    let mut found = None;
    for a in arms {
        let catch_all = a.pattern.bindings().is_empty()
            && matches!(a.pattern, Pattern::Or(_) | Pattern::Wildcard)
            && !declares_at_top(&a.body);
        if catch_all {
            if found.is_some() {
                return None;
            }
            found = Some(a);
        }
    }
    found
}
