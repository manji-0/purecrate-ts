//! `std::cmp::Ordering` (design/01 §6.1): `a.cmp(&b)` on the types whose
//! order JS can follow, and `Ordering`'s own methods, each as the `if` or
//! `match` std's code runs. The parser adds the enum when the crate names it
//! (`Enum::std_ordering`), so a `match` on it is an ordinary enum `match`.

use purecrate_ir::ORDERING;

use super::*;

/// `Ordering`'s methods on the allow-list.
pub(super) const ORDERING_METHODS: [&str; 9] =
    ["is_eq", "is_ne", "is_lt", "is_gt", "is_le", "is_ge", "reverse", "then", "then_with"];

impl<'d, 'a> Typer<'d, 'a> {
    /// `ty` is std's `Ordering`, not a crate enum of that name.
    pub(super) fn is_ordering(&self, ty: &Ty) -> bool {
        matches!(self.norm(ty), Ty::Named(n) if self.defs.enums.get(n.as_str()).is_some_and(|e| e.std))
    }

    /// `a.cmp(&b)` on an integer, `char`, `bool`, `String`/`&str`, or `Uuid`:
    /// `Callee::OrdCmp`, the receiver then the argument, each evaluated once.
    /// A `char`, string, or `Uuid` compares by code point (`Str.cmp`); JS `<`
    /// orders the others as Rust does, `bool` with `false` first. An
    /// `Option`, a tuple, or a `Vec` of other values compares by its
    /// comparator (`Callee::OrdCmpBy`). `None` for a crate type with a
    /// `cmp`, which is its own method (derived, or `impl Ord`'s).
    pub(super) fn cmp_method(&mut self, recv: Expr, rt: &Ty, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let failed = || Some((Expr::Lit(Lit::Unit), None));
        let norm = self.norm(rt);
        match &norm {
            Ty::Named(n) | Ty::App { name: n, .. }
                if !self.is_ordering(&norm) && !self.defs.methods.contains_key(&(n.as_str(), "cmp")) =>
            {
                let why = crate::order::why_unordered(self.defs, n.as_str());
                self.error(Reason::MethodCall, format!("`.cmp()` on `{}` is not in v0: {why}", show(&norm)));
                return failed();
            }
            Ty::Option(_) | Ty::Tuple(_) => return Some(self.cmp_by(recv, rt, &norm, args, want)),
            Ty::Vec(t) if !matches!(self.norm(t), Ty::Prim(_)) => {
                return Some(self.cmp_by(recv, rt, &norm, args, want))
            }
            _ => {}
        }
        // A `Vec` of what `cmp` orders, compared element by element.
        let list = match &norm {
            Ty::Vec(t) => Some(self.norm(t)),
            _ => None,
        };
        let norm = match &list {
            Some(t @ Ty::Prim(_)) => t.clone(),
            Some(other) => {
                self.error(Reason::MethodCall, format!(
                    "`.cmp()` on a `Vec` orders `Vec`s of integers, `char`, `bool`, `String`/`&str`, or `Uuid` in v0, not of `{}`",
                    show(other)
                ));
                return failed();
            }
            None => norm,
        };
        match &norm {
            Ty::Named(_) => return None,
            Ty::Prim(p) if p.int().is_some() => {}
            Ty::Prim(Prim::Char | Prim::String | Prim::Str | Prim::Uuid | Prim::Bool) => {}
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
            self.error(
                Reason::ConstructShape,
                format!("`cmp` takes 1 argument after the receiver, got {}", args.len()),
            );
            return failed();
        };
        if !self.ordering_known("cmp") {
            return failed();
        }
        // `s.cmp(t)` takes a `&str` or a `String` for a string receiver.
        let hint = match &norm {
            Ty::Prim(Prim::String) if list.is_none() => Ty::Prim(Prim::Str),
            _ => rt.clone(),
        };
        let (arg, _) = self.expr(arg, Some(&hint));
        let text = matches!(norm, Ty::Prim(Prim::Char | Prim::String | Prim::Str | Prim::Uuid));
        let callee = if list.is_some() { Callee::OrdCmpList { text } } else { Callee::OrdCmp { text } };
        let call = Expr::Call { callee, args: vec![recv, arg] };
        Some((call, self.expect(want, Some(Ty::named(ORDERING)))))
    }

