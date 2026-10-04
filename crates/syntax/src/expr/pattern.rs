//! Patterns and `match`: arm patterns and their rules, `if let`, `matches!`,
//! and match guards rewritten into an `if` chain.

use super::*;

/// A `match` with guards: each guard stays on its arm, with the arm's
/// bindings in scope, and `check::accept` lowers the arms into a decision
/// tree that tries a guard only where its pattern matched. `n if n > 3 =>`
/// binds `n` to the scrutinee (not in a tuple `match`).
pub(super) fn lower_guarded(
    cx: &Cx,
    scrutinee: &SynExpr,
    arms: &[syn::Arm],
    comments: &super::Comments,
) -> Result<Expr, ParseError> {
    let tuple = matches!(scrutinee, SynExpr::Tuple(t) if !t.elems.is_empty());
    let mut lowered = Vec::new();
    for arm in arms {
        let raw = lower_pat(cx, &arm.pat)?;
        let structs = cx.take_struct_pats();
        let guard = match &arm.guard {
            Some((_, g)) => Some(lower_expr(cx, g)?),
            None => None,
        };
        let (guard, wrap) = struct_arm(structs, guard).map_err(|e| e.or_at(arm.pat.span()))?;
        let pattern = match (&raw, &guard) {
            (Pattern::Var(_), Some(_)) if !tuple => raw,
            _ => arm_pattern(raw).map_err(|e| e.or_at(arm.pat.span()))?,
        };
        lowered.push(Arm {
            pattern,
            guard,
            body: comments.above(arm.span(), wrap(at(arm.body.span(), lower_expr(cx, &arm.body)?))),
        });
    }
    Ok(Expr::Match { scrutinee: Box::new(lower_expr(cx, scrutinee)?), arms: lowered })
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
    let raw = lower_pat(cx, &pat)?;
    let structs = cx.take_struct_pats();
    let has_structs = !structs.is_empty();
    let pattern = if has_structs && matches!(raw, Pattern::Var(_)) {
        raw
    } else {
        arm_pattern(raw).map_err(|e| e.or_at(pat.span()))?
    };
    if pattern == Pattern::Wildcard {
        return Err(ParseError::new(
            Reason::ArmPattern,
            "`matches!(x, _)` does not test `x`; write `true`, or the condition of `_ if c`",
        ));
    }
    let guard = match guard {
        Some(g) => Some(lower_expr(cx, &g)?),
        None => None,
    };
    let (guard, _) = struct_arm(structs, guard).map_err(|e| e.or_at(pat.span()))?;
    let hit = guard.unwrap_or(Expr::Lit(Lit::Bool(true)));
    Ok(Expr::Match {
        scrutinee: Box::new(lower_expr(cx, &scrutinee)?),
        arms: vec![
            Arm { guard: None, pattern, body: hit },
            Arm { guard: None, pattern: Pattern::Wildcard, body: Expr::Lit(Lit::Bool(false)) },
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
        Pat::Ident(id) if cx.is_local_const(&id.ident.to_string()) => Err(const_pattern(&id.ident.to_string())),
        Pat::Ident(id) if id.by_ref.is_none() && id.mutability.is_none() && id.subpat.is_none() => {
            Ok(Pattern::Var(Name::new(id.ident.to_string())))
        }
        Pat::Lit(l) => Ok(Pattern::Lit(lower_lit(&l.lit)?)),
        Pat::Range(r) => {
            let bound = |e: &Option<Box<syn::Expr>>| match e.as_deref() {
                Some(SynExpr::Lit(l)) => lower_lit(&l.lit),
                _ => {
                    Err(ParseError::new(Reason::UnsupportedPattern, "range patterns need a literal at both ends in v0"))
                }
            };
            Ok(Pattern::Range {
                lo: bound(&r.start)?,
                hi: bound(&r.end)?,
                inclusive: matches!(r.limits, syn::RangeLimits::Closed(_)),
            })
        }
        Pat::Path(p) => path_variant_pat(cx, &p.path, VariantBind::Unit),
        Pat::TupleStruct(t) => {
            let bind = VariantBind::Tuple(t.elems.iter().map(|e| lower_pat(cx, e)).collect::<Result<Vec<_>, _>>()?);
            path_variant_pat(cx, &t.path, bind)
        }
        // `S { f: p, .. }` of a struct: a fresh name here, its fields tested
        // and bound by the arm (`struct_arm`).
        Pat::Struct(s) if s.path.segments.len() == 1 && cx.is_struct(&s.path.segments[0].ident.to_string()) => {
            let fields = s
                .fields
                .iter()
                .map(|f| match &f.member {
                    Member::Named(id) => Ok((Name::new(id.to_string()), lower_pat(cx, &f.pat)?)),
                    Member::Unnamed(_) => {
                        Err(ParseError::new(Reason::PositionalFields, "unnamed fields in struct pattern"))
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            // Named after the struct: `$paymentMethod3` prints `paymentMethod`.
            let ty = s.path.segments[0].ident.to_string();
            let name = cx.fresh(&format!("{}{}", ty[..1].to_lowercase(), &ty[1..]));
            cx.push_struct_pat(crate::item::StructPat { name: name.clone(), fields });
            Ok(Pattern::Var(name))
        }
        Pat::Struct(s) => {
            let bind = VariantBind::Struct(
                s.fields
                    .iter()
                    .map(|f| {
                        let name = match &f.member {
                            Member::Named(id) => Name::new(id.to_string()),
                            Member::Unnamed(_) => {
                                return Err(ParseError::new(
                                    Reason::PositionalFields,
                                    "unnamed fields in struct pattern",
                                ))
                            }
                        };
                        Ok((name, lower_pat(cx, &f.pat)?))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
            path_variant_pat(cx, &s.path, bind)
        }
        Pat::Tuple(t) if t.elems.len() == 1 => lower_pat(cx, &t.elems[0]),
        Pat::Tuple(t) if t.elems.len() > 1 => {
            Ok(Pattern::Tuple(t.elems.iter().map(|p| lower_pat(cx, p)).collect::<Result<Vec<_>, _>>()?))
        }
        Pat::Or(o) => Ok(Pattern::Or(o.cases.iter().map(|p| lower_pat(cx, p)).collect::<Result<Vec<_>, _>>()?)),
        other => Err(ParseError::new(Reason::UnsupportedPattern, format!("unsupported pattern {}", snippet(other)))),
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
                    return Err(ParseError::new(
                        Reason::ArmPattern,
                        format!("each side of `|` must be a tuple pattern here, found {}", describe_pat(alt)),
                    ));
                };
                tuple_elems(elems)?;
            }
            if let Some(name) = pattern.bindings().first() {
                return Err(ParseError::new(
                    Reason::ArmPattern,
                    format!(
                        "`|` arms may not bind names in v0, found binding `{}`; write one arm per tuple",
                        name.as_str()
                    ),
                ));
            }
            return Ok(pattern);
        }
        Pattern::Or(alts) => {
            for alt in alts {
                if !matches!(alt, Pattern::Variant { .. }) {
                    return Err(ParseError::new(
                        Reason::ArmPattern,
                        format!("each side of `|` must name an enum variant in v0, found {}", describe_pat(alt)),
                    ));
                }
                variant_fields(alt)?;
                if let Some(name) = alt.bindings().first() {
                    return Err(ParseError::new(
                        Reason::ArmPattern,
                        format!(
                            "`|` arms may not bind names in v0, found binding `{}`; write one arm per variant",
                            name.as_str()
                        ),
                    ));
                }
            }
            return Ok(pattern);
        }
        _ => {}
    }
    variant_fields(&pattern)?;
    Ok(pattern)
}

/// The elements of a tuple arm: `_`, a binding, a tuple of these, or what
/// an arm of its own may be.
pub(super) fn tuple_elems(elems: &[Pattern]) -> Result<(), ParseError> {
    for elem in elems {
        field_pattern(elem)?;
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
    for p in inner {
        field_pattern(p)?;
    }
    Ok(())
}

/// A variant field (or the payload of `Some`, `Ok`, `Err`, or a tuple's
/// element) is `_`, a name, a tuple, or what an arm may be, nested to any
/// depth (`Some(Event::Pay { amount })`, `Checked { verified: false, .. }`,
/// `Some((1, b))`).
fn field_pattern(pattern: &Pattern) -> Result<(), ParseError> {
    match pattern {
        Pattern::Var(_) | Pattern::Wildcard => Ok(()),
        Pattern::Tuple(elems) => tuple_elems(elems),
        p => arm_pattern(p.clone()).map(|_| ()),
    }
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
        [ty, var] if cx.is_enum(ty) => {
            Ok(Pattern::Variant { ty: Name::new(ty.clone()), variant: Name::new(var.clone()), bind })
        }
        [var] => {
            if let Some(ty) = cx.enum_for_variant(var) {
                Ok(Pattern::Variant { ty: Name::new(ty), variant: Name::new(var.clone()), bind })
            } else {
                Err(ParseError::new(Reason::ExternalPath, format!("unknown variant {var}")).detail(var.to_string()))
            }
        }
        _ => Err(path_error(&segs, format!("unsupported pattern path {}", segs.join("::")))),
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
    if !cx.take_struct_pats().is_empty() {
        return Err(ParseError::new(
            Reason::UnsupportedPattern,
            "a struct pattern is in a `match` arm or `matches!` in v0, not `if let`",
        )
        .or_at(l.pat.span()));
    }
    let other = match &pattern {
        Pattern::OptionSome(_) => Pattern::OptionNone,
        Pattern::OptionNone => Pattern::OptionSome(Box::new(Pattern::Wildcard)),
        Pattern::ResultOk(_) => Pattern::ResultErr(Box::new(Pattern::Wildcard)),
        Pattern::ResultErr(_) => Pattern::ResultOk(Box::new(Pattern::Wildcard)),
        _ => {
            return Err(ParseError::new(
                Reason::IfLetVariant,
                "`if let` on an enum variant is not in v0; use `match` with every variant",
            )
            .or_at(l.pat.span()))
        }
    };
    Ok(Expr::Match {
        scrutinee: Box::new(lower_expr(cx, &l.expr)?),
        arms: vec![Arm { guard: None, pattern, body: then }, Arm { guard: None, pattern: other, body: else_ }],
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
        Ok(Some(Destructure { pattern: Pattern::Tuple(elems), rebind }))
    }

    pub(super) fn len(&self) -> usize {
        match &self.pattern {
            Pattern::Tuple(elems) => elems.len(),
            _ => 0,
        }
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
        Expr::Match { scrutinee: Box::new(scrutinee), arms: vec![Arm { guard: None, pattern: self.pattern, body }] }
    }
}

fn strip_refs(mut pat: &Pat) -> &Pat {
    while let Pat::Reference(r) = pat {
        pat = &r.pat;
    }
    pat
}

/// Rust matches a const's value where its name stands in a pattern; the IR
/// would bind a new name that matches anything.
pub(super) fn const_pattern(name: &str) -> ParseError {
    ParseError::new(
        Reason::UnsupportedPattern,
        format!("`{name}` is a const: matching against a const is not in v0; compare with `==` or write its value"),
    )
}

/// What the struct patterns of an arm add to it: a test of each refutable
/// field, before the arm's own guard, and a `let` of each bound field at the
/// head of its body. `P { m: M { kind: K::B, id } } if c => b` is
/// `P { m: $s } if matches!($s.kind, K::B) && c => { let id = $s.id; b }`, and
/// the guard reads `id` as `$s.id`. A field's pattern binds the whole field
/// or nothing: `M { kind: K::C(x) }` is refused.
pub(super) fn struct_arm(
    structs: Vec<crate::item::StructPat>,
    guard: Option<Expr>,
) -> Result<(Option<Expr>, impl FnOnce(Expr) -> Expr), ParseError> {
    let mut tests: Vec<Expr> = Vec::new();
    let mut lets: Vec<(Name, Expr)> = Vec::new();
    // Each name a struct pattern binds, read as its place in the guard.
    let mut places: Vec<(Name, Expr)> = Vec::new();
    for s in structs {
        let base =
            places.iter().find(|(n, _)| *n == s.name).map(|(_, p)| p.clone()).unwrap_or(Expr::Var(s.name.clone()));
        for (field, p) in s.fields {
            let in_body = Expr::Field { base: Box::new(Expr::Var(s.name.clone())), name: field.clone() };
            let in_guard = Expr::Field { base: Box::new(base.clone()), name: field };
            match p {
                Pattern::Wildcard => {}
                Pattern::Var(x) => {
                    lets.push((x.clone(), in_body));
                    places.push((x, in_guard));
                }
                p if p.bindings().is_empty() => tests.push(Expr::Match {
                    scrutinee: Box::new(in_guard),
                    arms: vec![
                        Arm { guard: None, pattern: p, body: Expr::Lit(Lit::Bool(true)) },
                        Arm { guard: None, pattern: Pattern::Wildcard, body: Expr::Lit(Lit::Bool(false)) },
                    ],
                }),
                _ => {
                    return Err(ParseError::new(
                        Reason::UnsupportedPattern,
                        "a field of a struct pattern is `_`, a name, a struct pattern, or a pattern that binds nothing (`K::B`, `0..=9`) in v0",
                    ))
                }
            }
        }
    }
    let guard = guard.map(|g| read_places(g, &places));
    let guard = tests.into_iter().chain(guard).reduce(|a, b| Expr::Binary {
        op: BinOp::And,
        left: Box::new(a),
        right: Box::new(b),
    });
    let wrap = move |body: Expr| {
        lets.into_iter().rev().fold(body, |then, (name, value)| Expr::Let {
            name,
            mutable: false,
            ty: None,
            value: Box::new(value),
            then: Box::new(then),
        })
    };
    Ok((guard, wrap))
}

/// `expr` with each read of a name in `places` replaced by its place,
/// except where a binding inside `expr` (a closure's parameter, a `let`, a
/// `match` arm's pattern, a `for` variable) hides the name.
fn read_places(mut expr: Expr, places: &[(Name, Expr)]) -> Expr {
    let without = |hidden: &[&Name]| -> Vec<(Name, Expr)> {
        places.iter().filter(|(n, _)| !hidden.contains(&n)).cloned().collect()
    };
    match &mut expr {
        Expr::Var(n) => {
            if let Some((_, p)) = places.iter().find(|(m, _)| m == n) {
                return p.clone();
            }
            return expr;
        }
        Expr::Closure { params, body, .. } => {
            let inner = without(&params.iter().map(|p| &p.name).collect::<Vec<_>>());
            let b = std::mem::replace(&mut **body, Expr::Lit(Lit::Unit));
            **body = read_places(b, &inner);
            return expr;
        }
        Expr::Let { name, value, then, .. } => {
            let v = std::mem::replace(&mut **value, Expr::Lit(Lit::Unit));
            **value = read_places(v, places);
            let inner = without(&[&*name]);
            let t = std::mem::replace(&mut **then, Expr::Lit(Lit::Unit));
            **then = read_places(t, &inner);
            return expr;
        }
        Expr::Match { scrutinee, arms } => {
            let sc = std::mem::replace(&mut **scrutinee, Expr::Lit(Lit::Unit));
            **scrutinee = read_places(sc, places);
            for arm in arms.iter_mut() {
                let bound = arm.pattern.bindings();
                let inner = without(&bound.iter().copied().collect::<Vec<_>>());
                if let Some(g) = arm.guard.take() {
                    arm.guard = Some(read_places(g, &inner));
                }
                let b = std::mem::replace(&mut arm.body, Expr::Lit(Lit::Unit));
                arm.body = read_places(b, &inner);
            }
            return expr;
        }
        Expr::For { var, start, end, body, .. } => {
            let st = std::mem::replace(&mut **start, Expr::Lit(Lit::Unit));
            **start = read_places(st, places);
            let en = std::mem::replace(&mut **end, Expr::Lit(Lit::Unit));
            **end = read_places(en, places);
            let inner = without(&[&*var]);
            let b = std::mem::replace(&mut **body, Expr::Lit(Lit::Unit));
            **body = read_places(b, &inner);
            return expr;
        }
        Expr::ForEach { var, source, body, .. } => {
            let src = std::mem::replace(&mut **source, Expr::Lit(Lit::Unit));
            **source = read_places(src, places);
            let inner = without(&[&*var]);
            let b = std::mem::replace(&mut **body, Expr::Lit(Lit::Unit));
            **body = read_places(b, &inner);
            return expr;
        }
        _ => {}
    }
    for c in expr.children_mut() {
        let e = std::mem::replace(c, Expr::Lit(Lit::Unit));
        *c = read_places(e, places);
    }
    expr
}

/// Whether `pat` holds a struct pattern, which the arm turns into a guard.
pub(super) fn has_struct_pat(cx: &Cx, pat: &Pat) -> bool {
    match pat {
        Pat::Struct(s) => {
            (s.path.segments.len() == 1 && cx.is_struct(&s.path.segments[0].ident.to_string()))
                || s.fields.iter().any(|f| has_struct_pat(cx, &f.pat))
        }
        Pat::TupleStruct(t) => t.elems.iter().any(|p| has_struct_pat(cx, p)),
        Pat::Tuple(t) => t.elems.iter().any(|p| has_struct_pat(cx, p)),
        Pat::Or(o) => o.cases.iter().any(|p| has_struct_pat(cx, p)),
        Pat::Paren(p) => has_struct_pat(cx, &p.pat),
        _ => false,
    }
}
