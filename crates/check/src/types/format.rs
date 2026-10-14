//! `format!`'s arguments: what `{}` prints the same in Rust and TS
//! (design/01 §6.8).

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
        for (i, t) in tys.iter().enumerate() {
            let Some(t) = t else { continue };
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
        // Rust aligns a number right and the rest left, unless told.
        let pieces = pieces
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
        let e = Expr::Call { callee: Callee::Format(pieces), args: typed };
        (e, self.expect(want, Some(Ty::Prim(Prim::String))))
    }

    /// `x.to_string()` on a primitive `t`: what `format!("{}", x)` prints,
    /// refused where it is; a `String` or `&str` is the value, as `clone` is.
    /// `None` where `t` has no `Display` (`()`), left to the usual message.
    pub(super) fn displayed_text(&mut self, recv: Expr, t: &Ty, want: Option<&Ty>) -> Option<Typed> {
        let string = Some(Ty::Prim(Prim::String));
        if matches!(t, Ty::Prim(Prim::String | Prim::Str)) {
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

/// What `{}` prints the same in Rust and TS: an integer, `bool`, `char`,
/// a string, or a `Uuid`.
fn displayed(t: &Ty) -> bool {
    matches!(t, Ty::Prim(Prim::Bool | Prim::Char | Prim::Str | Prim::String | Prim::Uuid | Prim::Usize) | Ty::Never)
        || matches!(t, Ty::Prim(p) if p.int().is_some())
}
