//! Hoists every `?` into the value of its own `let`, in evaluation order, so
//! the printer only meets `let x = e?` and can emit an early `return`.
//! `position` has already rejected `?` in places this pass cannot reach.

use std::collections::HashSet;

use purecrate_ir::{Arm, BinOp, Callee, Crate, Expr, Fields, Fn, Item, Lit, Name, TryOn, UnOp};

pub fn lift(krate: Crate) -> Crate {
    let items = krate
        .items
        .into_iter()
        .map(|item| match item {
            Item::Fn(f) => Item::Fn(Fn { body: lift_body(f.body), ..f }),
            other => other,
        })
        .collect();
    Crate::new(krate.name.as_str(), items)
}

/// A closure body is printed as its own arrow body, where `?` returns from
/// the closure, so each is lifted on its own, innermost first.
fn lift_body(mut body: Expr) -> Expr {
    lift_closures(&mut body);
    let mut body = Lifter::for_body(&body).stmt(body);
    let mut taken = bound_names(&body);
    // The guard puts `ok_or`'s argument in a `let` of its own: a `?` in it
    // (`o.ok_or(a / r?)?`) is lifted from there, before the test, as Rust
    // evaluates the argument whether or not the option is `Some`.
    if guard_ok_or(&mut body, &mut taken) {
        body = Lifter::for_body(&body).stmt(body);
    }
    body
}

/// `let x = opt.ok_or(e)?` as a guard: `ok_or` lowers to a `match` that
/// builds a `Result` only for `?` to take it apart again, which printed as
/// an inline function. `e` still runs before the test, as `ok_or` is eager:
///
/// ```text
/// let $opt = opt; let $arg = e;
/// if $opt.is_none() { return Err($arg) }
/// let x = $opt;
/// ```
///
/// `rename` saw `$opt` and `$arg` inside the `?`, each in a scope of its
/// own, so two of them may have one name; here they share the function's
/// block, so each gets a made name (`$opt_1`) no other binding of the body
/// has, which `plain_names` keeps apart. `rename` numbers its own made names
/// the same way (`$opt`, `$opt_1`), so `taken` holds every name bound.
/// Whether an argument it moved holds a `?`, which is still to be lifted.
fn guard_ok_or(expr: &mut Expr, taken: &mut HashSet<String>) -> bool {
    let mut tries = false;
    if let Expr::Let { name, mutable, ty, value, then } = expr {
        if let Some((opt, opt_ty, recv, arg, arg_ty, e)) = ok_or_try(value) {
            // `map_err(f)?` is a `match` that returns by now, not a `Try`.
            tries |= e.exits();
            // An option typing could not name (`$opt`, the receiver a call)
            // is named after the local it is for: `qtyOpt`, `qtyOr`.
            // A shadow `x$1` prints as `x2`, so its option is `x2Opt`.
            let local = match name.as_str().rsplit_once('$') {
                Some((b, k)) if !b.is_empty() && k.bytes().all(|c| c.is_ascii_digit()) => {
                    format!("{b}{}", k.parse::<usize>().map_or(0, |k| k + 1))
                }
                Some(_) => String::new(),
                None => name.as_str().to_string(),
            };
            let mut fresh = |n: &Name| {
                let base =
                    n.as_str().trim_start_matches('$').trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
                let base = match base {
                    "opt" if !local.is_empty() => format!("{local}Opt"),
                    "optOr" if !local.is_empty() => format!("{local}Or"),
                    b => b.to_string(),
                };
                let name = (1..).map(|k| format!("${base}_{k}")).find(|n| !taken.contains(n)).expect("a free name");
                taken.insert(name.clone());
                Name::new(name)
            };
            let (opt, arg) = (fresh(&opt), fresh(&arg));
            // An immutable local holds the option itself: past the test, TS
            // reads it as the value (`const qty = ..; if (qty === null)
            // return ..;`), with no copy. A `let mut` keeps its own type.
            // A made local (a tuple's, a hoisted value's) and a receiver
            // that is a place, read in place, keep the made option.
            let direct =
                !*mutable && !name.as_str().starts_with('$') && !matches!(recv, Expr::Var(_) | Expr::Field { .. });
            let opt = if direct { name.clone() } else { opt };
            let then = std::mem::replace(&mut **then, Expr::Unreachable);
            let guard = Expr::If {
                cond: Box::new(Expr::Call { callee: Callee::OptionIsNone, args: vec![Expr::Var(opt.clone())] }),
                then: Box::new(Expr::Return(Box::new(Expr::Call {
                    callee: Callee::ResultErr,
                    args: vec![Expr::Var(arg.clone())],
                }))),
                else_: Box::new(Expr::Lit(Lit::Unit)),
            };
            let bind = if !direct {
                Expr::Let {
                    name: name.clone(),
                    mutable: *mutable,
                    ty: ty.clone(),
                    value: Box::new(Expr::Var(opt.clone())),
                    then: Box::new(then),
                }
            } else {
                then
            };
            *expr = Expr::Let {
                name: opt,
                mutable: false,
                ty: opt_ty,
                value: Box::new(recv),
                then: Box::new(Expr::Let {
                    name: arg,
                    mutable: false,
                    ty: arg_ty,
                    value: Box::new(e),
                    then: Box::new(Expr::Seq { first: Box::new(guard), then: Box::new(bind) }),
                }),
            };
        }
    }
    for c in expr.children_mut() {
        tries |= guard_ok_or(c, taken);
    }
    tries
}

