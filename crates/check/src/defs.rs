use std::collections::HashMap;

use purecrate_ir::{Alias, Const, Crate, Enum, Fn, Item, Struct};

/// Crate-level definitions by name.
pub struct Defs<'a> {
    pub structs: HashMap<&'a str, &'a Struct>,
    pub enums: HashMap<&'a str, &'a Enum>,
    pub aliases: HashMap<&'a str, &'a Alias>,
    pub free_fns: HashMap<&'a str, &'a Fn>,
    pub methods: HashMap<(&'a str, &'a str), &'a Fn>,
    pub consts: HashMap<&'a str, &'a Const>,
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
                    d.consts.insert(c.name.as_str(), c);
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
        self.structs.get(name).map(|s| s.generics.len()).or_else(|| self.enums.get(name).map(|e| e.generics.len()))
    }

    pub fn is_type(&self, name: &str) -> bool {
        self.structs.contains_key(name) || self.enums.contains_key(name) || self.aliases.contains_key(name)
    }
}
