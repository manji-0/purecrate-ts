//! Every name the emitted TS will reference must exist in the crate, with the
//! shape the reference assumes. There is no rustc pass behind the parser.

use purecrate_ir::{Callee, Crate, Expr, Fields, Item, Name, Pattern, Ty, VariantBind, VariantFields};

use crate::defs::Defs;
use crate::Diagnostic;

pub fn check(krate: &Crate) -> Vec<Diagnostic> {
    let defs = Defs::of(krate);
    let mut out = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        let mut cx = Cx {
            defs: &defs,
            item: i,
            out: &mut out,
            scopes: Vec::new(),
        };
        match item {
            Item::Struct(s) => s.fields.iter().for_each(|f| cx.ty(&f.ty)),
            Item::Enum(e) => {
                for v in &e.variants {
                    match &v.fields {
                        VariantFields::Unit => {}
                        VariantFields::Tuple(tys) => tys.iter().for_each(|t| cx.ty(t)),
                        VariantFields::Struct(fs) => fs.iter().for_each(|f| cx.ty(&f.ty)),
                    }
                }
            }
            Item::Alias(a) => cx.ty(&a.ty),
            Item::Fn(f) => {
                if let Some(owner) = &f.owner {
                    if !defs.structs.contains_key(owner.as_str()) && !defs.enums.contains_key(owner.as_str()) {
                        cx.error(format!("`impl {}` has no struct or enum to attach to", owner.as_str()));
                    }
                }
                f.params.iter().for_each(|p| cx.ty(&p.ty));
                cx.ty(&f.ret);
                cx.scopes.push(f.params.iter().map(|p| p.name.as_str().to_string()).collect());
                cx.expr(&f.body);
            }
        }
    }
    out
}

struct Cx<'d, 'a> {
    defs: &'d Defs<'a>,
    item: usize,
    out: &'d mut Vec<Diagnostic>,
    scopes: Vec<Vec<String>>,
}