    /// `a.cmp(&b)` on an `Option`, a tuple, or a `Vec` of what `OrdCmp` does
    /// not order: `a`, then `b`, compared by `norm`'s comparator.
    fn cmp_by(&mut self, recv: Expr, rt: &Ty, norm: &Ty, args: &[Expr], want: Option<&Ty>) -> Typed {
        let failed = (Expr::Lit(Lit::Unit), None);
        let by = match self.comparator(norm) {
            Ok(by) => by,
            Err(why) => {
                self.error(
                    Reason::MethodCall,
                    format!("`.cmp()` on `{}` is not in v0: {}", show(norm), self.unordered(norm, why)),
                );
                return failed;
            }
        };
        let [arg] = args else {
            self.error(
                Reason::ConstructShape,
                format!("`cmp` takes 1 argument after the receiver, got {}", args.len()),
            );
            return failed;
        };
        if !self.ordering_known("cmp") {
            return failed;
        }
        let (arg, _) = self.expr(arg, Some(rt));
        (compare(by, recv, arg), self.expect(want, Some(Ty::named(ORDERING))))
    }

    /// How `t`'s values compare by `cmp` (`order::comparator`), or what in
    /// it has no order.
    pub(super) fn comparator(&self, t: &Ty) -> Result<Cmp, String> {
        crate::order::comparator(self.defs, t, &|n| crate::order::has_cmp(self.defs, n))
    }

    /// Why `t`, which `comparator` found `why` in, has no order: a crate
    /// type's own reason, else what in it has none.
    pub(super) fn unordered(&self, t: &Ty, why: String) -> String {
        match self.norm(t) {
            Ty::Named(n) | Ty::App { name: n, .. } => crate::order::why_unordered(self.defs, n.as_str()),
            _ => format!("it holds {why}"),
        }
    }

    /// What `name` (`sort`, `max`, `max_by_key`, ..) orders `t` by: `Ord.cmp`
    /// (`Ok(false)`) or `Ord.cmpStr` (`Ok(true)`) as before, else a
    /// comparator passed to the runtime (`Err`); `None` once reported.
    pub(super) fn ordered_by_any(&mut self, t: &Ty, name: &str) -> Option<Result<bool, Cmp>> {
        match self.comparator(t) {
            Ok(Cmp::Num) => Some(Ok(false)),
            Ok(Cmp::Text) => Some(Ok(true)),
            Ok(by) => Some(Err(by)),
            Err(_) if matches!(self.norm(t), Ty::Never) => None,
            Err(why) => {
                self.error(
                    Reason::Comparison,
                    format!(
                        "`{name}` orders by `cmp`, and `{}` has none: {}; use the `_by` form with a closure giving an `Ordering`",
                        show(t),
                        self.unordered(t, why)
                    ),
                );
                None
            }
        }
    }

    /// `a.max(b)`, `a.min(b)`, or `x.clamp(lo, hi)` on a crate type, an
    /// `Option`, a `Vec`, or a tuple, by its comparator (`Callee::OrdPick`).
    /// `None` where the receiver is none of these, or its type has its own
    /// method of that name.
    pub(super) fn pick(&mut self, recv: Expr, rt: &Ty, name: &str, args: &[Expr], want: Option<&Ty>) -> Option<Typed> {
        let pick = match name {
            "max" => Pick::Max,
            "min" => Pick::Min,
            "clamp" => Pick::Clamp,
            _ => return None,
        };
        let norm = self.norm(rt);
        match &norm {
            Ty::Named(n) | Ty::App { name: n, .. }
                if self.is_ordering(&norm) || self.defs.methods.contains_key(&(n.as_str(), name)) =>
            {
                return None
            }
            Ty::Named(_) | Ty::App { .. } | Ty::Option(_) | Ty::Vec(_) | Ty::Tuple(_) => {}
            _ => return None,
        }
        let failed = Some((Expr::Lit(Lit::Unit), None));
        let by = match self.comparator(&norm) {
            Ok(by) => by,
            Err(why) => {
                self.error(
                    Reason::MethodCall,
                    format!(
                        "`.{name}()` on `{}` orders by `cmp`, which it has not: {}",
                        show(&norm),
                        self.unordered(&norm, why)
                    ),
                );
                return failed;
            }
        };
        let takes = if pick == Pick::Clamp { 2 } else { 1 };
        if args.len() != takes {
            self.error(
                Reason::ConstructShape,
                format!("`{name}` takes {takes} argument(s) after the receiver, got {}", args.len()),
            );
            return failed;
        }
        let mut all = vec![recv];
        all.extend(args.iter().map(|a| self.expr(a, Some(rt)).0));
        let e = Expr::Call { callee: Callee::OrdPick { pick, by }, args: all };
        Some((e, self.expect(want, Some(rt.clone()))))
    }