/// The pieces of `Try` on what `option_method` builds for `ok_or`:
/// `let $opt = recv; let $arg = e; let $res = match $opt { Some(s) => Ok(s),
/// None => Err($arg) }; $res`.
#[allow(clippy::type_complexity)]
fn ok_or_try(value: &Expr) -> Option<(Name, Option<purecrate_ir::Ty>, Expr, Name, Option<purecrate_ir::Ty>, Expr)> {
    let Expr::Try { expr, on: Some(TryOn::Result) } = value else { return None };
    let Expr::Let { name: opt, ty: opt_ty, value: recv, then, .. } = &**expr else { return None };
    let Expr::Let { name: arg, ty: arg_ty, value: e, then, .. } = &**then else { return None };
    let Expr::Let { name: res, value: m, then, .. } = &**then else { return None };
    if **then != Expr::Var(res.clone()) {
        return None;
    }
    let Expr::Match { scrutinee, arms } = &**m else { return None };
    let [some, none] = arms.as_slice() else { return None };
    let ok = match (&some.pattern, &some.body) {
        (purecrate_ir::Pattern::OptionSome(p), Expr::Call { callee: Callee::ResultOk, args }) => {
            matches!((&**p, args.as_slice()), (purecrate_ir::Pattern::Var(s), [Expr::Var(v)]) if s == v)
        }
        _ => false,
    };
    let err = matches!((&none.pattern, &none.body), (purecrate_ir::Pattern::OptionNone, Expr::Call { callee: Callee::ResultErr, args })
        if args.as_slice() == [Expr::Var(arg.clone())]);
    (**scrutinee == Expr::Var(opt.clone()) && ok && err && some.guard.is_none() && none.guard.is_none())
        .then(|| (opt.clone(), opt_ty.clone(), (**recv).clone(), arg.clone(), arg_ty.clone(), (**e).clone()))
}

/// Whether `expr` holds a `?` outside a closure, which has its own.
fn has_try(expr: &Expr) -> bool {
    expr.search(|e| match e {
        Expr::Try { .. } => Some(true),
        Expr::Closure { .. } => Some(false),
        _ => None,
    })
}

fn lift_closures(expr: &mut Expr) {
    if let Expr::Closure { body, .. } = expr {
        let inner = std::mem::replace(&mut **body, Expr::Unreachable);
        **body = lift_body(inner);
        return;
    }
    expr.children_mut().into_iter().for_each(lift_closures);
}

/// Every name `body` binds: `let`, loop variables, patterns, and closure
/// parameters.
fn bound_names(body: &Expr) -> HashSet<String> {
    let mut taken = HashSet::new();
    body.walk(|e| taken.extend(e.own_bindings().into_iter().map(|n| n.as_str().to_string())));
    taken
}

/// A binding placed in front of the statement: `let name = inner?` when the
/// kind is set, else `let name = inner`, with the type the binding had.
type Hoisted = Vec<(Name, Expr, Option<Option<TryOn>>, Option<purecrate_ir::Ty>)>;

