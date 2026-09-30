//! Keep `pub` items and whatever they reach; drop the rest. Run after `check`,
//! which guarantees the names used here are unique.

use std::collections::HashMap;

use purecrate_ir::{Callee, Crate, Expr, Item, Pattern, Ty, VariantFields, Vis};

pub fn prune_unreachable(krate: &Crate) -> Crate {
    let keep = reachable(krate);
    let items = krate
        .items
        .iter()
        .zip(keep)
        .filter(|(_, kept)| *kept)
        .map(|(item, _)| item.clone())
        .collect();
    Crate {
        name: krate.name.clone(),
        items,
    }
}

fn reachable(krate: &Crate) -> Vec<bool> {
    let mut top: HashMap<&str, usize> = HashMap::new();
    let mut methods: HashMap<(&str, &str), usize> = HashMap::new();
    for (i, item) in krate.items.iter().enumerate() {
        match item {
            Item::Fn(purecrate_ir::Fn {
                owner: Some(owner),
                name,
                ..
            }) => {
                methods.insert((owner.as_str(), name.as_str()), i);
            }
            other => {
                top.insert(other.name().as_str(), i);
            }
        }
    }

    // Methods start unkept: they follow their owner below.
    let mut keep: Vec<bool> = krate
        .items
        .iter()
        .map(|item| item.vis() == Vis::Pub && !matches!(item, Item::Fn(f) if f.owner.is_some()))
        .collect();

    loop {
        let mut changed = false;
        let mut mark = |i: usize, keep: &mut Vec<bool>| {
            if !keep[i] {
                keep[i] = true;
                changed = true;
            }
        };
        for (i, item) in krate.items.iter().enumerate() {
            if let Item::Fn(f) = item {
                let owner_kept = f
                    .owner
                    .as_ref()
                    .and_then(|o| top.get(o.as_str()))
                    .is_some_and(|&o| keep[o]);
                if owner_kept && f.vis == Vis::Pub {
                    mark(i, &mut keep);
                }
            }
            if !keep[i] {
                continue;
            }
            let mut refs = Refs::default();
            refs.item(item);
            for name in &refs.top {
                if let Some(&j) = top.get(name.as_str()) {
                    mark(j, &mut keep);
                }
            }
            for (owner, name) in &refs.methods {
                if let Some(&j) = methods.get(&(owner.as_str(), name.as_str())) {
                    mark(j, &mut keep);
                }
            }
            if let Item::Fn(f) = item {
                if let Some(&o) = f.owner.as_ref().and_then(|o| top.get(o.as_str())) {
                    mark(o, &mut keep);
                }
            }
        }
        if !changed {
            return keep;
        }
    }
}

#[derive(Default)]
struct Refs {
    top: Vec<String>,
    methods: Vec<(String, String)>,
}

impl Refs {
    fn name(&mut self, n: &purecrate_ir::Name) {
        self.top.push(n.as_str().to_string());
    }

    fn item(&mut self, item: &Item) {
        match item {
            Item::Struct(s) => {
                s.fields.iter().for_each(|f| self.ty(&f.ty));
                // The wire schema reads `T` and calls `try_from`.
                if let Some(t) = &s.wire_from {
                    self.ty(t);
                    self.methods.push((s.name.as_str().to_string(), "try_from".to_string()));
                }
            }
            Item::Enum(e) => {
                for v in &e.variants {
                    match &v.fields {
                        VariantFields::Unit => {}
                        VariantFields::Tuple(tys) => tys.iter().for_each(|t| self.ty(t)),
                        VariantFields::Struct(fs) => fs.iter().for_each(|f| self.ty(&f.ty)),
                    }
                }
            }
            Item::Alias(a) => self.ty(&a.ty),
            Item::Fn(f) => {
                f.params.iter().for_each(|p| self.ty(&p.ty));
                self.ty(&f.ret);
                self.expr(&f.body);
            }
            Item::Const(c) => self.expr(&c.value),
        }
    }

    fn ty(&mut self, ty: &Ty) {
        match ty {
            Ty::Named(n) => self.name(n),
            Ty::Option(t) | Ty::Vec(t) => self.ty(t),
            Ty::Result { ok, err } => {
                self.ty(ok);
                self.ty(err);
            }
            Ty::Tuple(ts) => ts.iter().for_each(|t| self.ty(t)),
            Ty::Fn { params, ret } => {
                params.iter().for_each(|t| self.ty(t));
                self.ty(ret);
            }
            Ty::Ignored { inner, .. } => self.ty(inner),
            Ty::Prim(_) | Ty::Never => {}
        }
    }

    fn pattern(&mut self, p: &Pattern) {
        match p {
            Pattern::Variant { ty, .. } => self.name(ty),
            Pattern::Or(alts) | Pattern::Tuple(alts) => alts.iter().for_each(|alt| self.pattern(alt)),
            _ => {}
        }
    }

    /// The items `expr` names; everything else is walked through.
    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Call { callee, .. } => match callee {
                Callee::Fn(n) | Callee::StructNew(n) | Callee::Variant { ty: n, .. } => self.name(n),
                Callee::Method { ty, name } => {
                    self.name(ty);
                    self.methods.push((ty.as_str().to_string(), name.as_str().to_string()));
                }
                _ => {}
            },
            Expr::Construct { ty, .. } => self.name(ty),
            Expr::Match { arms, .. } => arms.iter().for_each(|arm| self.pattern(&arm.pattern)),
            // A const; `rename` keeps local names off every item name.
            Expr::Var(n) => self.name(n),
            _ => {}
        }
        expr.own_types().into_iter().for_each(|t| self.ty(t));
        expr.children().into_iter().for_each(|c| self.expr(c));
    }
}
