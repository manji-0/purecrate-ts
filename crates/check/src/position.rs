//! `return` and `?` leave the function. The printer keeps them in the
//! function's own JS body only where the Rust expression is a statement (a
//! tail, a `let` value, a `match`/`if` arm there) or, for `?`, a strict
//! subexpression of one (hoisted by `lift`). Anywhere else the printer would
//! wrap them in an arrow function, where they would leave only that.
//! Assignment and expression statements are statements for the same reason.

use purecrate_ir::{Crate, Expr, Item, Pos, Reason};

use crate::Diagnostic;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// Printed as statements of the function body.
    Stmt,
    /// Printed as a JS expression evaluated on every run of its statement.
    Strict,
    /// Printed inside a conditional or an arrow function.
    Nested,
}

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        if let Item::Fn(f) = item {
            visit(&f.body, Ctx::Stmt, None, &mut |m, at| {
                let mut d = Diagnostic::at(i, Reason::Position, m);
                d.at = at;
                out.push(d);
            });
        }
    }
    out
}

/// `at` is the innermost `Expr::At` around `expr`.
/// A `?` or `return` in `expr` that is not inside a closure.
fn exits(expr: &Expr) -> bool {
    match expr {
        Expr::Try { .. } | Expr::Return(_) => true,
        Expr::Closure { .. } => false,
        other => other.children().into_iter().any(exits),
    }
}

fn visit(expr: &Expr, ctx: Ctx, at: Option<Pos>, report: &mut impl FnMut(String, Option<Pos>)) {
    // A guard is tested inside the decision tree; a `?` there would have to
    // leave from the middle of it.
    if let Expr::Match { arms, .. } = expr {
        if arms.iter().filter_map(|a| a.guard.as_ref()).any(exits) {
            report("`?` or `return` inside a match guard is not in v0; bind the value with `let` before the `match`".into(), at);
        }
    }
    match expr {
        Expr::At { at, expr } => visit(expr, ctx, Some(*at), report),
        Expr::Return(value) => {
            if ctx != Ctx::Stmt {
                report(
                    "`return` inside a larger expression is not in v0; \
                     make it the value of an arm, a `let`, or the tail"
                        .into(),
                    at,
                );
            }
            visit(value, Ctx::Strict, at, report);
        }
        Expr::Try { expr, .. } => {
            if ctx == Ctx::Nested {
                report(
                    "`?` inside `&&`, `||`, or an `if`/`match` used within a larger expression \
                     is not in v0; bind it with `let` first"
                        .into(),
                    at,
                );
            }
            visit(expr, Ctx::Strict, at, report);
        }
        Expr::Assign { value, .. } => {
            if ctx != Ctx::Stmt {
                report(
                    "assignment inside a larger expression is not in v0; \
                     write it as its own statement"
                        .into(),
                    at,
                );
            }
            visit(value, value_ctx(value), at, report);
        }
        Expr::For { start, end, body, .. } => {
            if ctx != Ctx::Stmt {
                report("`for` inside a larger expression is not in v0; write it as its own statement".into(), at);
            }
            visit(start, Ctx::Strict, at, report);
            visit(end, Ctx::Strict, at, report);
            visit(body, Ctx::Stmt, at, report);
        }
        Expr::ForEach { source: string, body, .. } => {
            if ctx != Ctx::Stmt {
                report("`for` inside a larger expression is not in v0; write it as its own statement".into(), at);
            }
            visit(string, Ctx::Strict, at, report);
            visit(body, Ctx::Stmt, at, report);
        }
        Expr::While { cond, body } => {
            if ctx != Ctx::Stmt {
                report("`while` inside a larger expression is not in v0; write it as its own statement".into(), at);
            }
            // `lift` moves a `?` in the condition inside the loop.
            visit(cond, Ctx::Strict, at, report);
            visit(body, Ctx::Stmt, at, report);
        }
        Expr::Break | Expr::Continue if ctx != Ctx::Stmt => report(
            "`break` or `continue` inside a larger expression is not in v0; \
             make it a statement, an arm of a `match` or `if` there, or the tail"
                .into(),
            at,
        ),
        Expr::Seq { first, then } if ctx == Ctx::Stmt => {
            visit(first, Ctx::Stmt, at, report);
            visit(then, Ctx::Stmt, at, report);
        }
        Expr::Let { value, then, .. } if ctx == Ctx::Stmt => {
            visit(value, value_ctx(value), at, report);
            visit(then, Ctx::Stmt, at, report);
        }
        Expr::If { cond, then, else_ } if ctx == Ctx::Stmt => {
            visit(cond, Ctx::Strict, at, report);
            let branch = if expr.needs_statements() {
                Ctx::Stmt
            } else {
                Ctx::Nested
            };
            visit(then, branch, at, report);
            visit(else_, branch, at, report);
        }
        Expr::Match { scrutinee, arms } if ctx == Ctx::Stmt => {
            visit(scrutinee, Ctx::Strict, at, report);
            for a in arms {
                if let Some(guard) = &a.guard {
                    visit(guard, Ctx::Strict, at, report);
                }
                visit(&a.body, Ctx::Stmt, at, report);
            }
        }
        Expr::Closure { body, .. } => visit(body, Ctx::Stmt, at, report),
        Expr::Let { .. } | Expr::If { .. } | Expr::Match { .. } | Expr::Seq { .. } => expr
            .children()
            .into_iter()
            .for_each(|c| visit(c, Ctx::Nested, at, report)),
        _ => {
            let strict = expr.strict_children();
            for child in expr.children() {
                let inner = if ctx != Ctx::Nested && strict.iter().any(|s| std::ptr::eq(*s, child)) {
                    Ctx::Strict
                } else {
                    Ctx::Nested
                };
                visit(child, inner, at, report);
            }
        }
    }
}

fn value_ctx(value: &Expr) -> Ctx {
    if value.needs_statements() {
        Ctx::Stmt
    } else {
        Ctx::Strict
    }
}
