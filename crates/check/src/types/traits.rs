//! Traits (design/01 §7.25): a method or const of a type parameter's bound
//! read through its dictionary, a trait's member called by the type of its
//! arguments, and the dictionaries a call to a bounded function passes.

use super::*;

/// A dictionary in scope: the type parameter, the trait, and the parameter
/// that holds it.
pub(super) type Dict = (Name, Name, Name);

impl<'d, 'a> Typer<'d, 'a> {
    /// The dictionaries `f` takes, each over one of its type parameters.
    pub(super) fn dicts_of(&self, f: &Fn) -> Vec<Dict> {
        f.params
            .iter()
            .filter_map(|p| match self.defs.dict_of(&p.ty) {
                Some((tr, Ty::Param(of))) => Some((of.clone(), tr.clone(), p.name.clone())),
                _ => None,
            })
            .collect()
    }

    /// The first dictionary in scope for the type parameter `p` whose trait
    /// (or one of its supertraits) `has`.
    fn dict_where(&self, p: &Name, has: impl std::ops::Fn(&str) -> bool) -> Option<Dict> {
        self.dicts.iter().find(|(of, tr, _)| of == p && has(tr.as_str())).cloned()
    }

    /// A trait method's parameter types and result with `Self` as `of`.
    fn method_sig(m: &TraitMethod, of: &Ty) -> (Vec<Ty>, Ty) {
        let binds = [(Name::new(purecrate_ir::SELF), of.clone())];
        (m.params.iter().map(|p| p.ty.subst(&binds)).collect(), m.ret.subst(&binds))
    }

