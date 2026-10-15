//! `format!`'s arguments: what `{}` prints the same in Rust and TS
//! (design/01 §6.8), and a crate type through its `Display` (§7.25).

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    pub(super) fn format(&mut self, pieces: &[FmtPiece], args: &[Expr], want: Option<&Ty>) -> Typed {
        let mut typed = Vec::with_capacity(args.len());
        let mut tys = Vec::with_capacity(args.len());
        for a in args {
            let (e, t) = self.expr(a, None);
            typed.push(e);
            tys.push(t.map(|t| self.norm(&t)));
        }
        let mut shown = vec![false; args.len()];
        for (i, t) in tys.iter().enumerate() {
            let Some(t) = t else { continue };
            // A `T: Display`: its dictionary's `toString`, where the
            // template places it, as a crate type's own.
            if let Ty::Param(p) = t {
                if let Some(dict) = self.display_dict(p) {
                    let padded = pieces
                        .iter()
                        .any(|piece| matches!(piece, FmtPiece::Arg { index, spec } if *index == i && !spec.is_plain()));
                    if padded {
                        self.error(
                            Reason::Macro,
                            format!(
                                "a width, alignment, or fill on a `{}` is not in v0: Rust pads it only where its type's `fmt` calls `f.pad`, which the output cannot know; pad its text: `format!(\"{{:>8}}\", x.to_string())`",
                                p.as_str()
                            ),
                        );
                    }
                    let arg = std::mem::replace(&mut typed[i], Expr::Lit(Lit::Unit));
                    let callee = Callee::Dict { name: Name::new("to_string") };
                    typed[i] = Expr::Call { callee, args: vec![Expr::Var(dict), arg] };
                    shown[i] = true;
                    continue;
                }
            }
            if let Some(owner) = self.crate_type(t) {
                if !self.has_display(&owner) {
                    self.error(Reason::TypeMismatch, no_display("format!", &owner));
                    continue;
                }
                let padded = pieces
                    .iter()
                    .any(|p| matches!(p, FmtPiece::Arg { index, spec } if *index == i && !spec.is_plain()));
                if padded {
                    self.error(
                        Reason::Macro,
                        format!(
                            "a width, alignment, or fill on `{}` is not in v0: Rust pads a value only where its `fmt` calls `f.pad`, which `write!` does not; pad its text: `format!(\"{{:>8}}\", x.to_string())`",
                            owner.as_str()
                        ),
                    );
                }
                let arg = std::mem::replace(&mut typed[i], Expr::Lit(Lit::Unit));
                typed[i] =
                    Expr::Call { callee: Callee::Method { ty: owner, name: Name::new("to_string") }, args: vec![arg] };
                shown[i] = true;
                continue;
            }
            if !displayed(t) {
                self.error(
                    Reason::TypeMismatch,
                    format!(
                        "`format!` prints an integer, `bool`, `char`, `&str`, `String`, or `Uuid` in v0, not `{}`",
                        show(t)
                    ),
                );
                continue;
            }
            for spec in pieces.iter().filter_map(|p| match p {
                FmtPiece::Arg { index, spec } if *index == i => Some(spec),
                _ => None,
            }) {
                let int = matches!(t, Ty::Prim(p) if p.int().is_some());
                if spec.zero && !int && !matches!(t, Ty::Never) {
                    self.error(
                        Reason::Macro,
                        format!("zero padding is for integers in v0; Rust pads `{}` with spaces: `{{:5}}`", show(t)),
                    );
                }
                if spec.width.is_some() && matches!(t, Ty::Prim(Prim::Uuid)) {
                    self.error(Reason::Macro, "`Uuid` prints without padding in Rust, whatever the width: `{}`".into());
                }
            }
        }
        // Rust evaluates every argument, then runs each `fmt` where the
        // string places it; the template runs a crate type's `toString` in
        // place too, so no argument that computes may come after one.
        let mut after_shown = false;
        for p in pieces {
            let FmtPiece::Arg { index, .. } = p else { continue };
            if after_shown && !is_read(&args[*index]) {
                self.error(
                    Reason::Macro,
                    "`format!` runs a crate type's `Display` after evaluating every argument, the template where it places it: bind each argument after it with `let` first and name it in the string (`{x}`)".to_string(),
                );
                break;
            }
            after_shown |= shown[*index];
        }
        // Rust aligns a number right and the rest left, unless told.
        let pieces: Vec<FmtPiece> = pieces
            .iter()
            .map(|p| match p {
                FmtPiece::Arg { index, spec } if spec.width.is_some() && spec.align.is_none() && !spec.zero => {
                    let int = matches!(&tys[*index], Some(Ty::Prim(p)) if p.int().is_some());
                    let align = if int { FmtAlign::Right } else { FmtAlign::Left };
                    FmtPiece::Arg { index: *index, spec: FmtSpec { align: Some(align), ..*spec } }
                }
                other => other.clone(),
            })
            .collect();
        // `format!("{}", x)` on a crate type is its text, not a template.
        if let ([FmtPiece::Arg { spec, .. }], [true]) = (pieces.as_slice(), shown.as_slice()) {
            if spec.is_plain() {
                let text = typed.into_iter().next().expect("one argument");
                return (text, self.expect(want, Some(Ty::Prim(Prim::String))));
            }
        }
        let strings = tys.iter().map(|t| matches!(t, Some(t) if is_string(t))).collect();
        (reduced(pieces, typed, strings), self.expect(want, Some(Ty::Prim(Prim::String))))
    }

    /// `x.to_string()` on a primitive `t`: what `format!("{}", x)` prints,
    /// refused where it is. A string, a `char`, and a `Uuid` are already the
    /// text `{}` prints (a `Uuid` its canonical form, §6.3 of design/01), so
    /// the value itself, as `clone` is. `None` where `t` has no `Display`
    /// (`()`), left to the usual message.
    pub(super) fn displayed_text(&mut self, recv: Expr, t: &Ty, want: Option<&Ty>) -> Option<Typed> {
        let string = Some(Ty::Prim(Prim::String));
        if matches!(t, Ty::Prim(Prim::String | Prim::Str | Prim::Char | Prim::Uuid)) {
            return Some((recv, self.expect(want, string)));
        }
        if matches!(t, Ty::Prim(p) if p.float().is_some()) {
            self.error(
                Reason::MethodCall,
                format!("`.to_string()` on `{}` is not in v0: `Display` of a float differs from JS past 1e21 and below 1e-7, as `format!` does", show(t)),
            );
            return Some((Expr::Lit(Lit::Unit), None));
        }
        if !displayed(t) {
            return None;
        }
        let pieces = vec![FmtPiece::Arg { index: 0, spec: FmtSpec::default() }];
        Some((reduced(pieces, vec![recv], vec![false]), self.expect(want, string)))
    }
}

