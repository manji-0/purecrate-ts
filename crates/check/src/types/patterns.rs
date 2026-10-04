//! `match` and its patterns: arm types, literal and range patterns, and the
//! bindings a pattern introduces.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    pub(super) fn match_(&mut self, scrutinee: &Expr, arms: &[Arm], want: Option<&Ty>) -> Typed {
        let (scrutinee, st) = self.expr(scrutinee, None);
        // The arms' bindings would have no type, and the printed temporary
        // would be `Result<T, unknown>` in TS (`Ok::<T, E>(x)` is read as
        // `Ok(x)`). Not reported when a binding of this item already was.
        if st.is_none() && !self.out.iter().any(|d| d.item == self.item) {
            self.error(
                Reason::NeedsAnnotation,
                "the type of the matched value is not known here; bind it first with `let x: T = ..`".to_string(),
            );
        }
        let st = st.map(|t| self.norm(&t));
        let mut result: Option<Ty> = None;
        let arms = arms
            .iter()
            .map(|arm| {
                let depth = self.scopes.len();
                self.bind(&arm.pattern, st.as_ref());
                let guard = arm.guard.as_ref().map(|g| self.expr(g, Some(&Ty::bool())).0);
                let hint = want.cloned().or_else(|| result.clone());
                let (body, t) = self.expr(&arm.body, hint.as_ref());
                self.scopes.truncate(depth);
                result = join(result.take(), t);
                Arm { guard, pattern: self.lit_pattern(&arm.pattern, st.as_ref()), body }
            })
            .collect::<Vec<Arm>>();
        // Typed; now one `match` per element, so TS checks each is exhaustive.
        if let Some(Ty::Tuple(ts)) = &st {
            if arms.iter().any(|a| a.pattern.is_tuple_case()) {
                let tys = ts.iter().map(|t| self.norm(t)).collect();
                return (crate::tuple::lower(self.defs, scrutinee, tys, arms, &mut self.fresh), result);
            }
        }
        // Guards, and arms that test inside a case, are tested in the same
        // decision tree, a single value as a tuple of one.
        if let Some(t) = st.as_ref().filter(|_| arms.iter().any(|a| a.guard.is_some() || a.pattern.nests())) {
            let arms = arms
                .into_iter()
                .map(|a| Arm {
                    pattern: match a.pattern {
                        Pattern::Wildcard => Pattern::Wildcard,
                        p => Pattern::Tuple(vec![p]),
                    },
                    ..a
                })
                .collect();
            let scrutinee = Expr::Tuple(vec![scrutinee]);
            return (crate::tuple::lower(self.defs, scrutinee, vec![t.clone()], arms, &mut self.fresh), result);
        }
        (Expr::Match { scrutinee: Box::new(scrutinee), arms }, result)
    }

    /// The normalized element types of a tuple scrutinee of `width`, or
    /// `None`s after reporting a scrutinee that is not one.
    pub(super) fn tuple_elems(&mut self, width: usize, scrutinee: Option<&Ty>) -> Vec<Option<Ty>> {
        match scrutinee {
            Some(Ty::Tuple(ts)) if ts.len() == width => ts.iter().map(|t| Some(self.norm(t))).collect(),
            Some(t) if *t != Ty::Never => {
                self.error(
                    Reason::TypeMismatch,
                    format!("a tuple pattern of {width} elements does not match a value of type `{}`", show(t)),
                );
                vec![None; width]
            }
            _ => vec![None; width],
        }
    }

    /// An integer arm with every literal given the scrutinee's type, so the
    /// printer knows `1` from `1n`; a string arm checked against a `&str`.
    /// A tuple arm is checked element by element. Other patterns are
    /// returned as they are.
    pub(super) fn lit_pattern(&mut self, pattern: &Pattern, scrutinee: Option<&Ty>) -> Pattern {
        match pattern {
            // `bind` has reported a scrutinee that is not a tuple.
            Pattern::Tuple(ps) => {
                let tys = match scrutinee {
                    Some(Ty::Tuple(ts)) if ts.len() == ps.len() => ts.iter().map(|t| Some(self.norm(t))).collect(),
                    _ => vec![None; ps.len()],
                };
                return Pattern::Tuple(ps.iter().zip(&tys).map(|(p, t)| self.lit_pattern(p, t.as_ref())).collect());
            }
            Pattern::Or(alts) if pattern.is_tuple_case() || pattern.nests() => {
                return Pattern::Or(alts.iter().map(|a| self.lit_pattern(a, scrutinee)).collect());
            }
            // A literal inside a case takes the field's or the payload's type.
            Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => {
                let inner = match (pattern, scrutinee) {
                    (Pattern::OptionSome(_), Some(Ty::Option(t))) => Some(self.norm(t)),
                    (Pattern::ResultOk(_), Some(Ty::Result { ok, .. })) => Some(self.norm(ok)),
                    (Pattern::ResultErr(_), Some(Ty::Result { err, .. })) => Some(self.norm(err)),
                    _ => None,
                };
                let p = Box::new(self.lit_pattern(p, inner.as_ref()));
                return match pattern {
                    Pattern::OptionSome(_) => Pattern::OptionSome(p),
                    Pattern::ResultOk(_) => Pattern::ResultOk(p),
                    _ => Pattern::ResultErr(p),
                };
            }
            Pattern::Variant { ty, variant, bind } => {
                let fields = self
                    .defs
                    .enums
                    .get(ty.as_str())
                    .and_then(|e| e.variants.iter().find(|v| v.name == *variant))
                    .map(|v| v.fields.clone());
                let bind = match (bind, fields) {
                    (VariantBind::Tuple(ps), Some(VariantFields::Tuple(tys))) => VariantBind::Tuple(
                        ps.iter()
                            .zip(&tys)
                            .map(|(p, t)| {
                                let t = self.norm(t);
                                self.lit_pattern(p, Some(&t))
                            })
                            .collect(),
                    ),
                    (VariantBind::Struct(ps), Some(VariantFields::Struct(fs))) => VariantBind::Struct(
                        ps.iter()
                            .map(|(f, p)| {
                                let t = fs.iter().find(|d| d.name == *f).map(|d| self.norm(&d.ty));
                                (f.clone(), self.lit_pattern(p, t.as_ref()))
                            })
                            .collect(),
                    ),
                    (other, _) => other.clone(),
                };
                return Pattern::Variant { ty: ty.clone(), variant: variant.clone(), bind };
            }
            _ => {}
        }
        if pattern.is_bool_case() {
            match scrutinee {
                Some(Ty::Prim(Prim::Bool) | Ty::Never) | None => {}
                Some(t) => self
                    .error(Reason::TypeMismatch, format!("`bool` patterns do not match a value of type `{}`", show(t))),
            }
            return pattern.clone();
        }
        if pattern.is_char_case() {
            match scrutinee {
                Some(Ty::Prim(Prim::Char) | Ty::Never) | None => {}
                Some(t) => self
                    .error(Reason::TypeMismatch, format!("`char` patterns do not match a value of type `{}`", show(t))),
            }
            return pattern.clone();
        }
        if pattern.is_str_case() {
            match scrutinee {
                Some(Ty::Prim(Prim::Str) | Ty::Never) | None => {}
                Some(Ty::Prim(Prim::String)) => self.error(
                    Reason::TypeMismatch,
                    "string patterns match a `&str`, found `String`; match on `s.as_str()`".to_string(),
                ),
                Some(t) => self
                    .error(Reason::TypeMismatch, format!("string patterns do not match a value of type `{}`", show(t))),
            }
            return pattern.clone();
        }
        if !pattern.is_int_case() {
            return pattern.clone();
        }
        let ty = match scrutinee.and_then(|t| self.num(t)) {
            Some(Num::Int(t)) => Some(t),
            _ => {
                if let Some(t) = scrutinee {
                    self.error(
                        Reason::TypeMismatch,
                        format!("integer patterns do not match a value of type `{}`", show(t)),
                    );
                }
                None
            }
        };
        self.fill_int(pattern, ty)
    }

    pub(super) fn fill_int(&mut self, pattern: &Pattern, ty: Option<IntTy>) -> Pattern {
        match pattern {
            Pattern::Lit(lit) => Pattern::Lit(self.int_lit(lit, ty)),
            Pattern::Range { lo, hi, inclusive } => {
                Pattern::Range { lo: self.int_lit(lo, ty), hi: self.int_lit(hi, ty), inclusive: *inclusive }
            }
            Pattern::Or(alts) => Pattern::Or(alts.iter().map(|a| self.fill_int(a, ty)).collect()),
            other => other.clone(),
        }
    }

    pub(super) fn int_lit(&mut self, lit: &Lit, ty: Option<IntTy>) -> Lit {
        match lit {
            Lit::Int { value, ty: written, byte, hex } => {
                if let (Some(w), Some(t)) = (written, ty) {
                    if *w != t {
                        self.error(
                            Reason::TypeMismatch,
                            format!("pattern `{value}{}` does not match a value of type `{}`", w.as_str(), t.as_str()),
                        );
                    }
                }
                Lit::Int { value: *value, ty: ty.or(*written), byte: *byte, hex: *hex }
            }
            other => other.clone(),
        }
    }

    /// `scrutinee` is the normalized type of the matched value, when known.
    pub(super) fn bind(&mut self, pattern: &Pattern, scrutinee: Option<&Ty>) {
        let (name, ty) = match (pattern, scrutinee) {
            (Pattern::Var(n), t) => (Some(n.as_str().to_string()), t.cloned()),
            (Pattern::OptionSome(p), Some(Ty::Option(t))) => return self.bind(p, Some(t.as_ref())),
            (Pattern::ResultOk(p), Some(Ty::Result { ok, .. })) => return self.bind(p, Some(ok.as_ref())),
            (Pattern::ResultErr(p), Some(Ty::Result { err, .. })) => return self.bind(p, Some(err.as_ref())),
            (Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p), other) => {
                if let Some(t) = other {
                    self.error(
                        Reason::TypeMismatch,
                        format!("pattern `{}` does not match a value of type `{}`", describe(pattern), show(t)),
                    );
                }
                return self.bind(p, None);
            }
            (Pattern::OptionNone, Some(t)) if !matches!(t, Ty::Option(_)) => {
                self.error(
                    Reason::TypeMismatch,
                    format!("pattern `None` does not match a value of type `{}`", show(t)),
                );
                (None, None)
            }
            (Pattern::OptionNone | Pattern::Wildcard | Pattern::Lit(_) | Pattern::Range { .. }, _) => (None, None),
            (Pattern::Variant { .. }, _) => return self.bind_variant(pattern, scrutinee),
            // The alternatives bind nothing (checked by the parser).
            (Pattern::Or(alts), _) => {
                alts.iter().for_each(|alt| self.bind(alt, scrutinee));
                return;
            }
            (Pattern::Tuple(ps), _) => {
                for (p, t) in ps.iter().zip(self.tuple_elems(ps.len(), scrutinee)) {
                    self.bind(p, t.as_ref());
                }
                return;
            }
        };
        if let Some(n) = name {
            self.scopes.push((n, ty));
        }
    }

    pub(super) fn bind_variant(&mut self, pattern: &Pattern, scrutinee: Option<&Ty>) {
        let Pattern::Variant { ty, variant, bind } = pattern else {
            return;
        };
        if let Some(t) = scrutinee.filter(|t| **t != Ty::Named(ty.clone())) {
            self.error(
                Reason::TypeMismatch,
                format!("pattern `{}::{}` does not match a value of type `{}`", ty.as_str(), variant.as_str(), show(t)),
            );
        }
        let fields = self
            .defs
            .enums
            .get(ty.as_str())
            .and_then(|e| e.variants.iter().find(|v| v.name == *variant))
            .map(|v| &v.fields);
        match (bind, fields) {
            (VariantBind::Tuple(ps), Some(VariantFields::Tuple(tys))) => {
                for (p, t) in ps.iter().zip(tys) {
                    self.bind(p, Some(t));
                }
            }
            (VariantBind::Struct(ps), Some(VariantFields::Struct(fs))) => {
                for (field, p) in ps {
                    let ty = fs.iter().find(|f| f.name == *field).map(|f| f.ty.clone());
                    self.bind(p, ty.as_ref());
                }
            }
            _ => {}
        }
    }
}

pub(super) fn describe(pattern: &Pattern) -> &'static str {
    match pattern {
        Pattern::OptionSome(_) => "Some(..)",
        Pattern::OptionNone => "None",
        Pattern::ResultOk(_) => "Ok(..)",
        Pattern::ResultErr(_) => "Err(..)",
        _ => "..",
    }
}
