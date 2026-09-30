//! Patterns and `match`: arm patterns and their rules, `if let`, `matches!`,
//! and match guards rewritten into an `if` chain.

use super::*;

/// A `match` with guards, as one without: arms are tried in order, and a
/// guarded arm is `if match $g { p => guard, _ => false } { match $g { p =>
/// body, _ => unreachable } } else { <the arms after it> }`, where `$g` holds
/// the scrutinee (each element of a tuple scrutinee) so it is evaluated
/// once. The guard runs only when its pattern matched, and the arms left
/// after the guarded ones must be exhaustive by themselves, as rustc
/// requires. `n if n > 3 =>` binds `n` to the scrutinee.
pub(super) fn lower_guarded(cx: &Cx, scrutinee: &SynExpr, arms: &[syn::Arm]) -> Result<Expr, ParseError> {
    // Bind the scrutinee (or each tuple element) once.
    let mut binds: Vec<(Name, Expr)> = Vec::new();
    let subject = match scrutinee {
        SynExpr::Tuple(t) if !t.elems.is_empty() => {
            let mut elems = Vec::new();
            for e in &t.elems {
                let name = cx.fresh("g");
                binds.push((name.clone(), lower_expr(cx, e)?));
                elems.push(Expr::Var(name));
            }
            Expr::Tuple(elems)
        }
        other => {
            let name = cx.fresh("g");
            binds.push((name.clone(), lower_expr(cx, other)?));
            Expr::Var(name)
        }
    };
    let tuple = matches!(subject, Expr::Tuple(_));
    let mut lowered = Vec::new();
    for arm in arms {
        let raw = lower_pat(cx, &arm.pat)?;
        let guard = match &arm.guard {
            Some((_, g)) => Some(lower_expr(cx, g)?),
            None => None,
        };
        let pattern = match (&raw, &guard) {
            (Pattern::Var(_), Some(_)) if !tuple => raw,
            _ => arm_pattern(raw).map_err(|e| e.or_at(arm.pat.span()))?,
        };
        lowered.push((pattern, guard, at(arm.body.span(), lower_expr(cx, &arm.body)?)));
    }
    let chain = guard_chain(&subject, &lowered);
    Ok(binds.into_iter().rev().fold(chain, |then, (name, value)| Expr::Let {
        name,
        mutable: false,
        ty: None,
        value: Box::new(value),
        then: Box::new(then),
    }))
}

/// `if hit_1 { take_1 } else if hit_2 { take_2 } .. else { unreachable }`,
/// arm by arm in order: `hit` is `match $g { p => guard, _ => false }` and
/// `take` is `match $g { p => body, _ => unreachable }`. Each `match` stands
/// alone rather than inside another on the same value, so TS narrows
/// nothing that a later `switch` would contradict. A pattern that always
/// matches (`_`, a binding, a tuple of those) needs no `match`.
pub(super) fn guard_chain(subject: &Expr, arms: &[(Pattern, Option<Expr>, Expr)]) -> Expr {
    let mut acc = Expr::Unreachable;
    for (pattern, guard, body) in arms.iter().rev() {
        let (hit, take) = match always_binds(subject, pattern) {
            Some(lets) => {
                let wrap = |e: &Expr| {
                    lets.iter().rev().fold(e.clone(), |then, (name, value)| Expr::Let {
                        name: name.clone(),
                        mutable: false,
                        ty: None,
                        value: Box::new(value.clone()),
                        then: Box::new(then),
                    })
                };
                (guard.as_ref().map(wrap), wrap(body))
            }
            None => {
                let two = |first: Expr, rest: Expr| Expr::Match {
                    scrutinee: Box::new(subject.clone()),
                    arms: vec![
                        Arm { pattern: pattern.clone(), body: first },
                        Arm { pattern: Pattern::Wildcard, body: rest },
                    ],
                };
                let hit = two(guard.clone().unwrap_or(Expr::Lit(Lit::Bool(true))), Expr::Lit(Lit::Bool(false)));
                (Some(hit), two(body.clone(), Expr::Unreachable))
            }
        };
        acc = match hit {
            None => take,
            Some(hit) => Expr::If {
                cond: Box::new(hit),
                then: Box::new(take),
                else_: Box::new(acc),
            },
        };
    }
    acc
}

