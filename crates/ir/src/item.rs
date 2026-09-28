use crate::expr::Expr;
use crate::name::Name;
use crate::ty::Ty;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Vis {
    /// `pub` — exported from the TS package.
    Pub,
    /// Reachable from the public surface but not exported.
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: Name,
    pub ty: Ty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VariantFields {
    Unit,
    Tuple(Vec<Ty>),
    Struct(Vec<Field>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    pub name: Name,
    pub fields: VariantFields,
}

/// Field name of a newtype's single element (`struct Id(u32)` → `id.0`).
/// Named fields cannot start with a digit, so this marks the shape.
pub const NEWTYPE_FIELD: &str = "0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Struct {
    pub vis: Vis,
    pub name: Name,
    pub fields: Vec<Field>,
    /// Some field is not `pub`. Outside the crate Rust builds the value only
    /// through the crate's functions, so the TS type is branded and its
    /// companion has no `of` (design/01 §4).
    pub closed: bool,
}

impl Struct {
    /// The wrapped type of a one-element tuple struct.
    pub fn newtype_inner(&self) -> Option<&Ty> {
        match self.fields.as_slice() {
            [f] if f.name.as_str() == NEWTYPE_FIELD => Some(&f.ty),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enum {
    pub vis: Vis,
    pub name: Name,
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    pub vis: Vis,
    pub name: Name,
    pub ty: Ty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Name,
    pub ty: Ty,
}

/// Free function, or a method folded onto a companion (`owner`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fn {
    pub vis: Vis,
    pub name: Name,
    pub owner: Option<Name>,
    pub params: Vec<Param>,
    pub ret: Ty,
    pub body: Expr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Struct(Struct),
    Enum(Enum),
    Alias(Alias),
    Fn(Fn),
}

impl Item {
    pub fn name(&self) -> &Name {
        match self {
            Item::Struct(s) => &s.name,
            Item::Enum(e) => &e.name,
            Item::Alias(a) => &a.name,
            Item::Fn(f) => &f.name,
        }
    }

    pub fn vis(&self) -> Vis {
        match self {
            Item::Struct(s) => s.vis,
            Item::Enum(e) => e.vis,
            Item::Alias(a) => a.vis,
            Item::Fn(f) => f.vis,
        }
    }

    /// File that owns this concept after flatten.
    /// Methods live in the owner's file.
    pub fn file_stem(&self) -> String {
        match self {
            Item::Fn(f) => match &f.owner {
                Some(owner) => owner.file_stem(),
                None => f.name.file_stem(),
            },
            other => other.name().file_stem(),
        }
    }
}