#[derive(Default)]
struct Lifter {
    /// Every name the body binds, and those this pass has made.
    taken: HashSet<String>,
}

impl Lifter {
    fn for_body(body: &Expr) -> Self {
        Lifter { taken: bound_names(body) }
    }

    /// `$<f><What>` for `f(..)` (`$removeSkuResult`), `$<what>` otherwise,
    /// kept apart from every name taken. The printer drops the `$`.
    fn fresh(&mut self, inner: &Expr, what: &str) -> Name {
        let base = match inner {
            Expr::Call { callee: Callee::Fn(f) | Callee::Method { name: f, .. }, .. } => {
                format!("${}{what}", purecrate_ir::to_camel(f.as_str()))
            }
            _ => format!("${}", what.to_ascii_lowercase()),
        };
        let name = (1..)
            .map(|i| if i == 1 { base.clone() } else { format!("{base}{i}") })
            .find(|n| !self.taken.contains(n))
            .expect("a free name");
        self.taken.insert(name.clone());
        Name::new(name)
    }

    /// A made name like `name` (`$opt` → `$opt_2`) no binding of the body has.
    fn fresh_like(&mut self, name: &Name) -> Name {
        let base = name.as_str().trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
        let fresh = (1..).map(|k| format!("{base}_{k}")).find(|n| !self.taken.contains(n)).expect("a free name");
        self.taken.insert(fresh.clone());
        Name::new(fresh)
    }

    fn stmt(&mut self, expr: Expr) -> Expr {
        match expr {
            // `let x = { let $t = a; b }` is `let $t = a; let x = b`: a made
            // name meets no other, and `a` runs first either way. A `?` in
            // `a` then leaves from the statement, not from a block in `x`'s
            // value (`let s = o.unwrap_or(..)` over `f(g()?)`).
            Expr::Let { name, mutable, ty, value, then } if matches!(&*value, Expr::Let { name: t, value: tv, .. } if t.as_str().starts_with('$') && has_try(tv)) =>
            {
                let Expr::Let { name: t, mutable: tm, ty: tt, value: tv, then: tthen } = *value else { unreachable!() };
                self.stmt(Expr::Let {
                    name: t,
                    mutable: tm,
                    ty: tt,
                    value: tv,
                    then: Box::new(Expr::Let { name, mutable, ty, value: tthen, then }),
                })
            }
            Expr::Let { name, mutable, ty, value, then } => {
                let (value, hoisted) = match *value {
                    Expr::Try { expr, on } => {
                        let (inner, hoisted) = self.extract(*expr);
                        (Expr::Try { expr: Box::new(inner), on }, hoisted)
                    }
                    v if v.needs_statements() => (self.stmt(v), Vec::new()),
                    v => self.extract(v),
                };
                let then = self.stmt(*then);
                wrap(hoisted, Expr::Let { name, mutable, ty, value: Box::new(value), then: Box::new(then) })
            }
            Expr::If { cond, then, else_ } => {
                let (cond, hoisted) = self.extract(*cond);
                let (then, else_) = (self.stmt(*then), self.stmt(*else_));
                wrap(hoisted, Expr::If { cond: Box::new(cond), then: Box::new(then), else_: Box::new(else_) })
            }
            Expr::Match { scrutinee, arms } => {
                let (scrutinee, hoisted) = self.extract(*scrutinee);
                let arms = arms
                    .into_iter()
                    .map(|a| Arm { guard: None, pattern: a.pattern, body: self.stmt(a.body) })
                    .collect();
                wrap(hoisted, Expr::Match { scrutinee: Box::new(scrutinee), arms })
            }
            Expr::Return(value) => {
                let (value, hoisted) = self.extract(*value);
                wrap(hoisted, Expr::Return(Box::new(value)))
            }
            Expr::Assign { name, value } if value.needs_statements() => {
                Expr::Assign { name, value: Box::new(self.stmt(*value)) }
            }
            Expr::Assign { name, value } => {
                let (value, hoisted) = self.extract(*value);
                wrap(hoisted, Expr::Assign { name, value: Box::new(value) })
            }
            Expr::Seq { first, then } => {
                Expr::Seq { first: Box::new(self.stmt(*first)), then: Box::new(self.stmt(*then)) }
            }
            // The condition runs before every pass, so a `?` in it cannot be
            // hoisted in front of the loop: the loop becomes `while true`
            // whose body computes the condition first and leaves when false.
            Expr::While { cond, body } => {
                let (cond, hoisted) = self.extract(*cond);
                let body = self.stmt(*body);
                if hoisted.is_empty() {
                    return Expr::While { cond: Box::new(cond), body: Box::new(body) };
                }
                let exit = Expr::If {
                    cond: Box::new(Expr::Unary { op: UnOp::Not, expr: Box::new(cond) }),
                    then: Box::new(Expr::Break),
                    else_: Box::new(Expr::Lit(Lit::Unit)),
                };
                Expr::While {
                    cond: Box::new(Expr::Lit(Lit::Bool(true))),
                    body: Box::new(wrap(hoisted, Expr::Seq { first: Box::new(exit), then: Box::new(body) })),
                }
            }
            // The bounds run once before the loop, in order: a start that
            // may panic is bound before a `?` in the end. A `?` in the body
            // leaves from inside it.
            Expr::For { var, ty, start, end, body } => {
                let mut hoisted = Vec::new();
                let [start, end]: [Expr; 2] =
                    self.in_order(vec![*start, *end], &mut hoisted).try_into().expect("two bounds in, two out");
                let body = self.stmt(*body);
                wrap(hoisted, Expr::For { var, ty, start: Box::new(start), end: Box::new(end), body: Box::new(body) })
            }
            Expr::ForEach { var, over, source: string, body } => {
                let mut hoisted = Vec::new();
                let string = self.extract_into(*string, &mut hoisted);
                let body = self.stmt(*body);
                wrap(hoisted, Expr::ForEach { var, over, source: Box::new(string), body: Box::new(body) })
            }
            other => {
                let (value, hoisted) = self.extract(other);
                wrap(hoisted, value)
            }
        }
    }

