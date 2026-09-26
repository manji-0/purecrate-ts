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
