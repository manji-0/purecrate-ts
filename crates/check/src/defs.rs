use std::collections::HashMap;

use purecrate_ir::{Alias, Crate, Enum, Fn, Item, Struct};

/// Crate-level definitions by name.
pub struct Defs<'a> {
    pub structs: HashMap<&'a str, &'a Struct>,
    pub enums: HashMap<&'a str, &'a Enum>,
    pub aliases: HashMap<&'a str, &'a Alias>,
    pub free_fns: HashMap<&'a str, &'a Fn>,
    pub methods: HashMap<(&'a str, &'a str), &'a Fn>,
}

impl<'a> Defs<'a> {
    pub fn of(krate: &'a Crate) -> Self {
        let mut d = Defs {
            structs: HashMap::new(),
            enums: HashMap::new(),
            aliases: HashMap::new(),
            free_fns: HashMap::new(),
            methods: HashMap::new(),
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

    pub fn is_type(&self, name: &str) -> bool {
        self.structs.contains_key(name)
            || self.enums.contains_key(name)
            || self.aliases.contains_key(name)
    }
}
