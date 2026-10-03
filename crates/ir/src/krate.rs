use std::collections::{BTreeMap, BTreeSet};

use crate::expr::{Callee, Expr};
use crate::item::{Item, Vis};
use crate::name::Name;

/// Closed crate: every Named type used in public fns is an item here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crate {
    pub name: Name,
    pub items: Vec<Item>,
}

impl Crate {
    pub fn new(name: impl Into<String>, items: Vec<Item>) -> Self {
        Self {
            name: Name::new(name),
            items,
        }
    }

    pub fn exported(&self) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(|item| item.vis() == Vis::Pub)
    }

    /// File stems that will be emitted (unique, sorted).
    pub fn file_stems(&self) -> Vec<String> {
        let mut stems: Vec<String> = self.items.iter().map(Item::file_stem).collect();
        stems.sort();
        stems.dedup();
        stems
    }
}

impl Crate {
    /// Where each moved helper is printed: a free function the package does
    /// not export, whose users all sit in one other file, goes into that file
    /// (and is not exported from it), so a reader of the caller finds it
    /// there. A helper of a helper follows it, until no more move. `uses`
    /// gives the free functions an item calls or reads. Where a helper lands
    /// depends only on the source, never on how many files there are.
    pub fn homes(&self, uses: impl Fn(&Item) -> BTreeSet<String>) -> BTreeMap<String, String> {
        /// Beside a free function, so it follows when that one moves too, or
        /// into the file of a type, method, or const, which does not.
        enum Host {
            Fn(String),
            File(String),
        }
        fn stem_of(beside: &BTreeMap<String, Host>, item: &Item) -> String {
            let Item::Fn(f) = item else { return item.file_stem() };
            if f.owner.is_some() {
                return item.file_stem();
            }
            let mut at = f.name.as_str().to_string();
            loop {
                match beside.get(&at) {
                    Some(Host::Fn(host)) => at = host.clone(),
                    Some(Host::File(stem)) => return stem.clone(),
                    None => return Name::new(at).file_stem(),
                }
            }
        }
        let uses: Vec<(&Item, BTreeSet<String>)> = self.items.iter().map(|item| (item, uses(item))).collect();
        let mut beside: BTreeMap<String, Host> = BTreeMap::new();
        loop {
            let mut moved = false;
            for item in &self.items {
                let Item::Fn(f) = item else { continue };
                let name = f.name.as_str();
                if f.owner.is_some() || f.vis == Vis::Pub || beside.contains_key(name) {
                    continue;
                }
                let own = stem_of(&beside, item);
                let users: Vec<&Item> = uses
                    .iter()
                    .filter(|(user, used)| used.contains(name) && stem_of(&beside, user) != own)
                    .map(|(user, _)| *user)
                    .collect();
                let files: BTreeSet<String> = users.iter().map(|u| stem_of(&beside, u)).collect();
                if files.len() != 1 {
                    continue;
                }
                let host = match users[0] {
                    Item::Fn(u) if u.owner.is_none() => Host::Fn(u.name.as_str().to_string()),
                    user => Host::File(stem_of(&beside, user)),
                };
                beside.insert(name.to_string(), host);
                moved = true;
            }
            if !moved {
                break;
            }
        }
        self.items
            .iter()
            .filter_map(|item| match item {
                Item::Fn(f) if f.owner.is_none() && beside.contains_key(f.name.as_str()) => {
                    Some((f.name.as_str().to_string(), stem_of(&beside, item)))
                }
                _ => None,
            })
            .collect()
    }

    /// The file `item` is printed in, given `homes`.
    pub fn home(item: &Item, homes: &BTreeMap<String, String>) -> String {
        match item {
            Item::Fn(f) if f.owner.is_none() => homes.get(f.name.as_str()).cloned().unwrap_or_else(|| item.file_stem()),
            _ => item.file_stem(),
        }
    }

    /// The free functions `item` calls or names, by name alone: exact once
    /// `check` has renamed, since no local then has a helper's name.
    pub fn fns_named(&self, item: &Item) -> BTreeSet<String> {
        fn walk(free: &BTreeSet<&str>, expr: &Expr, out: &mut BTreeSet<String>) {
            match expr {
                Expr::Call { callee: Callee::Fn(n), .. } | Expr::Var(n) if free.contains(n.as_str()) => {
                    out.insert(n.as_str().to_string());
                }
                _ => {}
            }
            expr.children().into_iter().for_each(|c| walk(free, c, out));
        }
        let free: BTreeSet<&str> = self
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Fn(f) if f.owner.is_none() => Some(f.name.as_str()),
                _ => None,
            })
            .collect();
        let mut out = BTreeSet::new();
        match item {
            Item::Fn(f) => walk(&free, &f.body, &mut out),
            Item::Const(c) => walk(&free, &c.value, &mut out),
            _ => {}
        }
        out
    }
}