/// For a pattern that matches every value: the names it binds, each with
/// what it binds to.
pub(super) fn always_binds(subject: &Expr, pattern: &Pattern) -> Option<Vec<(Name, Expr)>> {
    match (pattern, subject) {
        (Pattern::Wildcard, _) => Some(Vec::new()),
        (Pattern::Var(n), s) => Some(vec![(n.clone(), s.clone())]),
        (Pattern::Tuple(ps), Expr::Tuple(xs)) if ps.len() == xs.len() => {
            let mut out = Vec::new();
            for (p, x) in ps.iter().zip(xs) {
                out.extend(always_binds(x, p)?);
            }
            Some(out)
        }
        _ => None,
    }
}

/// `matches!(e, p)` is std's `match e { p => true, _ => false }`, and
/// `matches!(e, p if c)` is `match e { p => c, _ => false }`.
pub(super) fn lower_matches(cx: &Cx, mac: &syn::Macro) -> Result<Expr, ParseError> {
    let (scrutinee, pat, guard) = mac
        .parse_body_with(|input: syn::parse::ParseStream| {
            let e: SynExpr = input.parse()?;
            input.parse::<syn::Token![,]>()?;
            let p = Pat::parse_multi_with_leading_vert(input)?;
            let guard = if input.peek(syn::Token![if]) {
                input.parse::<syn::Token![if]>()?;
                Some(input.parse::<SynExpr>()?)
            } else {
                None
            };
            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
            Ok((e, p, guard))
        })
        .map_err(|e| ParseError::new(Reason::Macro, format!("`matches!` expects `matches!(value, pattern)`: {e}")))?;
    let pattern = arm_pattern(lower_pat(cx, &pat)?).map_err(|e| e.or_at(pat.span()))?;
    if pattern == Pattern::Wildcard {
        return Err(ParseError::new(
            Reason::ArmPattern,
            "`matches!(x, _)` does not test `x`; write `true`, or the condition of `_ if c`",
        ));
    }
    let hit = match guard {
        Some(g) => lower_expr(cx, &g)?,
        None => Expr::Lit(Lit::Bool(true)),
    };
    Ok(Expr::Match {
        scrutinee: Box::new(lower_expr(cx, &scrutinee)?),
        arms: vec![
            Arm { pattern, body: hit },
            Arm {
                pattern: Pattern::Wildcard,
                body: Expr::Lit(Lit::Bool(false)),
            },
        ],
    })
}

pub(super) fn lower_pat(cx: &Cx, pat: &Pat) -> Result<Pattern, ParseError> {
    lower_pat_node(cx, pat).map_err(|e| e.or_at(pat.span()))
}

