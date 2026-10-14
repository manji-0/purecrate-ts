//! Type parameters (design/01 §7.20): a generic item's types at one
//! instantiation, its type arguments inferred from what the call or the
//! value gives, first-order.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// Binds each of `params` that `decl` names to the part of `got` in its
    /// place. The first binding of a parameter stands.
    pub(super) fn unify(&self, decl: &Ty, got: &Ty, params: &[Name], binds: &mut Vec<(Name, Ty)>) {
        let got = self.norm(got);
        if got == Ty::Never || super::has_hole(&got) {
            return;
        }
        match (decl, &got) {
            (Ty::Param(p), _) if params.contains(p) => {
                if !binds.iter().any(|(b, _)| b == p) {
                    binds.push((p.clone(), got.clone()));
                }
            }
            (Ty::Option(d), Ty::Option(g)) | (Ty::Vec(d), Ty::Vec(g)) | (Ty::Set(d), Ty::Set(g)) => {
                self.unify(d, g, params, binds)
            }
            (Ty::Result { ok: a, err: b }, Ty::Result { ok: c, err: d })
            | (Ty::Map { key: a, value: b }, Ty::Map { key: c, value: d }) => {
                self.unify(a, c, params, binds);
                self.unify(b, d, params, binds);
            }
            (Ty::Tuple(ds), Ty::Tuple(gs)) if ds.len() == gs.len() => {
                ds.iter().zip(gs).for_each(|(d, g)| self.unify(d, g, params, binds))
            }
            (Ty::App { name: n, args: ds }, Ty::App { name: m, args: gs }) if n == m && ds.len() == gs.len() => {
                ds.iter().zip(gs).for_each(|(d, g)| self.unify(d, g, params, binds))
            }
            (Ty::Ignored { inner, .. }, g) => self.unify(inner, g, params, binds),
            (d, Ty::Ignored { inner, .. }) => self.unify(d, inner, params, binds),
            _ => {}
        }
    }

    /// Whether `ty` still names one of `params` that `binds` leaves unbound
    /// (one bound to itself, the caller's own `T` in a recursive call, is
    /// bound).
    pub(super) fn open(ty: &Ty, params: &[Name], binds: &[(Name, Ty)]) -> bool {
        ty.any(&|t| matches!(t, Ty::Param(p) if params.contains(p) && !binds.iter().any(|(b, _)| b == p)))
    }

    /// The arguments of a call to a generic function, typed against its
    /// parameters `decl` once what is known of `params` is put in, and the
    /// result: `ret` with every parameter bound, from the arguments or from
    /// `want`. `binds` comes in with what the receiver fixed (`impl<T>`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn generic_args(
        &mut self,
        what: &str,
        params: &[Name],
        mut binds: Vec<(Name, Ty)>,
        decl: &[Ty],
        args: &[Expr],
        ret: &Ty,
        want: Option<&Ty>,
    ) -> (Vec<Expr>, Option<Ty>) {
        if let Some(w) = want {
            self.unify(ret, w, params, &mut binds);
        }
        let mut typed = Vec::new();
        for (i, a) in args.iter().enumerate() {
            typed.push(self.typed_against(decl.get(i), a, params, &mut binds));
        }
        if let Some(p) = params.iter().find(|p| Self::open(ret, std::slice::from_ref(p), &binds)) {
            self.error(
                Reason::NeedsAnnotation,
                format!("the type `{}` of {what} is not known here; annotate the binding it goes to", p.as_str()),
            );
            return (typed, None);
        }
        if !self.instance_ok(&binds, what) {
            return (typed, None);
        }
        (typed, Some(ret.subst(&binds)))
    }

    /// `x` typed against its declared type `decl` once what is known of
    /// `params` is put in; where `decl` still names an unbound parameter,
    /// what `x` turns out to be binds it.
    pub(super) fn typed_against(
        &mut self,
        decl: Option<&Ty>,
        x: &Expr,
        params: &[Name],
        binds: &mut Vec<(Name, Ty)>,
    ) -> Expr {
        let hint = decl.filter(|d| !Self::open(d, params, binds)).map(|d| d.subst(binds));
        let (e, t) = self.expr(x, hint.as_ref());
        if let (None, Some(t), Some(d)) = (&hint, &t, decl) {
            self.unify(d, t, params, binds);
        }
        e
    }

    /// Whether the types bound to a generic item's parameters make no type
    /// the subset refuses: a value `Eq.deep` cannot compare, a function,
    /// and (inside the item) an `Option<Option<_>>` or a map of `Option`s.
    pub(super) fn instance_ok(&mut self, binds: &[(Name, Ty)], what: &str) -> bool {
        for (p, t) in binds {
            let t = self.norm(t);
            let bad = match &t {
                Ty::Prim(Prim::ParseIntError | Prim::UuidError) | Ty::Fn { .. } => {
                    Some("a value that is not compared as Rust compares it")
                }
                Ty::Option(_) | Ty::Prim(Prim::Unit) | Ty::Never => {
                    Some("a type that `None` or `()` would print as `null` / `undefined` beside")
                }
                _ => None,
            };
            if let Some(why) = bad {
                self.error(
                    Reason::Generics,
                    format!(
                        "{what} takes `{}` as `{}`, {why}: a generic item's `Option<{0}>` or map value would be refused (design/01 §7.20)",
                        show(&t),
                        p.as_str()
                    ),
                );
                return false;
            }
        }
        true
    }

    /// A struct's or enum's type parameters and their bindings from `ty`,
    /// a `Named` (none) or an `App`.
    pub(super) fn bindings_of(&self, ty: &Ty) -> Vec<(Name, Ty)> {
        let Some((name, args)) = ty.applied() else { return Vec::new() };
        let params = self.defs.type_params(name.as_str()).unwrap_or_default();
        params.iter().cloned().zip(args.iter().cloned()).collect()
    }

    /// The type a struct or enum named `name` is with its parameters bound
    /// by `binds`: `Named` when it has none.
    pub(super) fn applied_ty(&self, name: &Name, params: &[Name], binds: &[(Name, Ty)]) -> Ty {
        if params.is_empty() {
            return Ty::Named(name.clone());
        }
        Ty::App { name: name.clone(), args: params.iter().map(|p| Ty::Param(p.clone()).subst(binds)).collect() }
    }
}
