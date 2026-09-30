//! `std::cmp::Ordering` (design/01 §6.1): `a.cmp(&b)` on the types whose
//! order JS can follow, and `Ordering`'s own methods, each as the `if` or
//! `match` std's code runs. The parser adds the enum when the crate names it
//! (`Enum::std_ordering`), so a `match` on it is an ordinary enum `match`.

use purecrate_ir::ORDERING;

use super::*;

/// `Ordering`'s methods on the allow-list.
pub(super) const ORDERING_METHODS: [&str; 9] = ["is_eq", "is_ne", "is_lt", "is_gt", "is_le", "is_ge", "reverse", "then", "then_with"];

impl<'d, 'a> Typer<'d, 'a> {
    /// `ty` is std's `Ordering`, not a crate enum of that name.
    pub(super) fn is_ordering(&self, ty: &Ty) -> bool {
        matches!(self.norm(ty), Ty::Named(n) if self.defs.enums.get(n.as_str()).is_some_and(|e| e.std))
    }

    /// `a.cmp(&b)` on an integer, `char`, `bool`, `String`/`&str`, or `Uuid`:
    /// the receiver, then the argument, each evaluated once, then `Less` if
    /// `a < b`, `Equal` if `a == b`, else `Greater`. The comparisons are the
    /// operators' own, so a `char` compares its code point and a string
    /// calls `Str.cmp`; `bool` orders `false` first. `None` for a crate type,
    /// whose `cmp` would be its own method.
    pub(super) fn cmp_method(&mut self, recv: Expr, rt: &Ty, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let failed = || Some((Expr::Lit(Lit::Unit), None));
        let norm = self.norm(rt);
        let bool_ = match &norm {
            Ty::Named(_) => return None,
            Ty::Prim(p) if p.int().is_some() => false,
            Ty::Prim(Prim::Char | Prim::String | Prim::Str | Prim::Uuid) => false,
            Ty::Prim(Prim::Bool) => true,
            Ty::Prim(Prim::F32 | Prim::F64) => {
                self.error(Reason::MethodCall, format!(
                    "`.cmp()` on `{}` is not in v0: floats are not `Ord` (`NaN` has no place in the order), and `partial_cmp` is not in v0",
                    show(&norm)
                ));
                return failed();
            }
            other => {
                self.error(Reason::MethodCall, format!(
                    "`.cmp()` on `{}` is not in v0: it orders integers, `char`, `bool`, `String`/`&str`, and `Uuid`; compare the parts and chain them with `then` or `then_with`",
                    show(other)
                ));
                return failed();
            }
        };
        let [arg] = args else {
            self.error(Reason::ConstructShape, format!("`cmp` takes 1 argument after the receiver, got {}", args.len()));
            return failed();
        };
        if !self.ordering_known("cmp") {
            return failed();
        }
        // `s.cmp(t)` takes a `&str` or a `String` for a string receiver.
        let hint = match &norm {
            Ty::Prim(Prim::String) => Ty::Prim(Prim::Str),
            _ => rt.clone(),
        };
        let (arg, at) = self.expr(arg, Some(&hint));
        // A binding read after the argument ran could have been assigned by it.
        let reread = matches!(arg, Expr::Var(_) | Expr::Lit(_));
        let (lhs_let, lhs) = self.bind_once("lhs", recv, Some(rt.clone()), reread);
        let (rhs_let, rhs) = self.bind_once("rhs", arg, at.or(Some(hint)), true);
        let bin = |op, l: &Expr, r: &Expr| rebuild(op, l.clone(), r.clone());
        let if_ = |cond: Expr, then: Expr, else_: Expr| Expr::If { cond: Box::new(cond), then: Box::new(then), else_: Box::new(else_) };
        let body = if bool_ {
            if_(bin(BinOp::Eq, &lhs, &rhs), variant("Equal"), if_(lhs.clone(), variant("Greater"), variant("Less")))
        } else {
            if_(bin(BinOp::Lt, &lhs, &rhs), variant("Less"), if_(bin(BinOp::Eq, &lhs, &rhs), variant("Equal"), variant("Greater")))
        };
        Some(self.within(vec![lhs_let, rhs_let], &body, want))
    }