pub(super) fn lower_pat_node(cx: &Cx, pat: &Pat) -> Result<Pattern, ParseError> {
    match pat {
        Pat::Wild(_) => Ok(Pattern::Wildcard),
        Pat::Ident(id) if id.ident == "None" && id.subpat.is_none() => Ok(Pattern::OptionNone),
        Pat::Ident(id) if id.by_ref.is_none() && id.mutability.is_none() && id.subpat.is_none() => {
            Ok(Pattern::Var(Name::new(id.ident.to_string())))
        }
        Pat::Lit(l) => Ok(Pattern::Lit(lower_lit(&l.lit)?)),
        Pat::Range(r) => {
            let bound = |e: &Option<Box<syn::Expr>>| match e.as_deref() {
                Some(SynExpr::Lit(l)) => lower_lit(&l.lit),
                _ => Err(ParseError::new(
                    Reason::UnsupportedPattern,
                    "range patterns need a literal at both ends in v0",
                )),
            };
            Ok(Pattern::Range {
                lo: bound(&r.start)?,
                hi: bound(&r.end)?,
                inclusive: matches!(r.limits, syn::RangeLimits::Closed(_)),
            })
        }
        Pat::Path(p) => path_variant_pat(cx, &p.path, VariantBind::Unit),
        Pat::TupleStruct(t) => {
            let bind = VariantBind::Tuple(
                t.elems
                    .iter()
                    .map(|e| lower_pat(cx, e))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            path_variant_pat(cx, &t.path, bind)
        }
        Pat::Struct(s) => {
            let bind = VariantBind::Struct(
                s.fields
                    .iter()
                    .map(|f| {
                        let name = match &f.member {
                            Member::Named(id) => Name::new(id.to_string()),
                            Member::Unnamed(_) => {
                                return Err(ParseError::new(Reason::PositionalFields, "unnamed fields in struct pattern"))
                            }
                        };
                        Ok((name, lower_pat(cx, &f.pat)?))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            path_variant_pat(cx, &s.path, bind)
        }
        Pat::Tuple(t) if t.elems.len() == 1 => lower_pat(cx, &t.elems[0]),
        Pat::Tuple(t) if t.elems.len() > 1 => Ok(Pattern::Tuple(
            t.elems
                .iter()
                .map(|p| lower_pat(cx, p))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Pat::Or(o) => Ok(Pattern::Or(
            o.cases
                .iter()
                .map(|p| lower_pat(cx, p))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        other => Err(ParseError::new(
            Reason::UnsupportedPattern,
            format!("unsupported pattern {}", snippet(other)),
        )),
    }
}

/// v0 prints `match` as `switch (x.kind)`: each arm names one variant and
/// binds its fields to plain names, names several variants of one enum with
/// `A | B` binding nothing, or is `_` (the variants no other arm names).
pub(super) fn arm_pattern(pattern: Pattern) -> Result<Pattern, ParseError> {
    if pattern.is_lit_case() {
        return Ok(pattern);
    }
    match &pattern {
        Pattern::Wildcard => return Ok(pattern),
        Pattern::Tuple(elems) => {
            tuple_elems(elems)?;
            return Ok(pattern);
        }
        Pattern::Or(alts) if pattern.is_tuple_case() => {
            for alt in alts {
                let Pattern::Tuple(elems) = alt else {
                    return Err(ParseError::new(Reason::ArmPattern, format!(
                        "each side of `|` must be a tuple pattern here, found {}",
                        describe_pat(alt)
                    )));
                };
                tuple_elems(elems)?;
            }
            if let Some(name) = pattern.bindings().first() {
                return Err(ParseError::new(Reason::ArmPattern, format!(
                    "`|` arms may not bind names in v0, found binding `{}`; write one arm per tuple",
                    name.as_str()
                )));
            }
            return Ok(pattern);
        }
        Pattern::Or(alts) => {
            for alt in alts {
                if !matches!(alt, Pattern::Variant { .. }) {
                    return Err(ParseError::new(Reason::ArmPattern, format!(
                        "each side of `|` must name an enum variant in v0, found {}",
                        describe_pat(alt)
                    )));
                }
                variant_fields(alt)?;
                if let Some(name) = alt.bindings().first() {
                    return Err(ParseError::new(Reason::ArmPattern, format!(
                        "`|` arms may not bind names in v0, found binding `{}`; write one arm per variant",
                        name.as_str()
                    )));
                }
            }
            return Ok(pattern);
        }
        _ => {}
    }
    variant_fields(&pattern)?;
    Ok(pattern)
}

/// The elements of a tuple arm: `_`, a binding, or what an arm of its own
/// may be. A tuple inside a tuple is not flattened.
pub(super) fn tuple_elems(elems: &[Pattern]) -> Result<(), ParseError> {
    for elem in elems {
        match elem {
            Pattern::Wildcard | Pattern::Var(_) => {}
            p if p.is_tuple_case() => {
                return Err(ParseError::new(Reason::NestedPattern, "tuple patterns may not nest in v0"));
            }
            p => {
                arm_pattern(p.clone())?;
            }
        }
    }
    Ok(())
}

/// Checks that `pattern` names a case and binds only names or `_` inside.
pub(super) fn variant_fields(pattern: &Pattern) -> Result<(), ParseError> {
    let inner: Vec<&Pattern> = match pattern {
        Pattern::Variant { bind, .. } => match bind {
            VariantBind::Unit => Vec::new(),
            VariantBind::Tuple(pats) => pats.iter().collect(),
            VariantBind::Struct(pairs) => pairs.iter().map(|(_, p)| p).collect(),
        },
        Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => vec![&**p],
        Pattern::OptionNone => Vec::new(),
        Pattern::Wildcard
        | Pattern::Var(_)
        | Pattern::Lit(_)
        | Pattern::Or(_)
        | Pattern::Range { .. }
        | Pattern::Tuple(_) => {
            return Err(ParseError::new(Reason::ArmPattern, format!(
                "match arms must name an enum variant, `Some`/`None`, `Ok`/`Err`, an integer, a `char`, or a range of either, a string literal, or be `_` in v0, found {}",
                describe_pat(pattern)
            )))
        }
    };
    if let Some(bad) = inner
        .into_iter()
        .find(|p| !matches!(p, Pattern::Var(_) | Pattern::Wildcard))
    {
        return Err(ParseError::new(Reason::NestedPattern, format!(
            "variant fields may only bind names or `_` in v0, found {}",
            describe_pat(bad)
        )));
    }
    Ok(())
}

pub(super) fn describe_pat(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Wildcard => "`_`".into(),
        Pattern::Var(n) => format!("binding `{}`", n.as_str()),
        Pattern::Lit(_) => "a literal".into(),
        Pattern::Variant { ty, variant, .. } => {
            format!("nested variant `{}::{}`", ty.as_str(), variant.as_str())
        }
        Pattern::OptionSome(_) => "nested `Some(..)`".into(),
        Pattern::OptionNone => "nested `None`".into(),
        Pattern::ResultOk(_) => "nested `Ok(..)`".into(),
        Pattern::ResultErr(_) => "nested `Err(..)`".into(),
        Pattern::Or(_) => "`|`".into(),
        Pattern::Range { .. } => "a range".into(),
        Pattern::Tuple(_) => "a tuple".into(),
    }
}

pub(super) fn path_variant_pat(cx: &Cx, path: &syn::Path, bind: VariantBind) -> Result<Pattern, ParseError> {
    let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    if let [one] = segs.as_slice() {
        if let Some(p) = prelude_pat(one, bind.clone())? {
            return Ok(p);
        }
    }
    match segs.as_slice() {
        [ty, var] if cx.is_enum(ty) => Ok(Pattern::Variant {
            ty: Name::new(ty.clone()),
            variant: Name::new(var.clone()),
            bind,
        }),
        [var] => {
            if let Some(ty) = cx.enum_for_variant(var) {
                Ok(Pattern::Variant {
                    ty: Name::new(ty),
                    variant: Name::new(var.clone()),
                    bind,
                })
            } else {
                Err(ParseError::new(Reason::ExternalPath, format!("unknown variant {var}")).detail(var.to_string()))
            }
        }
        _ => Err(path_error(&segs, format!(
            "unsupported pattern path {}",
            segs.join("::")
        ))),
    }
}

/// Bare `Some`/`None`/`Ok`/`Err` always mean the prelude's, as in Rust.
pub(super) fn prelude_pat(name: &str, bind: VariantBind) -> Result<Option<Pattern>, ParseError> {
    let one = |bind: VariantBind| match bind {
        VariantBind::Tuple(mut ps) if ps.len() == 1 => Ok(Box::new(ps.remove(0))),
        _ => Err(ParseError::new(Reason::UnsupportedPattern, format!("`{name}` takes exactly one field"))),
    };
    Ok(Some(match name {
        "Some" => Pattern::OptionSome(one(bind)?),
        "Ok" => Pattern::ResultOk(one(bind)?),
        "Err" => Pattern::ResultErr(one(bind)?),
        "None" if bind == VariantBind::Unit => Pattern::OptionNone,
        "None" => return Err(ParseError::new(Reason::UnsupportedPattern, "`None` has no fields")),
        _ => return Ok(None),
    }))
}

/// `if let P = e { a } else { b }` is `match e { P => a, <the other case> => b }`.
pub(super) fn lower_if_let(cx: &Cx, l: &syn::ExprLet, then: Expr, else_: Expr) -> Result<Expr, ParseError> {
    let pattern = arm_pattern(lower_pat(cx, &l.pat)?).map_err(|e| e.or_at(l.pat.span()))?;
    let other = match &pattern {
        Pattern::OptionSome(_) => Pattern::OptionNone,
        Pattern::OptionNone => Pattern::OptionSome(Box::new(Pattern::Wildcard)),
        Pattern::ResultOk(_) => Pattern::ResultErr(Box::new(Pattern::Wildcard)),
        Pattern::ResultErr(_) => Pattern::ResultOk(Box::new(Pattern::Wildcard)),
        _ => {
            return Err(ParseError::new(Reason::IfLetVariant, 
                "`if let` on an enum variant is not in v0; use `match` with every variant",
            )
            .or_at(l.pat.span()))
        }
    };
    Ok(Expr::Match {
        scrutinee: Box::new(lower_expr(cx, &l.expr)?),
        arms: vec![
            Arm {
                pattern,
                body: then,
            },
            Arm {
                pattern: other,
                body: else_,
            },
        ],
    })
}

/// A tuple pattern where Rust takes an irrefutable one: a `let`, a closure
/// parameter, or a `for` variable. It becomes a one-arm `match` on a tuple,
/// which binds each element once (`check::tuple`). A `mut` element binds a
/// fresh name in the pattern and is rebound as `let mut` in the body.
pub(super) struct Destructure {
    pattern: Pattern,
    rebind: Vec<(Name, Name)>,
}

impl Destructure {
    /// `(a, _, mut b, &c)`: two or more elements, each `_`, a name, `mut` a
    /// name, or `&` one of these; the tuple itself may be behind `&`
    /// (`for &(a, b) in xs.iter()`). A reference reads as its value. `None`
    /// when `pat` is not a tuple.
    pub(super) fn of(cx: &Cx, pat: &Pat, reason: Reason) -> Result<Option<Self>, ParseError> {
        let mut pat = pat;
        while let Pat::Reference(r) = pat {
            if r.mutability.is_some() || !matches!(strip_refs(&r.pat), Pat::Tuple(_)) {
                return Ok(None);
            }
            pat = &r.pat;
        }
        let Pat::Tuple(t) = pat else { return Ok(None) };
        let refuse = |p: &Pat| {
            Err(ParseError::new(
                reason,
                format!(
                    "a tuple pattern here takes `_`, a name, or `mut` a name for each element, found {}",
                    snippet(p)
                ),
            )
            .or_at(p.span()))
        };
        if t.elems.len() < 2 {
            return refuse(pat);
        }
        let mut elems = Vec::new();
        let mut rebind = Vec::new();
        for elem in &t.elems {
            let mut p = elem;
            while let Pat::Reference(r) = p {
                if r.mutability.is_some() {
                    return refuse(elem);
                }
                p = &r.pat;
            }
            elems.push(match p {
                Pat::Wild(_) => Pattern::Wildcard,
                Pat::Ident(id) if id.by_ref.is_none() && id.subpat.is_none() => {
                    let name = Name::new(id.ident.to_string());
                    if id.mutability.is_some() {
                        let temp = cx.fresh("p");
                        rebind.push((name, temp.clone()));
                        Pattern::Var(temp)
                    } else {
                        Pattern::Var(name)
                    }
                }
                _ => return refuse(elem),
            });
        }
        Ok(Some(Destructure {
            pattern: Pattern::Tuple(elems),
            rebind,
        }))
    }

    /// `match scrutinee { (..) => body }`.
    pub(super) fn bind(self, scrutinee: Expr, body: Expr) -> Expr {
        let body = self.rebind.into_iter().rev().fold(body, |then, (name, temp)| Expr::Let {
            name,
            mutable: true,
            ty: None,
            value: Box::new(Expr::Var(temp)),
            then: Box::new(then),
        });
        Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms: vec![Arm { pattern: self.pattern, body }],
        }
    }
}

fn strip_refs(mut pat: &Pat) -> &Pat {
    while let Pat::Reference(r) = pat {
        pat = &r.pat;
    }
    pat
}
