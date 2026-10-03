//! Every name the emitted TS will reference must exist in the crate, with the
//! shape the reference assumes. There is no rustc pass behind the parser.

use purecrate_ir::{
    Callee, Crate, Expr, Fields, Item, Name, Pattern, Prim, Reason, Ty, VariantBind, VariantFields,
    NEWTYPE_FIELD,
};

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
            closures: Vec::new(),
        };
        match item {
            Item::Struct(s) => {
                s.fields.iter().for_each(|f| cx.ty(&f.ty));
                if let Some(inner) = s.newtype_inner().filter(|t| cx.is_nullish(t)) {
                    cx.error(Reason::NewtypeInner, format!(
                        "`{}` wraps `{}`, which is `null` or `undefined` in TS and cannot carry a brand",
                        s.name.as_str(),
                        crate::types::show(inner)
                    ));
                }
            }
            Item::Enum(e) => {
                for v in &e.variants {
                    match &v.fields {
                        VariantFields::Unit => {}
                        VariantFields::Tuple(tys) => tys.iter().for_each(|t| cx.ty(t)),
                        VariantFields::Struct(fs) => fs.iter().for_each(|f| cx.ty(&f.ty)),
                    }
                    if let Some(d) = &v.discriminant {
                        cx.expr(d);
                    }
                }
            }
            Item::Alias(a) => cx.ty(&a.ty),
            Item::Const(c) => {
                cx.ty(&c.ty);
                cx.expr(&c.value);
            }
            Item::Fn(f) => {
                if let Some(owner) = &f.owner {
                    if !defs.structs.contains_key(owner.as_str()) && !defs.enums.contains_key(owner.as_str()) {
                        cx.error_about(
                            Reason::UndefinedType,
                            owner.as_str(),
                            format!("`impl {}` has no struct or enum to attach to", owner.as_str()),
                        );
                    }
                }
                f.params.iter().for_each(|p| cx.ty(&p.ty));
                cx.ty(&f.ret);
                cx.scopes.push(f.params.iter().map(|p| (p.name.as_str().to_string(), false)).collect());
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
    /// Bindings in scope, each with whether it is `let mut`.
    scopes: Vec<Vec<(String, bool)>>,
    /// `scopes.len()` where each enclosing closure's own bindings start.
    closures: Vec<usize>,
}

impl<'a> Cx<'_, 'a> {
    fn error(&mut self, reason: Reason, message: String) {
        self.out.push(Diagnostic::at(self.item, reason, message));
    }

    fn error_about(&mut self, reason: Reason, detail: &str, message: String) {
        self.out.push(Diagnostic::at(self.item, reason, message).about(detail));
    }

    fn ty(&mut self, ty: &Ty) {
        match ty {
            // A hole of `collect::<Vec<_>>()`, filled when typing.
            Ty::Named(n) if n.as_str() == "_" => {}
            Ty::Named(n) if !self.defs.is_type(n.as_str()) => {
                self.error_about(
                    Reason::UndefinedType,
                    n.as_str(),
                    format!("type `{}` is not defined in this crate", n.as_str()),
                )
            }
            Ty::Ignored { inner, .. } => self.ty(inner),
            Ty::Named(_) | Ty::Prim(_) | Ty::Never => {}
            Ty::Fn { params, ret } => {
                params.iter().for_each(|t| self.ty(t));
                self.ty(ret);
            }
            Ty::Option(t) if self.is_option(t) => {
                self.error(Reason::NestedOption, 
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

    /// `Option`, `()` or `!`, looking through aliases.
    fn is_nullish(&self, ty: &Ty) -> bool {
        let mut t = ty;
        for _ in 0..32 {
            match t {
                Ty::Option(_) | Ty::Prim(Prim::Unit) | Ty::Never => return true,
                Ty::Named(n) => match self.defs.aliases.get(n.as_str()) {
                    Some(a) => t = &a.ty,
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    }

    /// Through aliases; alias cycles stop after a fixed depth.
    fn is_option(&self, ty: &Ty) -> bool {
        let mut t = ty.peel();
        for _ in 0..32 {
            match t {
                Ty::Option(_) => return true,
                Ty::Named(n) => match self.defs.aliases.get(n.as_str()) {
                    Some(a) => t = a.ty.peel(),
                    None => return false,
                },
                _ => return false,
            }
        }
        false
    }

    /// `Some(mutable)` for the innermost binding of `name`.
    fn binding(&self, name: &str) -> Option<bool> {
        self.binding_at(name).map(|(_, m)| m)
    }

    /// The innermost binding of `name`: its scope depth and whether it is `let mut`.
    fn binding_at(&self, name: &str) -> Option<(usize, bool)> {
        self.scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(depth, s)| s.iter().rev().find(|(n, _)| n == name).map(|(_, m)| (depth, *m)))
    }

    /// A closure sees a `let mut` binding by reference in JS but, under
    /// `move`, by a copy in Rust; a later assignment would tell them apart.
    fn captures_mutable(&self, name: &str) -> bool {
        match (self.binding_at(name), self.closures.last()) {
            (Some((depth, true)), Some(&floor)) => depth < floor,
            _ => false,
        }
    }

    fn capture_error(&mut self, name: &str) {
        self.error_about(
            Reason::Closure,
            name,
            format!("a closure cannot capture `let mut {name}` in v0; bind its current value with `let` first"),
        );
    }

    fn in_scope(&self, name: &str) -> bool {
        self.binding(name).is_some()
    }

    fn expr(&mut self, expr: &Expr) {
        match expr {
            Expr::At { at, expr } => {
                let before = self.out.len();
                self.expr(expr);
                crate::locate(&mut self.out[before..], *at);
            }
            Expr::Var(n) if !self.in_scope(n.as_str()) && self.defs.consts.contains_key(n.as_str()) => {}
            Expr::Var(n) if !self.in_scope(n.as_str()) => {
                self.error(Reason::UndefinedName, format!("`{}` is not a parameter or local binding", n.as_str()))
            }
            Expr::Var(n) if self.captures_mutable(n.as_str()) => self.capture_error(n.as_str()),
            Expr::Lit(_) | Expr::Var(_) | Expr::Unreachable | Expr::Comment(_) => {}
            Expr::Closure { params, ret, body } => {
                params.iter().filter_map(|p| p.ty.as_ref()).for_each(|t| self.ty(t));
                if let Some(t) = ret {
                    self.ty(t);
                }
                self.closures.push(self.scopes.len());
                self.scopes.push(params.iter().map(|p| (p.name.as_str().to_string(), false)).collect());
                self.expr(body);
                self.scopes.pop();
                self.closures.pop();
            }
            Expr::Let {
                name,
                mutable,
                ty,
                value,
                then,
            } => {
                if let Some(t) = ty {
                    self.ty(t);
                }
                self.expr(value);
                self.scopes.push(vec![(name.as_str().to_string(), *mutable)]);
                self.expr(then);
                self.scopes.pop();
            }
            Expr::Match { scrutinee, arms } => {
                self.expr(scrutinee);
                for arm in arms {
                    let mut bound = Vec::new();
                    self.pattern(&arm.pattern, &mut bound);
                    self.scopes.push(bound.into_iter().map(|n| (n, false)).collect());
                    if let Some(guard) = &arm.guard {
                        self.expr(guard);
                    }
                    self.expr(&arm.body);
                    self.scopes.pop();
                }
            }
            Expr::If { cond, then, else_ } => {
                self.expr(cond);
                self.expr(then);
                self.expr(else_);
            }
            Expr::Call {
                callee: Callee::Fn(n) | Callee::Local(n),
                args,
            } if self.in_scope(n.as_str()) => {
                if self.captures_mutable(n.as_str()) {
                    self.capture_error(n.as_str());
                }
                args.iter().for_each(|a| self.expr(a));
            }
            Expr::Call { callee, args } => {
                self.callee(callee, args.len());
                args.iter().for_each(|a| self.expr(a));
            }
            Expr::MethodCall { receiver, name, args } => {
                self.expr(receiver);
                for a in args {
                    // `opt.map(f)`, `r.map_err(f)`, `it.all(f)`, and `ord.then_with(f)` name a
                    // function; `types` checks the receiver.
                    let calls_with = if name.as_str() == "then_with" { 0 } else { 1 };
                    let takes_fn = matches!(name.as_str(), "map" | "map_err" | "all" | "any" | "position" | "then_with");
                    let fn_name = match a.unpositioned() {
                        Expr::Var(n) if takes_fn && !self.in_scope(n.as_str()) => {
                            self.defs.free_fns.get(n.as_str()).map(|f| f.params.len())
                        }
                        // `E::V` as a function of its one field.
                        Expr::Construct { ty, variant: Some(v), fields: Fields::Unit, base: None }
                            if takes_fn && one_field_variant(self.defs, ty, v) =>
                        {
                            Some(1)
                        }
                        _ => None,
                    };
                    match fn_name {
                        Some(p) if p == calls_with => {}
                        Some(p) => self.error(Reason::ConstructShape, format!(
                            "`{}` calls its function with {calls_with} argument{}, which takes {p}",
                            name.as_str(),
                            if calls_with == 1 { "" } else { "s" }
                        )),
                        None => self.expr(a),
                    }
                }
            }
            Expr::Index { base, index } => {
                self.expr(base);
                self.expr(index);
            }
            Expr::Cast { expr, to } => {
                self.ty(to);
                self.expr(expr);
            }
            Expr::While { cond, body } => {
                self.expr(cond);
                self.scopes.push(Vec::new());
                self.expr(body);
                self.scopes.pop();
            }
            Expr::Break | Expr::Continue => {}
            Expr::Construct { ty, variant, fields, base } => {
                self.construct(ty, variant.as_ref(), fields, base.is_some());
                match fields {
                    Fields::Positional(xs) => xs.iter().for_each(|x| self.expr(x)),
                    Fields::Named(xs) => xs.iter().for_each(|(_, x)| self.expr(x)),
                    Fields::Unit => {}
                }
                if let Some(b) = base {
                    self.expr(b);
                }
            }
            Expr::Field { base, .. }
            | Expr::Unary { expr: base, .. }
            | Expr::Return(base)
            | Expr::Try { expr: base, .. }
            | Expr::Ignored { expr: base, .. } => {
                self.expr(base)
            }
            Expr::Binary { left, right, .. } => {
                self.expr(left);
                self.expr(right);
            }
            Expr::Tuple(xs) | Expr::Array(xs) => xs.iter().for_each(|x| self.expr(x)),
            Expr::Assign { name, value } if self.captures_mutable(name.as_str()) => {
                self.capture_error(name.as_str());
                self.expr(value);
            }
            Expr::Assign { name, value } => {
                match self.binding(name.as_str()) {
                    None => self.error(Reason::UndefinedName, format!("`{}` is not a parameter or local binding", name.as_str())),
                    Some(false) => self.error(Reason::ImmutableAssign, format!(
                        "`{}` is not `let mut`; only `let mut` bindings can be assigned",
                        name.as_str()
                    )),
                    Some(true) => {}
                }
                self.expr(value);
            }
            Expr::Seq { first, then } => {
                self.expr(first);
                self.expr(then);
            }
            Expr::For { var, start, end, body, .. } => {
                self.expr(start);
                self.expr(end);
                self.scopes.push(vec![(var.as_str().to_string(), false)]);
                self.expr(body);
                self.scopes.pop();
            }
            Expr::ForEach { var, source: string, body, .. } => {
                self.expr(string);
                self.scopes.push(vec![(var.as_str().to_string(), false)]);
                self.expr(body);
                self.scopes.pop();
            }
        }
    }

    fn arity(&mut self, what: &str, want: usize, got: usize) {
        if want != got {
            self.error(Reason::ConstructShape, format!("{what} takes {want} argument(s), got {got}"));
        }
    }

    fn callee(&mut self, callee: &Callee, argc: usize) {
        match callee {
            Callee::Local(_) => {}
            Callee::Fn(n) => match self.defs.free_fns.get(n.as_str()) {
                Some(f) => self.arity(&format!("`{}`", n.as_str()), f.params.len(), argc),
                None => self.error_about(
                    Reason::UndefinedFn,
                    n.as_str(),
                    format!("function `{}` is not defined in this crate", n.as_str()),
                ),
            },
            Callee::Method { ty, name } => {
                match self.defs.methods.get(&(ty.as_str(), name.as_str())) {
                    Some(f) => {
                        self.arity(&format!("`{}.{}`", ty.as_str(), name.as_str()), f.params.len(), argc)
                    }
                    None => self.error_about(Reason::UndefinedFn, &format!("{}.{}", ty.as_str(), name.as_str()), format!(
                        "method `{}.{}` is not defined in this crate",
                        ty.as_str(),
                        name.as_str()
                    )),
                }
            }
            Callee::StructNew(n) => match self.defs.structs.get(n.as_str()) {
                Some(s) => self.arity(&format!("`{}`", n.as_str()), s.fields.len(), argc),
                None => self.error_about(
                    Reason::UndefinedType,
                    n.as_str(),
                    format!("struct `{}` is not defined in this crate", n.as_str()),
                ),
            },
            Callee::Variant { ty, variant } => {
                let label = format!("`{}::{}`", ty.as_str(), variant.as_str());
                match self.variant(ty, variant).map(|v| &v.fields) {
                    Some(VariantFields::Tuple(tys)) => self.arity(&label, tys.len(), argc),
                    Some(_) => self.error(Reason::ConstructShape, format!("{label} is not a tuple variant")),
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
            Callee::AsFloat(_) => self.arity("`as float`", 1, argc),
            Callee::VecLen => self.arity("`Vec::len`", 1, argc),
            Callee::VecIsEmpty => self.arity("`Vec::is_empty`", 1, argc),
            Callee::OptionIsSome => self.arity("`Option::is_some`", 1, argc),
            Callee::OptionIsNone => self.arity("`Option::is_none`", 1, argc),
            Callee::StrBytes => self.arity("`str::as_bytes`", 1, argc),
            Callee::StrCmp => self.arity("`str::cmp`", 2, argc),
            Callee::OrdCmp { .. } | Callee::OrdCmpList { .. } => self.arity("`cmp`", 2, argc),
            Callee::OrdThen => self.arity("`Ordering::then`", 2, argc),
            Callee::Consume { method, .. } => {
                let takes = if matches!(method, purecrate_ir::Consume::Count | purecrate_ir::Consume::Sum(_)) { 1 } else { 2 };
                self.arity(&format!("`{}`", method.ts_name()), takes, argc)
            }
            Callee::StrSplit => self.arity("`str::split`", 2, argc),
            Callee::Collect { .. } => {
                if !(1..=2).contains(&argc) {
                    self.arity("`collect`", 2, argc);
                }
            }
            Callee::StringFrom => self.arity("`String::from`", 1, argc),
            Callee::Slice { start, end, .. } => self.arity("slicing", 1 + usize::from(*start) + usize::from(*end), argc),
            Callee::Str(m) => self.arity(&format!("`str::{}`", m.name()), 1 + m.needles(), argc),
            Callee::IntFrom { to, .. } | Callee::CharCode(to) => self.arity(&format!("`{}::from`", to.as_str()), 1, argc),
            Callee::CharFromU8 => self.arity("`char::from`", 1, argc),
            Callee::CharFromU32 => self.arity("`char::from_u32`", 1, argc),
            Callee::Char(m) => self.arity(&format!("`char::{}`", m.name()), 1 + m.args(), argc),
            Callee::UuidParse => self.arity("`Uuid::parse_str`", 1, argc),
            Callee::UuidNil => self.arity("`Uuid::nil`", 0, argc),
            Callee::StrParse(_) => self.arity("`str::parse`", 1, argc),
            Callee::Discriminant { .. } => self.arity("`as`", 1, argc),
        }
    }

    fn construct(&mut self, ty: &Name, variant: Option<&Name>, fields: &Fields, update: bool) {
        let t = ty.as_str();
        match variant {
            None => match self.defs.structs.get(t) {
                Some(s) => {
                    if update && s.fields.iter().any(|f| f.name.as_str() == NEWTYPE_FIELD) {
                        self.error(Reason::StructUpdate, format!(
                            "struct update on newtype `{t}` is not in v0; write `{t}(value)`"
                        ));
                        return;
                    }
                    let declared: Vec<&str> = s.fields.iter().map(|f| f.name.as_str()).collect();
                    self.named_fields(t, &declared, fields, update);
                }
                None => self.error_about(Reason::UndefinedType, t, format!("struct `{t}` is not defined in this crate")),
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
                        self.named_fields(&label, &declared, fields, false);
                    }
                    _ => self.error(Reason::ConstructShape, format!("`{label}` is constructed with the wrong shape")),
                }
            }
        }
    }

    /// Reports and returns `None` when the enum or the variant is unknown.
    fn variant(&mut self, ty: &Name, variant: &Name) -> Option<&'a purecrate_ir::Variant> {
        let t = ty.as_str();
        let Some(e) = self.defs.enums.get(t).copied() else {
            self.error_about(Reason::UndefinedType, t, format!("enum `{t}` is not defined in this crate"));
            return None;
        };
        let found = e.variants.iter().find(|x| x.name == *variant);
        if found.is_none() {
            self.error(Reason::ConstructShape, format!("enum `{t}` has no variant `{}`", variant.as_str()));
        }
        found
    }

    fn named_fields(&mut self, label: &str, declared: &[&str], fields: &Fields, update: bool) {
        let Fields::Named(given) = fields else {
            self.error(Reason::ConstructShape, format!("`{label}` needs named fields"));
            return;
        };
        let given: Vec<&str> = given.iter().map(|(n, _)| n.as_str()).collect();
        let mut seen = Vec::<&str>::new();
        for name in &given {
            if seen.contains(name) {
                self.error(Reason::ConstructShape, format!("field `{name}` of `{label}` is specified more than once"));
            } else {
                seen.push(name);
            }
        }
        let missing: Vec<&str> = declared.iter().copied().filter(|d| !given.contains(d)).collect();
        let extra: Vec<&str> = given.iter().copied().filter(|g| !declared.contains(g)).collect();
        if !missing.is_empty() && !update {
            self.error(Reason::ConstructShape, format!("`{label}` is missing field(s) {}", missing.join(", ")));
        }
        if !extra.is_empty() {
            self.error(Reason::ConstructShape, format!("`{label}` has no field(s) {}", extra.join(", ")));
        }
    }

    fn pattern(&mut self, pattern: &Pattern, bound: &mut Vec<String>) {
        match pattern {
            // Rust matches a const's value here; the IR would bind a name.
            Pattern::Var(n) if self.defs.consts.contains_key(n.as_str()) => self.error_about(
                Reason::UnsupportedPattern,
                n.as_str(),
                format!(
                    "`{}` is a const: matching against a const is not in v0; compare with `==` or write its value",
                    n.as_str()
                ),
            ),
            Pattern::Var(n) => bound.push(n.as_str().to_string()),
            Pattern::Wildcard | Pattern::Lit(_) | Pattern::Range { .. } | Pattern::OptionNone => {}
            Pattern::Or(ps) | Pattern::Tuple(ps) => ps.iter().for_each(|p| self.pattern(p, bound)),
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
                                self.error(Reason::ConstructShape, format!("`{label}` has no field `{}`", field.as_str()));
                            }
                        }
                    }
                    _ => self.error(Reason::ConstructShape, format!("pattern `{label}` does not match the variant's fields")),
                }
            }
        }
    }
}

/// `ty::variant` is a tuple variant of one field: in Rust, a function.
pub(crate) fn one_field_variant(defs: &Defs, ty: &Name, variant: &Name) -> bool {
    defs.enums.get(ty.as_str()).is_some_and(|e| {
        e.variants.iter().any(|v| v.name == *variant && matches!(&v.fields, VariantFields::Tuple(ts) if ts.len() == 1))
    })
}
