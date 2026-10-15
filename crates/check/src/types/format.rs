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
        let e = Expr::Call { callee: Callee::Format(pieces), args: typed };
        (e, self.expect(want, Some(Ty::Prim(Prim::String))))
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
        let e = Expr::Call { callee: Callee::Format(pieces), args: vec![recv] };
        Some((e, self.expect(want, string)))
    }
}

impl Typer<'_, '_> {
    /// The crate's own struct or enum `t` names.
    pub(super) fn crate_type(&self, t: &Ty) -> Option<Name> {
        let (n, _) = t.applied()?;
        (self.defs.structs.contains_key(n.as_str()) || self.defs.enums.contains_key(n.as_str())).then(|| n.clone())
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
    fn template(e: Expr) -> (Vec<FmtPiece>, Vec<Expr>) {
        if let Some(t) = literal(&e) {
            return (vec![FmtPiece::Text(t.to_string())], Vec::new());
        }
        match e {
            Expr::Call { callee: Callee::Format(pieces), args } => (pieces, args),
            _ => unreachable!("a text, tested by the caller"),
        }
    }
    let text = |e: &Expr| literal(e).is_some() || matches!(e, Expr::Call { callee: Callee::Format(_), .. });
    if !text(&a) || !text(&b) {
        return Expr::Call { callee: Callee::StrConcat, args: vec![a, b] };
    }
    let ((mut pieces, mut args), (more, rest)) = (template(a), template(b));
    let offset = args.len();
    for p in more {
        match (pieces.last_mut(), p) {
            (Some(FmtPiece::Text(t)), FmtPiece::Text(u)) => t.push_str(&u),
            (_, FmtPiece::Arg { index, spec }) => pieces.push(FmtPiece::Arg { index: index + offset, spec }),
            (_, text) => pieces.push(text),
        }
    }
    args.extend(rest);
    pieces.retain(|p| !matches!(p, FmtPiece::Text(t) if t.is_empty()));
    match pieces.as_slice() {
        [] => Expr::Call { callee: Callee::StringFrom, args: vec![Expr::Lit(Lit::Str(String::new()))] },
        [FmtPiece::Text(t)] => Expr::Call { callee: Callee::StringFrom, args: vec![Expr::Lit(Lit::Str(t.clone()))] },
        _ => Expr::Call { callee: Callee::Format(pieces), args },
    }
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
        "`{what}` prints `{0}` through its `impl Display`, and `{0}` has none translated: v0 reads a `fmt` that is `write!(f, \"..\", args)` on values `format!` prints, `f.write_str(\"..\")`, a `match self` of those, or several joined with `?` and ending in `Ok(())`; a `fmt` of any other shape is skipped",
        owner.as_str()
    )
}

/// What `{}` prints the same in Rust and TS: an integer, `bool`, `char`,
/// a string, or a `Uuid`.
fn displayed(t: &Ty) -> bool {
    matches!(t, Ty::Prim(Prim::Bool | Prim::Char | Prim::Str | Prim::String | Prim::Uuid | Prim::Usize) | Ty::Never)
        || matches!(t, Ty::Prim(p) if p.int().is_some())
}
