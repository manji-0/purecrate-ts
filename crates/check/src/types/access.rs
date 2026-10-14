//! `Vec`'s reads that may find nothing: `get`, `first`, `last`, and
//! `contains`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// `xs.get(i)`, `xs.first()`, `xs.last()`, and `xs.contains(&x)` on a
    /// typed `Vec<item>`. `None` when `name` is none of these.
    pub(super) fn vec_access(
        &mut self,
        recv: Expr,
        item: &Ty,
        name: &str,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
        let call = match (name, args) {
            ("get", [_]) | ("first" | "last", []) => {
                // `Some(None)` and `None` would both be `null`.
                if matches!(self.norm(item), Ty::Option(_)) {
                    self.error(Reason::NestedOption, format!(
                        "`{name}` on a `Vec<{}>` is `Option<Option<_>>`, and both `None`s are `null` in TS; index it, after checking `len()`",
                        show(item)
                    ));
                    return Some((Expr::Lit(Lit::Unit), None));
                }
                let (callee, rest) = match name {
                    "get" => (Callee::VecGet, vec![self.expr(&args[0], Some(&Ty::Prim(Prim::Usize))).0]),
                    "first" => (
                        Callee::VecGet,
                        vec![Expr::Lit(Lit::Int { value: 0, ty: Some(IntTy::Usize), byte: false, hex: false })],
                    ),
                    _ => (Callee::VecLast, vec![]),
                };
                let e = Expr::Call { callee, args: std::iter::once(recv).chain(rest).collect() };
                return Some((e, self.expect(want, Some(Ty::Option(Box::new(item.clone()))))));
            }
            ("contains", [x]) => {
                let (x, _) = self.expr(x, Some(item));
                Expr::Call { callee: Callee::VecContains, args: vec![recv, x] }
            }
            _ => return None,
        };
        Some((call, self.expect(want, Some(Ty::bool()))))
    }
}