    /// Rewrites in place, reusing the allocation.
    fn boxed(&mut self, mut e: Box<Expr>, out: &mut Hoisted) -> Box<Expr> {
        *e = self.operand(std::mem::replace(&mut *e, Expr::Unreachable), out);
        e
    }

    /// An operand, an argument, or an element, each always evaluated. A
    /// `let` / `match` / `if` that leaves the function (what typing makes of
    /// `r.map_err(f)?` or `o.unwrap_or(s.parse()?)`), or that holds
    /// statements, runs before the statement, bound to a fresh name: printed
    /// in place it would be an inline function, whose `return` would leave
    /// only itself, and whose lines would sit inside a one-line expression.
    fn operand(&mut self, expr: Expr, out: &mut Hoisted) -> Expr {
        let block = matches!(expr, Expr::Let { .. } | Expr::Match { .. } | Expr::If { .. });
        if block && (expr.exits() || holds_statements(&expr)) {
            let name = self.fresh(&expr, "Value");
            let value = self.stmt(expr);
            out.push((name.clone(), value, None, None));
            return Expr::Var(name);
        }
        self.extract_into(expr, out)
    }

    /// Replaces each `?` in a strict position with a fresh variable.
    fn extract(&mut self, expr: Expr) -> (Expr, Hoisted) {
        let mut hoisted = Vec::new();
        let expr = self.extract_into(expr, &mut hoisted);
        (expr, hoisted)
    }

