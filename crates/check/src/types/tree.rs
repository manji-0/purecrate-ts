//! `BTreeMap<String, V>` and `BTreeSet<String>` (design/01 §7.19): their
//! methods, typed into `Callee::Tree`.

use super::*;

impl<'d, 'a> Typer<'d, 'a> {
    /// A `Callee::Tree` call typed again (it is built typed): its arguments,
    /// and the type the first argument's map or set gives it.
    pub(super) fn tree_call(&mut self, op: TreeOp, args: &[Expr], want: Option<&Ty>) -> Typed {
        let typed: Vec<Typed> = args.iter().map(|a| self.expr(a, None)).collect();
        let first = typed.first().and_then(|(_, t)| t.as_ref()).map(|t| self.norm(t));
        let t = self.tree_ty(op, first, want);
        let e = Expr::Call { callee: Callee::Tree(op), args: typed.into_iter().map(|(e, _)| e).collect() };
        (e, self.expect(want, t))
    }

    /// The type of a `Callee::Tree` call whose first argument is of type
    /// `first` (normalized): a new map or set takes `want`'s.
    pub(super) fn tree_ty(&mut self, op: TreeOp, first: Option<Ty>, want: Option<&Ty>) -> Option<Ty> {
        match (op, first) {
            (TreeOp::NewMap | TreeOp::NewSet, _) => match want.map(|w| self.norm(w)) {
                Some(w @ Ty::Map { .. }) if op == TreeOp::NewMap => Some(w),
                Some(w @ Ty::Set(_)) if op == TreeOp::NewSet => Some(w),
                Some(Ty::Never) => None,
                _ => {
                    self.error(
                        Reason::NeedsAnnotation,
                        "the type of an empty map or set is not known here; annotate the binding (`let mut m: BTreeMap<String, u32> = BTreeMap::new();`)".to_string(),
                    );
                    None
                }
            },
            (TreeOp::Get | TreeOp::Insert | TreeOp::RemoveKey, Some(Ty::Map { value, .. })) => Some(Ty::Option(value)),
            (TreeOp::Has | TreeOp::IsEmpty | TreeOp::Add | TreeOp::Delete, _) => Some(Ty::bool()),
            (TreeOp::Len, _) => Some(Ty::Prim(Prim::Usize)),
            (TreeOp::CloneMap | TreeOp::CloneSet, t) => t,
            (TreeOp::Entries, Some(Ty::Map { key, value })) => Some(Ty::Vec(Box::new(Ty::Tuple(vec![*key, *value])))),
            (TreeOp::Keys, Some(Ty::Map { key, .. } | Ty::Set(key))) => Some(Ty::Vec(key)),
            (TreeOp::Values, Some(Ty::Map { value, .. })) => Some(Ty::Vec(value)),
            (TreeOp::FromPairs, Some(Ty::Vec(item))) => match *item {
                Ty::Tuple(kv) if kv.len() == 2 => {
                    Some(Ty::Map { key: Box::new(kv[0].clone()), value: Box::new(kv[1].clone()) })
                }
                _ => None,
            },
            (TreeOp::FromKeys, Some(Ty::Vec(item))) => Some(Ty::Set(item)),
            _ => None,
        }
    }

    /// A map's entries, keys, or values (`op`), or a set's keys, in key
    /// order as a new array, and its type; `ty` is `source`'s, normalized.
    pub(super) fn tree_items(&mut self, op: TreeOp, source: Expr, ty: Ty) -> (Expr, Option<Ty>) {
        let t = self.tree_ty(op, Some(ty), None);
        (Expr::Call { callee: Callee::Tree(op), args: vec![source] }, t)
    }

    /// `m.get(k)` and the other methods of a typed `BTreeMap` / `BTreeSet`
    /// receiver; `None` when `name` is none of them. A key argument is a
    /// `&str` or `String` (`&k`, `k.as_str()`, a literal).
    pub(super) fn tree_method(
        &mut self,
        recv: Expr,
        rt: &Ty,
        name: &str,
        args: &[Expr],
        want: Option<&Ty>,
    ) -> Option<Typed> {
        let map = matches!(rt, Ty::Map { .. });
        let op = match (name, args.len(), map) {
            ("get", 1, true) => TreeOp::Get,
            ("contains_key", 1, true) | ("contains", 1, false) => TreeOp::Has,
            ("len", 0, _) => TreeOp::Len,
            ("is_empty", 0, _) => TreeOp::IsEmpty,
            ("insert", 2, true) => TreeOp::Insert,
            ("insert", 1, false) => TreeOp::Add,
            ("remove", 1, true) => TreeOp::RemoveKey,
            ("remove", 1, false) => TreeOp::Delete,
            ("clone", 0, true) => TreeOp::CloneMap,
            ("clone", 0, false) => TreeOp::CloneSet,
            ("keys", 0, true) => TreeOp::Keys,
            ("values", 0, true) => TreeOp::Values,
            ("iter" | "into_iter", 0, true) => TreeOp::Entries,
            ("iter" | "into_iter", 0, false) => TreeOp::Keys,
            _ => return None,
        };
        if op.writes() && !matches!(recv.unpositioned(), Expr::Var(_)) {
            self.error(
                Reason::MethodCall,
                format!("`{name}` writes a local `let mut` map or set in v0, not a field, an element, or a call's value: build the new one and put it in a new value (design/01 §7.19)"),
            );
            return Some((Expr::Lit(Lit::Unit), None));
        }
        let key = Ty::Prim(Prim::Str);
        let mut call_args = vec![recv];
        for (i, a) in args.iter().enumerate() {
            let hint = match (op, i) {
                (TreeOp::Insert, 1) => match rt {
                    Ty::Map { value, .. } => Some((**value).clone()),
                    _ => None,
                },
                // `insert` takes the `String` it keeps; a lookup a `&str`.
                (TreeOp::Insert | TreeOp::Add, 0) => Some(Ty::Prim(Prim::String)),
                _ => Some(key.clone()),
            };
            let (e, t) = self.expr(a, hint.as_ref());
            if let (Some(t), 0) = (&t, i) {
                if !matches!(self.norm(t), Ty::Prim(Prim::Str | Prim::String) | Ty::Never) {
                    self.error(Reason::TypeMismatch, format!("a key is a `String` or `&str`, found `{}`", show(t)));
                }
            }
            call_args.push(e);
        }
        Some(self.tree_call(op, &call_args, want))
    }
}
