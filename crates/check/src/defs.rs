use std::collections::HashMap;

use purecrate_ir::{Alias, Const, Crate, Enum, Fn, Item, Name, Struct, Trait, TraitMethod, Ty, Variant, VariantFields};

/// A std trait of `STD_DICT_TRAITS` as its record (`purecrate_ir::std_trait`).
fn std_trait(name: &str) -> Option<&'static Trait> {
    static STD: std::sync::LazyLock<Vec<Trait>> = std::sync::LazyLock::new(|| {
        purecrate_ir::STD_DICT_TRAITS.iter().filter_map(|n| purecrate_ir::std_trait(n)).collect()
    });
    STD.iter().find(|t| t.name.as_str() == name)
}

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
    /// The crate's traits (design/01 §7.25).
    pub traits: HashMap<&'a str, &'a Trait>,
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
            traits: HashMap::new(),
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
                Item::Trait(t) => {
                    d.traits.insert(t.name.as_str(), t);
                }
            }
        }
        // A default the impl does not write is the type's method too: the
        // trait's body, its `Self` the type's (`X.m` calls `Tr.m(X, ..)`).
        let traits: Vec<&'a Trait> = d.traits.values().copied().collect();
        for t in traits {
            for imp in &t.impls {
                for m in t.methods.iter().filter(|m| m.default) {
                    let key = (imp.owner().as_str(), m.name.as_str());
                    if !d.methods.contains_key(&key) {
                        if let Some(f) = d.methods.get(&(t.name.as_str(), m.name.as_str())).copied() {
                            d.methods.insert(key, f);
                        }
                    }
                }
            }
        }
        d
    }

    /// The trait and the dictionary's type of a dictionary parameter's
    /// type, `Priced<T>`: a crate trait's, or one of `STD_DICT_TRAITS`.
    pub fn dict_of<'t>(&self, ty: &'t Ty) -> Option<(&'t Name, &'t Ty)> {
        match ty {
            Ty::App { name, args } if args.len() == 1 => {
                let named =
                    self.traits.contains_key(name.as_str()) || purecrate_ir::STD_DICT_TRAITS.contains(&name.as_str());
                named.then(|| (name, &args[0]))
            }
            _ => None,
        }
    }

    /// Whether the bound `name` is std's (`STD_DICT_TRAITS`), not hidden by
    /// a trait of the crate.
    pub fn is_std_trait(&self, name: &str) -> bool {
        !self.traits.contains_key(name) && std_trait(name).is_some()
    }

    /// The parameters a call writes: `f`'s, less its dictionaries.
    pub fn written_params<'f>(&self, f: &'f Fn) -> Vec<&'f purecrate_ir::Param> {
        f.params.iter().filter(|p| self.dict_of(&p.ty).is_none()).collect()
    }

    /// `trait_` and its supertraits, each once, the trait first.
    pub fn trait_and_supers(&self, trait_: &str) -> Vec<&'a Trait> {
        let mut out: Vec<&'a Trait> = Vec::new();
        let mut todo = vec![trait_.to_string()];
        while let Some(name) = todo.pop() {
            let Some(t) = self.traits.get(name.as_str()).copied().or_else(|| std_trait(&name)) else { continue };
            if out.iter().any(|o| o.name == t.name) {
                continue;
            }
            out.push(t);
            todo.extend(t.supertraits.iter().rev().map(|s| s.as_str().to_string()));
        }
        out
    }

    /// The method `name` of `trait_` or one of its supertraits.
    pub fn trait_method(&self, trait_: &str, name: &str) -> Option<&'a TraitMethod> {
        self.trait_and_supers(trait_).into_iter().find_map(|t| t.methods.iter().find(|m| m.name.as_str() == name))
    }

    /// The const `name` of `trait_` or one of its supertraits.
    pub fn trait_const(&self, trait_: &str, name: &str) -> Option<&'a purecrate_ir::TraitConst> {
        self.trait_and_supers(trait_).into_iter().find_map(|t| t.consts.iter().find(|c| c.name.as_str() == name))
    }

    /// `sub` is `sup` or has it among its supertraits.
    pub fn extends(&self, sub: &str, sup: &str) -> bool {
        self.trait_and_supers(sub).iter().any(|t| t.name.as_str() == sup)
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
    pub fn conversions_into(&self, ty: &str) -> impl Iterator<Item = (&'a Fn, &'a Ty)> + '_ {
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