impl<'a> Cx<'_, 'a> {
    fn error(&mut self, message: String) {
        self.out.push(Diagnostic::at(self.item, message));
    }

    fn ty(&mut self, ty: &Ty) {
        match ty {
            Ty::Named(n) if !self.defs.is_type(n.as_str()) => {
                self.error(format!("type `{}` is not defined in this crate", n.as_str()))
            }
            Ty::Named(_) | Ty::Prim(_) | Ty::Never => {}
            Ty::Option(t) if self.is_option(t) => {
                self.error(
                    "`Option<Option<_>>` is not in v0: both `None` and `Some(None)` would be `null` in TS"
                        .to_string(),
                );
            }
            Ty::Option(t) | Ty::Vec(t) => self.ty(t),
            Ty::Result { ok, err } => {
                self.ty(ok);
                self.ty(err);
            }
            Ty::Tuple(ts) => ts.iter().for_each(|t| self.ty(t)),
        }
    }

    /// Through aliases; alias cycles stop after a fixed depth.
    fn is_option(&self, ty: &Ty) -> bool {
        let mut t = ty;
        for _ in 0..32 {
            match t {
                Ty::Option(_) => return true,
                Ty::Named(n) => match self.defs.aliases.get(n.as_str()) {
                    Some(a) => t = &a.ty,
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    }

    fn in_scope(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|s| s.iter().any(|n| n == name))
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(n) if !self.in_scope(n.as_str()) => {
                self.error(format!("`{}` is not a parameter or local binding", n.as_str()))
            }
            Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable => {}
            Expr::Let { name, ty, value, then } => {
                if let Some(t) = ty {
                    self.ty(t);
                }
                self.expr(value);
                self.scopes.push(vec![name.as_str().to_string()]);
                self.expr(then);
                self.scopes.pop();
            }
            Expr::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    let mut bound = Vec::new();
                    self.pattern(&arm.pattern, &mut bound);
                    self.scopes.push(bound);
                    self.expr(&arm.body);
                    self.scopes.pop();
                }
            }
            Expr::If { cond, then, else_ } => {
                self.expr(cond);
                self.expr(then);
                self.expr(else_);
            }
            Expr::Call { callee, args } => {
                self.callee(callee, args.len());
                args.iter().for_each(|a| self.expr(a));
            }
            Expr::Construct { ty, variant, fields } => {
                self.construct(ty, variant.as_ref(), fields);
                match fields {
                    Fields::Positional(xs) => xs.iter().for_each(|x| self.expr(x)),
                    Fields::Named(xs) => xs.iter().for_each(|(_, x)| self.expr(x)),
                    Fields::Unit => {}
                }
            }
            Expr::Field { base, .. }
            | Expr::Unary { expr: base, .. }
            | Expr::Return(base)
            | Expr::Try { expr: base, .. } => {
                self.expr(base)
            }
            Expr::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            Expr::Tuple(xs) | Expr::Array(xs) => xs.iter().for_each(|x| self.expr(x)),
        }
    }

    fn arity(&mut self, what: &str, want: usize, got: usize) {
        if want != got {
            self.error(format!("{what} takes {want} argument(s), got {got}"));
        }
    }

    fn callee(&mut self, callee: &Callee, argc: usize) {
        match callee {
            Callee::Fn(n) => match self.defs.free_fns.get(n.as_str()) {
                Some(f) => self.arity(&format!("`{}`", n.as_str()), f.params.len(), argc),
                None => self.error(format!("function `{}` is not defined in this crate", n.as_str())),
            },
            Callee::Method { ty, name } => {
                match self.defs.methods.get(&(ty.as_str(), name.as_str())) {
                    Some(f) => {
                        self.arity(&format!("`{}.{}`", ty.as_str(), name.as_str()), f.params.len(), argc)
                    }
                    None => self.error(format!(
                        "method `{}.{}` is not defined in this crate",
                        ty.as_str(),
                        name.as_str()
                    )),
                }
            }
            Callee::StructNew(n) => match self.defs.structs.get(n.as_str()) {
                Some(s) => self.arity(&format!("`{}`", n.as_str()), s.fields.len(), argc),
                None => self.error(format!("struct `{}` is not defined in this crate", n.as_str())),
            },
            Callee::Variant { ty, variant } => {
                let label = format!("`{}::{}`", ty.as_str(), variant.as_str());
                match self.variant(ty, variant).map(|v| &v.fields) {
                    Some(VariantFields::Tuple(tys)) => self.arity(&label, tys.len(), argc),
                    Some(_) => self.error(format!("{label} is not a tuple variant")),
                    None => {}
                }
            }
            Callee::ResultOk => self.arity("`Ok`", 1, argc),
            Callee::ResultErr => self.arity("`Err`", 1, argc),
            Callee::OptionSome => self.arity("`Some`", 1, argc),
            Callee::OptionNone => self.arity("`None`", 0, argc),
            Callee::Int { ty, op } => {
                self.arity(&format!("`{}` {}", ty.as_str(), op.as_str()), op.arity(), argc)
            }
            Callee::Fround => self.arity("`Math.fround`", 1, argc),
        }
    }

    fn construct(&mut self, ty: &Name, variant: Option<&Name>, fields: &Fields) {
        let t = ty.as_str();
        match variant {
            None => match self.defs.structs.get(t) {
                Some(s) => {
                    let declared: Vec<&str> = s.fields.iter().map(|f| f.name.as_str()).collect();
                    self.named_fields(t, &declared, fields);
                }
                None => self.error(format!("struct `{t}` is not defined in this crate")),
            },
            Some(v) => {
                let Some(var) = self.variant(ty, v) else { return };
                let label = format!("{t}::{}", v.as_str());
                match (&var.fields, fields) {
                    (VariantFields::Unit, Fields::Unit) => {}
                    (VariantFields::Tuple(tys), Fields::Positional(xs)) => {
                        self.arity(&format!("`{label}`"), tys.len(), xs.len())
                    }
                    (VariantFields::Struct(fs), _) => {
                        let declared: Vec<&str> = fs.iter().map(|f| f.name.as_str()).collect();
                        self.named_fields(&label, &declared, fields);
                    }
                    _ => self.error(format!("`{label}` is constructed with the wrong shape")),
                }
            }
        }
    }

    /// Reports and returns `None` when the enum or the variant is unknown.
    fn variant(&mut self, ty: &Name, variant: &Name) -> Option<&'a purecrate_ir::Variant> {
        let t = ty.as_str();
        let Some(e) = self.defs.enums.get(t).copied() else {
            self.error(format!("enum `{t}` is not defined in this crate"));
            return None;
        };
        let found = e.variants.iter().find(|x| x.name == *variant);
        if found.is_none() {
            self.error(format!("enum `{t}` has no variant `{}`", variant.as_str()));
        }
        found
    }

    fn named_fields(&mut self, label: &str, declared: &[&str], fields: &Fields) {
        let Fields::Named(given) = fields else {
            self.error(format!("`{label}` needs named fields"));
            return;
        };
        let given: Vec<&str> = given.iter().map(|(n, _)| n.as_str()).collect();
        let missing: Vec<&str> = declared.iter().copied().filter(|d| !given.contains(d)).collect();
        let extra: Vec<&str> = given.iter().copied().filter(|g| !declared.contains(g)).collect();
        if !missing.is_empty() {
            self.error(format!("`{label}` is missing field(s) {}", missing.join(", ")));
        }
        if !extra.is_empty() {
            self.error(format!("`{label}` has no field(s) {}", extra.join(", ")));
        }
    }

    fn pattern(&mut self, pattern: &Pattern, bound: &mut Vec<String>) {
        match pattern {
            Pattern::Var(n) => bound.push(n.as_str().to_string()),
            Pattern::Wildcard | Pattern::Lit(_) | Pattern::OptionNone => {}
            Pattern::OptionSome(p) | Pattern::ResultOk(p) | Pattern::ResultErr(p) => {
                self.pattern(p, bound)
            }
            Pattern::Variant { ty, variant, bind } => {
                let Some(var) = self.variant(ty, variant) else { return };
                let label = format!("{}::{}", ty.as_str(), variant.as_str());
                match (&var.fields, bind) {
                    (VariantFields::Unit, VariantBind::Unit) => {}
                    (VariantFields::Tuple(tys), VariantBind::Tuple(ps)) if tys.len() == ps.len() => {
                        ps.iter().for_each(|p| self.pattern(p, bound))
                    }
                    (VariantFields::Struct(fs), VariantBind::Struct(ps)) => {
                        for (field, p) in ps {
                            if fs.iter().any(|f| f.name == *field) {
                                self.pattern(p, bound);
                            } else {
                                self.error(format!("`{label}` has no field `{}`", field.as_str()));
                            }
                        }
                    }
                    _ => self.error(format!("pattern `{label}` does not match the variant's fields")),
                }
            }
        }
    }
}