impl Typer<'_, '_> {
    /// The crate's own struct or enum `t` names.
    pub(super) fn crate_type(&self, t: &Ty) -> Option<Name> {
        let (n, _) = t.applied()?;
        (self.defs.structs.contains_key(n.as_str()) || self.defs.enums.contains_key(n.as_str())).then(|| n.clone())
    }

    /// The dictionary in scope of the type parameter `p`'s `Display`
    /// bound.
    pub(super) fn display_dict(&self, p: &Name) -> Option<Name> {
        let defs = self.defs;
        self.dicts.iter().find(|(of, tr, _)| of == p && defs.extends(tr.as_str(), "Display")).map(|d| d.2.clone())
    }

    /// Whether the crate type `owner` has a `to_string` read from its
    /// `impl Display`.
    pub(super) fn has_display(&self, owner: &Name) -> bool {
        self.defs.methods.get(&(owner.as_str(), "to_string")).is_some_and(|f| f.display)
    }
}

/// `a` with `b` after it (`StrConcat`), as one template where both are a
/// `format!` or a literal: a `fmt`'s writes, which run their arguments and
/// `fmt`s in the order the joined template places them, each write's
/// checked on its own above. Anything else stays `a + b`.
pub(super) fn joined(a: Expr, b: Expr) -> Expr {
    fn literal(e: &Expr) -> Option<&str> {
        match e {
            Expr::Call { callee: Callee::StringFrom, args } => match args.as_slice() {
                [Expr::Lit(Lit::Str(t))] => Some(t),
                _ => None,
            },
            Expr::Lit(Lit::Str(t)) => Some(t),
            _ => None,
        }
    }
    // A string `reduced` left as itself (`String::from` of it) is placed
    // in the template again.
    fn template(e: Expr) -> (Vec<FmtPiece>, Vec<Expr>) {
        if let Some(t) = literal(&e) {
            return (vec![FmtPiece::Text(t.to_string())], Vec::new());
        }
        match e {
            Expr::Call { callee: Callee::Format(pieces), args } => (pieces, args),
            e @ Expr::Call { callee: Callee::StringFrom, .. } => {
                (vec![FmtPiece::Arg { index: 0, spec: FmtSpec::default() }], vec![e])
            }
            _ => unreachable!("a text, tested by the caller"),
        }
    }
    let text = |e: &Expr| matches!(e, Expr::Call { callee: Callee::Format(_) | Callee::StringFrom, .. });
    if !(literal(&a).is_some() || text(&a)) || !(literal(&b).is_some() || text(&b)) {
        return Expr::Call { callee: Callee::StrConcat, args: vec![a, b] };
    }
    let ((mut pieces, mut args), (more, rest)) = (template(a), template(b));
    let offset = args.len();
    pieces.extend(more.into_iter().map(|p| match p {
        FmtPiece::Arg { index, spec } => FmtPiece::Arg { index: index + offset, spec },
        text => text,
    }));
    args.extend(rest);
    let strings = args.iter().map(|a| matches!(a, Expr::Call { callee: Callee::StringFrom, .. })).collect();
    reduced(pieces, args, strings)
}