    /// `is_eq`, `is_ne`, `is_lt`, `is_gt`, `is_le`, `is_ge`, `reverse`,
    /// `then(o)`, and `then_with(f)` on an `Ordering`, as a `match` on it.
    /// `then` evaluates its argument before choosing, as Rust evaluates a
    /// call's arguments; `then_with` calls `f` only on `Equal`. `None` when
    /// `name` is none of these.
    pub(super) fn ordering_method(&mut self, recv: Expr, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let failed = || Some((Expr::Lit(Lit::Unit), None));
        if !ORDERING_METHODS.contains(&name) {
            return None;
        }
        let takes = usize::from(matches!(name, "then" | "then_with"));
        if args.len() != takes {
            self.error(Reason::ConstructShape, format!(
                "`Ordering::{name}` takes {takes} argument(s) after the receiver, got {}",
                args.len()
            ));
            return failed();
        }
        // `then`'s argument runs after the receiver, which is read later.
        let reread = name != "then" || matches!(args[0].unpositioned(), Expr::Var(_) | Expr::Lit(_));
        let (recv_let, recv) = self.bind_once("ord", recv, Some(Ty::named(ORDERING)), reread);
        let mut lets = vec![recv_let];
        let same = |v: &'static str| (v, variant(v));
        let bool_ = |b: bool| Expr::Lit(Lit::Bool(b));
        let arms: [(&str, Expr); 3] = match name {
            "is_eq" => [("Less", bool_(false)), ("Equal", bool_(true)), ("Greater", bool_(false))],
            "is_ne" => [("Less", bool_(true)), ("Equal", bool_(false)), ("Greater", bool_(true))],
            "is_lt" => [("Less", bool_(true)), ("Equal", bool_(false)), ("Greater", bool_(false))],
            "is_gt" => [("Less", bool_(false)), ("Equal", bool_(false)), ("Greater", bool_(true))],
            "is_le" => [("Less", bool_(true)), ("Equal", bool_(true)), ("Greater", bool_(false))],
            "is_ge" => [("Less", bool_(false)), ("Equal", bool_(true)), ("Greater", bool_(true))],
            "reverse" => [("Less", variant("Greater")), same("Equal"), ("Greater", variant("Less"))],
            "then" => {
                let (arg, at) = self.expr(&args[0], Some(&Ty::named(ORDERING)));
                let (arg_let, arg) = self.bind_once("arg", arg, at, true);
                lets.push(arg_let);
                [same("Less"), ("Equal", arg), same("Greater")]
            }
            _ => {
                let call = match args[0].unpositioned() {
                    Expr::Closure { params, body, .. } if params.is_empty() => {
                        if leaves(body) {
                            self.error(Reason::Closure, "a closure passed to `Ordering::then_with` may not use `?` or `return` in v0; write the `match`".into());
                            return failed();
                        }
                        (**body).clone()
                    }
                    Expr::Var(f) => Expr::Call { callee: Callee::Fn(f.clone()), args: Vec::new() },
                    _ => {
                        self.error(Reason::Closure, "`Ordering::then_with` takes a closure `|| ..` or a function name in v0".into());
                        return failed();
                    }
                };
                [same("Less"), ("Equal", call), same("Greater")]
            }
        };
        let body = Expr::Match {
            scrutinee: Box::new(recv),
            arms: arms
                .into_iter()
                .map(|(v, body)| Arm::new(Pattern::Variant { ty: Name::new(ORDERING), variant: Name::new(v), bind: VariantBind::Unit }, body))
                .collect(),
        };
        Some(self.within(lets, &body, want))
    }

    /// Whether std's `Ordering` is the crate's `Ordering`; reported where
    /// `what` needs it when not.
    fn ordering_known(&mut self, what: &str) -> bool {
        match self.defs.enums.get(ORDERING) {
            Some(e) if e.std => true,
            Some(_) => {
                self.error(Reason::NameCollision, format!(
                    "`{what}` gives `std::cmp::Ordering`, which the crate's own `Ordering` hides; rename the crate's enum"
                ));
                false
            }
            None => {
                self.error(Reason::UndefinedType, format!("`{what}` gives `std::cmp::Ordering`; write `use std::cmp::Ordering;`"));
                false
            }
        }
    }

    /// `e` as an expression to read later: itself when it is a literal, or a
    /// binding and `reread` allows, else a fresh name bound to it (in the
    /// order bound).
    fn bind_once(&mut self, what: &str, e: Expr, ty: Option<Ty>, reread: bool) -> (Option<(Name, Option<Ty>, Expr)>, Expr) {
        if matches!(e, Expr::Lit(_)) || (reread && matches!(e, Expr::Var(_))) {
            return (None, e);
        }
        let name = self.fresh_name(what);
        (Some((name.clone(), ty, e)), Expr::Var(name))
    }

    /// Types `body` with the typed `lets` in scope, then puts them around it.
    fn within(&mut self, lets: Vec<Option<(Name, Option<Ty>, Expr)>>, body: &Expr, want: Option<&Ty>) -> Typed {
        let lets: Vec<(Name, Option<Ty>, Expr)> = lets.into_iter().flatten().collect();
        for (name, ty, _) in &lets {
            self.scopes.push((name.as_str().to_string(), ty.clone()));
        }
        let (mut typed, t) = self.expr(body, want);
        for _ in &lets {
            self.scopes.pop();
        }
        for (name, ty, value) in lets.into_iter().rev() {
            typed = Expr::Let { name, mutable: false, ty, value: Box::new(value), then: Box::new(typed) };
        }
        (typed, t)
    }
}

/// `Ordering::<v>`.
fn variant(v: &str) -> Expr {
    Expr::Construct { ty: Name::new(ORDERING), variant: Some(Name::new(v)), fields: Fields::Unit, base: None }
}