    fn extract_into(&mut self, expr: Expr, out: &mut Hoisted) -> Expr {
        match expr {
            Expr::Try { expr, on } => {
                // What `?` takes is bound before the statement already.
                let inner = Box::new(self.extract_into(*expr, out));
                let name = self.fresh(&inner, if on == Some(TryOn::Option) { "Opt" } else { "Result" });
                // The hoisted value is the `Result` itself, tested in place
                // (see `wrap`), and read here as its payload. A `None` is
                // `null`, so an `Option` is its own payload; so is the guard
                // `ok_or` becomes, which binds the payload.
                let guarded = ok_or_try(&Expr::Try { expr: inner.clone(), on }).is_some();
                let read = if on == Some(TryOn::Option) || guarded {
                    Expr::Var(name.clone())
                } else {
                    Expr::Field { base: Box::new(Expr::Var(name.clone())), name: Name::new("value") }
                };
                out.push((name, *inner, Some(on), None));
                read
            }
            Expr::Call { callee, args } => Expr::Call { callee, args: self.in_order(args, out) },
            Expr::Construct { ty, variant, fields, base } => {
                let (shape, names, values): (Shape, Vec<Name>, Vec<Expr>) = match fields {
                    Fields::Unit => (Shape::Unit, Vec::new(), Vec::new()),
                    Fields::Positional(xs) => (Shape::Positional, Vec::new(), xs),
                    Fields::Named(xs) => {
                        let (names, values) = xs.into_iter().unzip();
                        (Shape::Named, names, values)
                    }
                };
                let mut all = values;
                let has_base = base.is_some();
                all.extend(base.map(|b| *b));
                let mut all = self.in_order(all, out);
                // TS evaluates `...base` before the fields; Rust evaluates it
                // after. When the base can panic, the fields go first.
                let base = if has_base {
                    let b = all.pop().expect("pushed above");
                    if !pure(&b) {
                        all = all.into_iter().map(|x| self.spill(x, out)).collect();
                    }
                    Some(Box::new(self.spill(b, out)))
                } else {
                    None
                };
                let fields = match shape {
                    Shape::Unit => Fields::Unit,
                    Shape::Positional => Fields::Positional(all),
                    Shape::Named => Fields::Named(names.into_iter().zip(all).collect()),
                };
                Expr::Construct { ty, variant, fields, base }
            }
            Expr::Tuple(xs) => Expr::Tuple(self.in_order(xs, out)),
            Expr::Array(xs) => Expr::Array(self.in_order(xs, out)),
            // A made binding whose rest leaves (`ok_or(e)` with a `?` in `e`
            // lowers to `let $opt = o; let $arg = e; ..`): Rust runs `o`, then
            // `e`, before the statement does anything else, so both go in
            // front of it, renamed apart from every other binding there. Left
            // in place, in a scrutinee or an operand, the `?` would sit in an
            // inline function whose `return` leaves only itself.
            Expr::Let { name, mutable: false, ty, value, then } if name.as_str().starts_with('$') && then.exits() => {
                let value = self.operand(*value, out);
                let fresh = self.fresh_like(&name);
                let then = renamed(*then, &name, &fresh);
                out.push((fresh, value, None, ty));
                self.extract_into(then, out)
            }
            // A block's first value runs first, so its `?` may go before the
            // statement: what `ok_or(e)?` takes (`x.checked_add(g()?)`).
            Expr::Let { name, mutable, ty, value, then } => {
                Expr::Let { name, mutable, ty, value: self.boxed(value, out), then }
            }
            Expr::Field { base, name } => Expr::Field { base: self.boxed(base, out), name },
            Expr::Index { base, index } => {
                let mut xs = self.in_order(vec![*base, *index], out);
                let index = xs.pop().expect("two");
                let base = xs.pop().expect("two");
                Expr::Index { base: Box::new(base), index: Box::new(index) }
            }
            Expr::Unary { op, expr } => Expr::Unary { op, expr: self.boxed(expr, out) },
            Expr::Ignored { wrapper, expr } => Expr::Ignored { wrapper, expr: self.boxed(expr, out) },
            Expr::Binary { op: op @ (BinOp::And | BinOp::Or), left, right } => {
                Expr::Binary { op, left: self.boxed(left, out), right }
            }
            Expr::Binary { op, left, right } => {
                let mut xs = self.in_order(vec![*left, *right], out);
                let right = xs.pop().expect("two");
                let left = xs.pop().expect("two");
                Expr::Binary { op, left: Box::new(left), right: Box::new(right) }
            }
            other => other,
        }
    }

    /// Siblings evaluated left to right. Hoisting a `?` out of one moves it
    /// in front of the whole statement, so every earlier sibling that could
    /// panic is bound first, keeping Rust's order.
    fn in_order(&mut self, xs: Vec<Expr>, out: &mut Hoisted) -> Vec<Expr> {
        let mut done: Vec<Expr> = Vec::with_capacity(xs.len());
        for x in xs {
            let mut own = Vec::new();
            let x = self.operand(x, &mut own);
            if !own.is_empty() {
                done = std::mem::take(&mut done).into_iter().map(|d| self.spill(d, out)).collect();
            }
            out.extend(own);
            done.push(x);
        }
        done
    }

