//! `From` and `Default` on the crate's types (design/01 §7.25): `X::from(s)`,
//! `From::from(s)`, `s.into()`, `?` across error types, and
//! `Default::default()`, each rewritten to what it calls.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// The `From` member of the crate type `to` that takes `source`.
    fn conversion_of(&self, to: &Name, source: &Ty) -> Option<&'a Fn> {
        self.defs.conversions_into(to.as_str()).find(|(_, s)| self.same(s, source)).map(|(f, _)| f)
    }

    /// The sources `to` has a `From` member for, sorted by member name.
    fn sorted_conversions(&self, to: &Name) -> Vec<(&'a Fn, &'a Ty)> {
        let mut all: Vec<_> = self.defs.conversions_into(to.as_str()).collect();
        all.sort_by(|a, b| a.0.name.as_str().cmp(b.0.name.as_str()));
        all
    }

    /// The crate type a conversion goes into: a struct or an enum.
    fn conversion_target(&self, ty: &Ty) -> Option<Name> {
        match self.norm(ty) {
            Ty::Named(n) if self.defs.structs.contains_key(n.as_str()) || self.defs.enums.contains_key(n.as_str()) => {
                Some(n)
            }
            _ => None,
        }
    }

    /// `X::from(s)` (`to` is `X`), or `From::from(s)` and `s.into()` (`to`
    /// is what the context wants): `X.fromS(s)`. `arg` is typed here,
    /// against the one source when `X` has one, unless `typed` already has it.
    pub(super) fn convert(
        &mut self,
        what: &str,
        to: Option<&Ty>,
        arg: &Expr,
        typed: Option<Typed>,
        want: Option<&Ty>,
    ) -> Typed {
        let failed = |me: &mut Self, typed: Option<Typed>| match typed {
            Some((e, _)) => (e, None),
            None => (me.expr(arg, None).0, None),
        };
        let Some(target) = to.or(want) else {
            self.error(
                Reason::NeedsAnnotation,
                format!("the target of {what} is not known here; annotate the binding it goes to"),
            );
            return failed(self, typed);
        };
        let Some(x) = self.conversion_target(target) else {
            self.error(
                Reason::TypeMismatch,
                format!(
                    "{what} into `{}` is not in v0: it converts into a struct or enum of the crate with an `impl From`",
                    show(target)
                ),
            );
            return failed(self, typed);
        };
        let sources = self.sorted_conversions(&x);
        let (arg, at) = match typed {
            Some(t) => t,
            None => {
                let hint = match sources.as_slice() {
                    [(_, s)] => Some((*s).clone()),
                    _ => None,
                };
                self.expr(arg, hint.as_ref())
            }
        };
        let Some(at) = at else { return (arg, None) };
        let Some(f) = self.conversion_of(&x, &at) else {
            let known: Vec<String> = sources.iter().map(|(_, s)| format!("`{}`", show(s))).collect();
            let has = if known.is_empty() { "none".to_string() } else { known.join(", ") };
            self.error(
                Reason::TypeMismatch,
                format!("{what}: `{0}` has no `impl From<{1}> for {0}`; it converts from {has}", x.as_str(), show(&at)),
            );
            return (arg, None);
        };
        let e = Expr::Call { callee: Callee::Method { ty: x.clone(), name: f.name.clone() }, args: vec![arg] };
        (e, self.expect(want, Some(Ty::Named(x))))
    }

    /// `r?` where `r: Result<ok, err>` and the function returns
    /// `Result<_, X>` with `impl From<err> for X`: `match r { Ok(v) => v,
    /// Err(e) => return Err(X.fromErr(e)) }`, as `map_err(..)?` is, so the
    /// conversion runs only on `Err`, after `r`. `None` when there is no
    /// such `impl`.
    pub(super) fn converting_try(
        &mut self,
        recv: Expr,
        ok: &Ty,
        err: &Ty,
        ret_err: &Ty,
        want: Option<&Ty>,
    ) -> Option<Typed> {
        let x = self.conversion_target(ret_err)?;
        let f = self.conversion_of(&x, err)?;
        let e = self.fresh_name("error");
        let r = self.fresh_name("result");
        let value = self.fresh_name("value");
        let convert =
            Expr::Call { callee: Callee::Method { ty: x, name: f.name.clone() }, args: vec![Expr::Var(e.clone())] };
        let rest = Expr::Match {
            scrutinee: Box::new(Expr::Var(r.clone())),
            arms: vec![
                Arm {
                    guard: None,
                    pattern: Pattern::ResultOk(Box::new(Pattern::Var(value.clone()))),
                    body: Expr::Var(value),
                },
                Arm {
                    guard: None,
                    pattern: Pattern::ResultErr(Box::new(Pattern::Var(e))),
                    body: Expr::Return(Box::new(Expr::Call { callee: Callee::ResultErr, args: vec![convert] })),
                },
            ],
        };
        let lets = vec![(r, Some(Ty::result(ok.clone(), err.clone())), recv)];
        let (typed, t) = self.scoped(&lets, &rest, want.or(Some(ok)));
        Some((wrap_lets(lets, typed), t))
    }

    /// `Default::default()` where the context wants `want`: `0` of an
    /// integer's width, `0.0`, `false`, `'\0'`, `""`, an empty `Vec`, map, or
    /// set, `None`, a tuple of defaults, or a crate type's `X.default()`.
    /// `field` is the field of a `#[derive(Default)]` it stands for.
    pub(super) fn default_value(&mut self, field: Option<&Name>, want: Option<&Ty>) -> Typed {
        let failed = (Expr::Lit(Lit::Unit), None);
        let Some(t) = want else {
            self.error(
                Reason::NeedsAnnotation,
                "the type of `Default::default()` is not known here; annotate the binding it goes to".to_string(),
            );
            return failed;
        };
        let call = |callee| Expr::Call { callee, args: Vec::new() };
        let value = match self.norm(t) {
            Ty::Prim(p) if p.int().is_some() => {
                Some(Expr::Lit(Lit::Int { value: 0, ty: p.int(), byte: false, hex: false }))
            }
            Ty::Prim(p) if p.float().is_some() => Some(Expr::Lit(Lit::Float { digits: "0.0".into(), ty: p.float() })),
            Ty::Prim(Prim::Bool) => Some(Expr::Lit(Lit::Bool(false))),
            Ty::Prim(Prim::Char) => Some(Expr::Lit(Lit::Char('\0'))),
            Ty::Prim(Prim::String) => Some(call(Callee::StringNew)),
            Ty::Prim(Prim::Str) => Some(Expr::Lit(Lit::Str(String::new()))),
            Ty::Prim(Prim::Unit) => Some(Expr::Lit(Lit::Unit)),
            Ty::Vec(_) => Some(Expr::Array(Vec::new())),
            Ty::Map { .. } => Some(call(Callee::Tree(TreeOp::NewMap))),
            Ty::Set(_) => Some(call(Callee::Tree(TreeOp::NewSet))),
            Ty::Option(_) => Some(call(Callee::OptionNone)),
            Ty::Tuple(ts) => Some(Expr::Tuple(ts.iter().map(|_| call(Callee::Default { field: None })).collect())),
            Ty::Named(n) if self.defs.methods.contains_key(&(n.as_str(), "default")) => {
                Some(call(Callee::Method { ty: n, name: Name::new("default") }))
            }
            // A `T: Default`: its dictionary's `default()`.
            Ty::Param(p) if self.dicts.iter().any(|(of, tr, _)| *of == p && self.defs.extends(tr.as_str(), "Default")) => {
                return self.bound_call(&p, &Name::new("default"), &[], want);
            }
            _ => None,
        };
        let Some(value) = value else {
            let what = match field {
                Some(f) => format!("`#[derive(Default)]`: field `{}` is a `{}`, which", f.as_str(), show(t)),
                None => format!("`Default::default()` of `{}`: the type", show(t)),
            };
            self.error(
                Reason::TypeMismatch,
                format!(
                    "{what} has no default in v0; the defaults are those of the integers, floats, `bool`, `char`, `String`, `Vec`, `BTreeMap`, `BTreeSet`, `Option`, tuples of them, and the crate's types with `#[derive(Default)]` or `impl Default`, and a type parameter with a `Default` bound (write the value, or `impl Default` by hand)"
                ),
            );
            return failed;
        };
        self.expr(&value, want)
    }
}