    /// `is_eq`, `is_ne`, `is_lt`, `is_gt`, `is_le`, `is_ge`, `reverse`,
    /// `then(o)`, and `then_with(f)` on an `Ordering`, as a `match` on it.
    /// `then` evaluates its argument before choosing, as Rust evaluates a
    /// call's arguments; `then_with` calls `f` only on `Equal`. `None` when
    /// `name` is none of these.
    pub(super) fn ordering_method(
        &mut self,
        recv: Expr,
        name: &str,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
        let failed = || Some((Expr::Lit(Lit::Unit), None));
        if !ORDERING_METHODS.contains(&name) {
            return None;
        }
        let takes = usize::from(matches!(name, "then" | "then_with"));
        if args.len() != takes {
            self.error(
                Reason::ConstructShape,
                format!("`Ordering::{name}` takes {takes} argument(s) after the receiver, got {}", args.len()),
            );
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
                let (arg, _) = self.expr(&args[0], Some(&Ty::named(ORDERING)));
                let recv = match lets.pop().flatten() {
                    Some((_, _, value)) => value,
                    None => recv,
                };
                let call = Expr::Call { callee: Callee::OrdThen, args: vec![recv, arg] };
                return Some((call, self.expect(want, Some(Ty::named(ORDERING)))));
            }
            _ => {
                let Some(call) = self.thunk("`Ordering::then_with`", &args[0]) else { return failed() };
                [same("Less"), ("Equal", call), same("Greater")]
            }
        };
        let body = Expr::Match {
            scrutinee: Box::new(recv),
            arms: arms
                .into_iter()
                .map(|(v, body)| {
                    Arm::new(
                        Pattern::Variant { ty: Name::new(ORDERING), variant: Name::new(v), bind: VariantBind::Unit },
                        body,
                    )
                })
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
                self.error(
                    Reason::UndefinedType,
                    format!("`{what}` gives `std::cmp::Ordering`; write `use std::cmp::Ordering;`"),
                );
                false
            }
        }
    }

    /// `e` as an expression to read later: itself when it is a literal, or a
    /// binding and `reread` allows, else a fresh name bound to it (in the
    /// order bound).
    fn bind_once(&mut self, what: &str, e: Expr, ty: Option<Ty>, reread: bool) -> (Option<Bound>, Expr) {
        if matches!(e, Expr::Lit(_)) || (reread && matches!(e, Expr::Var(_))) {
            return (None, e);
        }
        let name = self.fresh_name(what);
        (Some((name.clone(), ty, e)), Expr::Var(name))
    }

    /// Types `body` with the typed `lets` in scope, then puts them around it.
    fn within(&mut self, lets: Vec<Option<Bound>>, body: &Expr, want: Option<&Ty>) -> Typed {
        let lets: Vec<Bound> = lets.into_iter().flatten().collect();
        let (typed, t) = self.scoped(&lets, body, want);
        (wrap_lets(lets, typed), t)
    }
}

/// `a.cmp(&b)` by `by`: a crate type's own `cmp`, and the runtime's
/// `Ord.cmp`, `Ord.cmpStr`, and `Ord.cmpList` as before for what they
/// order, else `Callee::OrdCmpBy`.
pub(super) fn compare(by: Cmp, a: Expr, b: Expr) -> Expr {
    let callee = match by {
        Cmp::Type(ty) => Callee::Method { ty, name: Name::new("cmp") },
        Cmp::Num => Callee::OrdCmp { text: false },
        Cmp::Text => Callee::OrdCmp { text: true },
        Cmp::Vec(by) if matches!(*by, Cmp::Num | Cmp::Text) => Callee::OrdCmpList { text: *by == Cmp::Text },
        by => Callee::OrdCmpBy(by),
    };
    Expr::Call { callee, args: vec![a, b] }
}

/// A comparator as an argument of a runtime function.
pub(super) fn comparator_value(by: Cmp) -> Expr {
    Expr::Call { callee: Callee::OrdBy(by), args: Vec::new() }
}

/// `Ordering::<v>`.
fn variant(v: &str) -> Expr {
    Expr::Construct { ty: Name::new(ORDERING), variant: Some(Name::new(v)), fields: Fields::Unit, base: None }
}