    /// Binds `expr` to a fresh variable unless evaluating it cannot panic.
    /// Literal shapes keep their shape, so TS still sees the literal type.
    fn spill(&mut self, expr: Expr, out: &mut Hoisted) -> Expr {
        match expr {
            e if pure(&e) => e,
            Expr::Construct { ty, variant, fields, base } => {
                let fields = match fields {
                    Fields::Unit => Fields::Unit,
                    Fields::Positional(xs) => Fields::Positional(xs.into_iter().map(|x| self.spill(x, out)).collect()),
                    Fields::Named(xs) => Fields::Named(xs.into_iter().map(|(n, x)| (n, self.spill(x, out))).collect()),
                };
                let base = base.map(|b| Box::new(self.spill(*b, out)));
                Expr::Construct { ty, variant, fields, base }
            }
            Expr::Tuple(xs) => Expr::Tuple(xs.into_iter().map(|x| self.spill(x, out)).collect()),
            Expr::Array(xs) => Expr::Array(xs.into_iter().map(|x| self.spill(x, out)).collect()),
            Expr::Ignored { wrapper, expr } => Expr::Ignored { wrapper, expr: Box::new(self.spill(*expr, out)) },
            other => {
                let name = self.fresh(&other, "Value");
                out.push((name.clone(), other, None, None));
                Expr::Var(name)
            }
        }
    }
}

enum Shape {
    Unit,
    Positional,
    Named,
}

/// Evaluating it has no effect and cannot panic.
fn pure(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Var(_) | Expr::Closure { .. } => true,
        Expr::Field { base, .. } => pure(base),
        Expr::Call { callee: Callee::OptionNone, .. } => true,
        _ => false,
    }
}

/// `let name = inner; name?; body` for a hoisted `?`: the test is on the
/// binding itself, and the use reads its payload (`extract_into`). The
/// guard `ok_or` becomes keeps `let name = inner?`, which binds the payload.
/// Whether `expr` is a block whose value comes after statements: a `let`
/// of a value that is not a name or a literal, a `;`, an assignment, or a
/// loop, itself or in a side of its `if` / `match`.
fn holds_statements(expr: &Expr) -> bool {
    match expr {
        Expr::Let { value, then, .. } => !value.is_inlinable() || holds_statements(then),
        Expr::Seq { .. } | Expr::Assign { .. } | Expr::For { .. } | Expr::ForEach { .. } | Expr::While { .. } => true,
        Expr::If { then, else_, .. } => holds_statements(then) || holds_statements(else_),
        Expr::Match { arms, .. } => arms.iter().any(|a| holds_statements(&a.body)),
        _ => false,
    }
}

fn wrap(hoisted: Hoisted, body: Expr) -> Expr {
    hoisted.into_iter().rev().fold(body, |then, (name, inner, on, ty)| {
        let (value, then) = match on {
            Some(on) if ok_or_try(&Expr::Try { expr: Box::new(inner.clone()), on }).is_some() => {
                (Expr::Try { expr: Box::new(inner), on }, then)
            }
            Some(on) => (
                inner,
                Expr::Seq {
                    first: Box::new(Expr::Try { expr: Box::new(Expr::Var(name.clone())), on }),
                    then: Box::new(then),
                },
            ),
            None => (inner, then),
        };
        Expr::Let { name, mutable: false, ty, value: Box::new(value), then: Box::new(then) }
    })
}

/// `expr` reading `to` wherever it read `from`, up to a binding of `from`
/// that hides it.
fn renamed(expr: Expr, from: &Name, to: &Name) -> Expr {
    fn walk(e: &mut Expr, from: &Name, to: &Name) {
        match e {
            Expr::Var(n) if n == from => *n = to.clone(),
            Expr::Let { name, value, then, .. } => {
                walk(value, from, to);
                if name != from {
                    walk(then, from, to);
                }
            }
            other => other.children_mut().into_iter().for_each(|c| walk(c, from, to)),
        }
    }
    let mut expr = expr;
    walk(&mut expr, from, to);
    expr
}