    /// `dict.m(args)`: `args` typed against the method's parameters, `Self`
    /// the parameter the dictionary is over. `first` is the receiver, typed
    /// already.
    fn dict_call(
        &mut self,
        (of, tr, var): Dict,
        name: &Name,
        first: Option<Expr>,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Typed {
        let m = self.defs.trait_method(tr.as_str(), name.as_str()).expect("the dictionary has the method");
        let (params, ret) = Self::method_sig(m, &Ty::Param(of.clone()));
        let written = args.len() + usize::from(first.is_some());
        if params.len() != written {
            self.error(
                Reason::ConstructShape,
                format!("`{}::{}` takes {} argument(s), got {written}", tr.as_str(), name.as_str(), params.len()),
            );
        }
        let skip = usize::from(first.is_some());
        let mut typed = vec![Expr::Var(var)];
        typed.extend(first);
        for (a, p) in args.iter().zip(params.iter().skip(skip).map(Some).chain(std::iter::repeat(None))) {
            typed.push(self.expr(a, p).0);
        }
        let e = Expr::Call { callee: Callee::Dict { name: name.clone() }, args: typed };
        (e, self.expect(want, Some(ret)))
    }

    /// `x.m(args)` on a value of the type parameter `p`: through the first
    /// of its bounds that has `m`. `None` when none has.
    pub(super) fn bound_method(
        &mut self,
        p: &Name,
        name: &Name,
        recv: Expr,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
        let defs = self.defs;
        let dict = self.dict_where(p, |tr| defs.trait_method(tr, name.as_str()).is_some_and(|m| m.has_receiver()))?;
        Some(self.dict_call(dict, name, Some(recv), args, want))
    }

    /// `T::m(args)` (`Self::m(args)` in a default body): the method of one
    /// of `T`'s bounds, its arguments all written.
    pub(super) fn bound_call(&mut self, p: &Name, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        let defs = self.defs;
        match self.dict_where(p, |tr| defs.trait_method(tr, name.as_str()).is_some()) {
            Some(dict) => self.dict_call(dict, name, None, args, want),
            None => {
                self.error(
                    Reason::UndefinedFn,
                    format!(
                        "`{}::{}`: no bound of `{}` here has a method `{}`",
                        p.as_str(),
                        name.as_str(),
                        p.as_str(),
                        name.as_str()
                    ),
                );
                (Expr::Lit(Lit::Unit), None)
            }
        }
    }

    /// `Tr::m(args)`: the method of the type the receiver (or, for a
    /// function without one, the value's destination) has, as
    /// `Item::m(args)` or `T::m(args)` would be.
    pub(super) fn trait_call(&mut self, tr: &Name, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        let Some(m) = self.defs.trait_method(tr.as_str(), name.as_str()) else {
            self.error(Reason::UndefinedFn, format!("trait `{}` has no method `{}`", tr.as_str(), name.as_str()));
            return (Expr::Lit(Lit::Unit), None);
        };
        let of = if m.has_receiver() {
            let before = self.out.len();
            let t = args.first().and_then(|a| self.expr(a, None).1);
            // Typed again below, where its errors are reported.
            self.out.truncate(before);
            t
        } else {
            m.ret
                .any(&|t| matches!(t, Ty::Param(s) if s.as_str() == purecrate_ir::SELF))
                .then(|| want.cloned())
                .flatten()
        };
        match of.map(|t| self.norm(&t)) {
            Some(Ty::Param(p)) => self.bound_call(&p, name, args, want),
            Some(t) if t.applied().is_some() => {
                let (owner, _) = t.applied().expect("checked");
                let callee = Callee::Method { ty: owner.clone(), name: name.clone() };
                self.call(&callee, args, want)
            }
            Some(Ty::Never) => (Expr::Lit(Lit::Unit), Some(Ty::Never)),
            Some(t) => {
                self.error(Reason::TraitImpl, format!("`{}` has no `impl {}` in the crate", show(&t), tr.as_str()));
                (Expr::Lit(Lit::Unit), None)
            }
            None => {
                self.error(
                    Reason::NeedsAnnotation,
                    format!(
                        "the type `{}::{}` is called on is not known here; call it on the type (`Item::{}(..)`) or annotate the binding",
                        tr.as_str(),
                        name.as_str(),
                        name.as_str()
                    ),
                );
                (Expr::Lit(Lit::Unit), None)
            }
        }
    }

    /// `dict.m(args)` typed again (a pass that rebuilds a typed tree).
    pub(super) fn retyped_dict_call(&mut self, name: &Name, args: &[Expr], want: Option<&Ty>) -> Typed {
        let dict = args.first().and_then(|d| match d.unpositioned() {
            Expr::Var(v) => self.dicts.iter().find(|(_, _, var)| var == v).cloned(),
            _ => None,
        });
        match dict {
            Some(dict) => self.dict_call(dict, name, None, &args[1..], want),
            None => (Expr::Call { callee: Callee::Dict { name: name.clone() }, args: args.to_vec() }, None),
        }
    }

    /// `T::N`: the const `N` of one of `T`'s bounds, read from its
    /// dictionary (`pricedT.N`). `None` when `path` is no such read.
    pub(super) fn bound_const(&mut self, path: &str, want: Option<&Ty>) -> Option<Typed> {
        let (ty, name) = purecrate_ir::split_assoc_path(path)?;
        let defs = self.defs;
        if defs.consts.contains_key(path) || defs.is_type(ty) {
            return None;
        }
        let p = Name::new(ty);
        let found = self.dict_where(&p, |tr| defs.trait_const(tr, name).is_some());
        let Some((_, tr, var)) = found else {
            self.error(Reason::UndefinedName, format!("`{path}`: no bound of `{ty}` here has a const `{name}`"));
            return Some((Expr::Lit(Lit::Unit), None));
        };
        let c = defs.trait_const(tr.as_str(), name).expect("found above");
        let t = c.ty.subst(&[(Name::new(purecrate_ir::SELF), Ty::Param(p))]);
        let e = Expr::Field { base: Box::new(Expr::Var(var)), name: Name::new(name) };
        Some((e, self.expect(want, Some(t))))
    }

    /// The type of the const `name` read from a dictionary of type `dict`.
    pub(super) fn dict_field(&self, dict: &Ty, name: &Name) -> Option<Ty> {
        let (tr, of) = self.defs.dict_of(dict)?;
        let c = self.defs.trait_const(tr.as_str(), name.as_str())?;
        Some(c.ty.subst(&[(Name::new(purecrate_ir::SELF), of.clone())]))
    }

    /// The arguments of a call to `f`: `written`, the ones the source
    /// gives, with a dictionary in the place of each of `f`'s, for its type
    /// parameter as `binds` has it: the companion of a crate type, or the
    /// caller's own dictionary for one of its type parameters.
    pub(super) fn with_dicts(&mut self, f: &Fn, written: Vec<Expr>, binds: &[(Name, Ty)]) -> Vec<Expr> {
        let mut written = written.into_iter();
        let mut out = Vec::new();
        for p in &f.params {
            match self.defs.dict_of(&p.ty) {
                Some((tr, of)) => {
                    let of = of.subst(binds);
                    out.push(self.dict_arg(tr, &of));
                }
                None => out.extend(written.next()),
            }
        }
        out.extend(written);
        out
    }

    /// The dictionary of `tr` for the type `of`.
    fn dict_arg(&mut self, tr: &Name, of: &Ty) -> Expr {
        let defs = self.defs;
        match self.norm(of) {
            Ty::Param(p) => match self.dict_where(&p, |have| defs.extends(have, tr.as_str())) {
                Some((_, _, var)) => Expr::Var(var),
                None => {
                    self.error(
                        Reason::Generics,
                        format!("`{}` has no bound `{}` here to pass on", p.as_str(), tr.as_str()),
                    );
                    Expr::Lit(Lit::Unit)
                }
            },
            t => match t.applied() {
                Some((owner, _)) if self.implements(owner.as_str(), tr.as_str()) => Expr::Var(owner.clone()),
                _ => {
                    self.error(
                        Reason::TraitImpl,
                        format!("`{}` has no `impl {}` in the crate to pass as its dictionary", show(&t), tr.as_str()),
                    );
                    Expr::Lit(Lit::Unit)
                }
            },
        }
    }

    /// The crate type `owner` implements `tr` and each of its supertraits,
    /// so its companion holds every member of `tr`'s record.
    fn implements(&self, owner: &str, tr: &str) -> bool {
        self.defs.trait_and_supers(tr).iter().all(|t| t.impls.iter().any(|imp| imp.owner().as_str() == owner))
    }
}

/// A function owned by a trait: a default body, which a type that does not
/// write the method calls through its own member (`Item.doubled`), with no
/// dictionary.
pub(super) fn is_default_body(defs: &Defs, f: &Fn) -> bool {
    f.owner.as_ref().is_some_and(|o| defs.traits.contains_key(o.as_str()))
}
