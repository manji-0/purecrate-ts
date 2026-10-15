use std::collections::HashMap;

use purecrate_ir::{Alias, Const, Crate, Enum, Fn, Item, Name, Struct, Ty, Variant, VariantFields};

/// Where `Defs::expand` looks through `Box` and the other erased wrappers.
#[derive(Clone, Copy)]
pub enum Peel {
    Never,
    /// The type as written, not what an alias names.
    First,
    /// The type as written and what each alias names.
    Every,
}

/// Crate-level definitions by name.
pub struct Defs<'a> {
    pub structs: HashMap<&'a str, &'a Struct>,
    pub enums: HashMap<&'a str, &'a Enum>,
    pub aliases: HashMap<&'a str, &'a Alias>,
    pub free_fns: HashMap<&'a str, &'a Fn>,
    pub methods: HashMap<(&'a str, &'a str), &'a Fn>,
    /// By `Const::path`: `N`, or `T::N` for an associated const.
    pub consts: HashMap<String, &'a Const>,
}

impl<'a> Defs<'a> {
    pub fn of(krate: &'a Crate) -> Self {
        let mut d = Defs {
            structs: HashMap::new(),
            enums: HashMap::new(),
            aliases: HashMap::new(),
            free_fns: HashMap::new(),
            methods: HashMap::new(),
            consts: HashMap::new(),
        };
        for item in &krate.items {
            match item {
                Item::Struct(s) => {
                    d.structs.insert(s.name.as_str(), s);
                }
                Item::Enum(e) => {
                    d.enums.insert(e.name.as_str(), e);
                }
                Item::Alias(a) => {
                    d.aliases.insert(a.name.as_str(), a);
                }
                Item::Const(c) => {
                    d.consts.insert(c.path(), c);
                }
                Item::Fn(f) => match &f.owner {
                    Some(o) => {
                        d.methods.insert((o.as_str(), f.name.as_str()), f);
                    }
                    None => {
                        d.free_fns.insert(f.name.as_str(), f);
                    }
                },
            }
        }
        d
    }

    /// How many type parameters a struct or enum has; `None` when `name` is
    /// neither.
    pub fn generics_of(&self, name: &str) -> Option<usize> {
        self.type_params(name).map(<[_]>::len)
    }

    /// A struct's or enum's type parameters; `None` when `name` is neither.
    pub fn type_params(&self, name: &str) -> Option<&'a [Name]> {
        self.structs
            .get(name)
            .map(|s| s.generics.as_slice())
            .or_else(|| self.enums.get(name).map(|e| e.generics.as_slice()))
    }

    /// The variant `variant` of the enum `ty`, when both exist.
    pub fn variant(&self, ty: &str, variant: &str) -> Option<&'a Variant> {
        self.enums.get(ty).and_then(|e| e.variants.iter().find(|v| v.name.as_str() == variant))
    }

    /// The declared fields of a struct, or of a variant: each name (none in
    /// a tuple variant) and type, its parameters unbound.
    pub fn fields_of(&self, ty: &str, variant: Option<&str>) -> Vec<(Option<&'a Name>, &'a Ty)> {
        match variant {
            None => self
                .structs
                .get(ty)
                .map(|s| s.fields.iter().map(|f| (Some(&f.name), &f.ty)).collect())
                .unwrap_or_default(),
            Some(v) => match self.variant(ty, v).map(|x| &x.fields) {
                Some(VariantFields::Tuple(tys)) => tys.iter().map(|t| (None, t)).collect(),
                Some(VariantFields::Struct(fs)) => fs.iter().map(|f| (Some(&f.name), &f.ty)).collect(),
                _ => Vec::new(),
            },
        }
    }

    /// `ty` with its aliases followed, `Box` and the other erased wrappers
    /// looked through where `peel` says. Alias cycles are cut after a fixed
    /// depth.
    pub fn expand<'t>(&'t self, ty: &'t Ty, peel: Peel) -> &'t Ty {
        let mut t = if matches!(peel, Peel::Never) { ty } else { ty.peel() };
        for _ in 0..32 {
            match t {
                Ty::Named(n) => match self.aliases.get(n.as_str()) {
                    Some(a) if matches!(peel, Peel::Every) => t = a.ty.peel(),
                    Some(a) => t = &a.ty,
                    None => return t,
                },
                _ => return t,
            }
        }
        t
    }

    /// The members `impl From<S> for ty` gives `ty`, each with its `S`.
    pub fn from_members(&self, ty: &str) -> impl Iterator<Item = (&'a Fn, &'a Ty)> + '_ {
        let ty = ty.to_string();
        self.methods
            .iter()
            .filter(move |((owner, _), _)| *owner == ty)
            .filter_map(|(_, f)| f.from_source().map(|s| (*f, s)))
    }

    pub fn is_type(&self, name: &str) -> bool {
        self.structs.contains_key(name) || self.enums.contains_key(name) || self.aliases.contains_key(name)
    }
}