/// A `format!` as a template the type-aware lint accepts
/// (`no-unnecessary-template-expression`, design/01 §6.8): a literal placed
/// without a spec is text, which leaves its argument unused (a literal runs
/// nothing, so the order stays Rust's); text alone is that string; and a
/// string placed alone (`strings[i]`: a `String`, `&str`, `char`, or
/// `Uuid`) is itself, as `String::from` it, which `joined` places again.
/// A template placed once without a spec (a `format!` or an integer's
/// `to_string()` as an argument) is its pieces in its place, which run its
/// arguments where it ran them.
fn reduced(pieces: Vec<FmtPiece>, args: Vec<Expr>, strings: Vec<bool>) -> Expr {
    let (pieces, args, strings) = spliced(pieces, args, strings);
    fn shown(e: &Expr) -> Option<String> {
        match e {
            Expr::Lit(Lit::Str(s)) => Some(s.clone()),
            Expr::Lit(Lit::Char(c)) => Some(c.to_string()),
            Expr::Lit(Lit::Bool(b)) => Some(b.to_string()),
            Expr::Lit(Lit::Int { value, .. }) => Some(value.to_string()),
            Expr::Call { callee: Callee::StringFrom, args } => match args.as_slice() {
                [Expr::Lit(Lit::Str(s))] => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    }
    let mut out: Vec<FmtPiece> = Vec::with_capacity(pieces.len());
    let mut used = vec![false; args.len()];
    for p in pieces {
        let p = match p {
            FmtPiece::Arg { index, spec } if spec.is_plain() => match shown(&args[index]) {
                Some(t) => FmtPiece::Text(t),
                None => FmtPiece::Arg { index, spec },
            },
            p => p,
        };
        match (out.last_mut(), p) {
            (_, FmtPiece::Text(t)) if t.is_empty() => {}
            (Some(FmtPiece::Text(t)), FmtPiece::Text(u)) => t.push_str(&u),
            (_, p) => {
                if let FmtPiece::Arg { index, .. } = p {
                    used[index] = true;
                }
                out.push(p);
            }
        }
    }
    let mut renumbered = vec![0; args.len()];
    let mut kept = Vec::with_capacity(args.len());
    let mut kept_strings = Vec::with_capacity(args.len());
    for (i, (a, s)) in args.into_iter().zip(strings).enumerate() {
        if used[i] {
            renumbered[i] = kept.len();
            kept.push(a);
            kept_strings.push(s);
        }
    }
    for p in &mut out {
        if let FmtPiece::Arg { index, .. } = p {
            *index = renumbered[*index];
        }
    }
    let text = |t: &str| Expr::Call { callee: Callee::StringFrom, args: vec![Expr::Lit(Lit::Str(t.to_string()))] };
    match out.as_slice() {
        [] => text(""),
        [FmtPiece::Text(t)] => text(t),
        [FmtPiece::Arg { index, spec }] if spec.is_plain() && kept_strings[*index] => {
            match kept.pop().expect("the one argument") {
                e @ Expr::Call { callee: Callee::StringFrom, .. } => e,
                e => Expr::Call { callee: Callee::StringFrom, args: vec![e] },
            }
        }
        _ => Expr::Call { callee: Callee::Format(out), args: kept },
    }
}

/// `reduced`'s templates placed once without a spec, as their pieces; the
/// arguments renumbered in the order placed, those not placed dropped.
fn spliced(pieces: Vec<FmtPiece>, args: Vec<Expr>, strings: Vec<bool>) -> (Vec<FmtPiece>, Vec<Expr>, Vec<bool>) {
    let inline: Vec<bool> = (0..args.len())
        .map(|i| {
            let mut uses = pieces.iter().filter(|p| matches!(p, FmtPiece::Arg { index, .. } if *index == i));
            matches!((uses.next(), uses.next()), (Some(FmtPiece::Arg { spec, .. }), None) if spec.is_plain())
                && matches!(args[i], Expr::Call { callee: Callee::Format(_), .. })
        })
        .collect();
    let mut args: Vec<Option<Expr>> = args.into_iter().map(Some).collect();
    let mut slot: Vec<Option<usize>> = vec![None; args.len()];
    let (mut out, mut kept, mut kept_strings) = (Vec::new(), Vec::new(), Vec::new());
    for p in pieces {
        match p {
            FmtPiece::Arg { index, .. } if inline[index] => {
                let Some(Expr::Call { callee: Callee::Format(inner), args: inner_args }) = args[index].take() else {
                    unreachable!("a template, tested above")
                };
                let offset = kept.len();
                kept_strings.extend(inner_args.iter().map(|_| false));
                kept.extend(inner_args);
                out.extend(inner.into_iter().map(|q| match q {
                    FmtPiece::Arg { index, spec } => FmtPiece::Arg { index: index + offset, spec },
                    text => text,
                }));
            }
            FmtPiece::Arg { index, spec } => {
                let new = match slot[index] {
                    Some(n) => n,
                    None => {
                        kept.push(args[index].take().expect("placed once"));
                        kept_strings.push(strings[index]);
                        slot[index] = Some(kept.len() - 1);
                        kept.len() - 1
                    }
                };
                out.push(FmtPiece::Arg { index: new, spec });
            }
            text => out.push(text),
        }
    }
    (out, kept, kept_strings)
}

/// A type whose `{}` is its own text: a string, a `char`, or a `Uuid`.
fn is_string(t: &Ty) -> bool {
    matches!(t, Ty::Prim(Prim::String | Prim::Str | Prim::Char | Prim::Uuid))
}

/// A read that gives the same value whenever it runs: a local, a literal,
/// or a field of one (borrows are erased), as `syntax` reads one.
fn is_read(e: &Expr) -> bool {
    match e {
        Expr::Var(_) | Expr::Lit(_) => true,
        Expr::Field { base, .. } | Expr::At { expr: base, .. } => is_read(base),
        _ => false,
    }
}

/// Why `what` (`format!`, `.to_string()`) cannot print the crate type
/// `owner`: no `impl Display` of it was translated.
pub(super) fn no_display(what: &str, owner: &Name) -> String {
    format!(
        "`{what}` prints `{0}` through its `impl Display`, and `{0}` has none translated: v0 reads a `fmt` that is `write!(f, \"..\", args)` on values `format!` prints, `f.write_str(s)`, a `match self` of those, or several joined with `?` and ending in `Ok(())`; a `fmt` of any other shape is skipped",
        owner.as_str()
    )
}

/// What `{}` prints the same in Rust and TS: an integer, `bool`, `char`,
/// a string, or a `Uuid`.
fn displayed(t: &Ty) -> bool {
    matches!(t, Ty::Prim(Prim::Bool | Prim::Char | Prim::Str | Prim::String | Prim::Uuid | Prim::Usize) | Ty::Never)
        || matches!(t, Ty::Prim(p) if p.int().is_some())
}
