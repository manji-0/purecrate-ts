use crate::expr::Expr;
use crate::name::{upper_first, Name};
use crate::ty::{Prim, Ty};

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
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
    /// The JSON key, where `#[serde(rename)]` or a `rename_all` makes it
    /// other than the name (design/04 §3.6).
    pub wire: Option<String>,
}

impl Field {
    /// The key serde reads and writes the field under.
    pub fn wire_name(&self) -> &str {
        self.wire.as_deref().unwrap_or(self.name.as_str())
    }
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
    /// `A = 1 << 3` on a fieldless enum: a const expression, folded to an
    /// integer `Lit` by `check::accept`.
    pub discriminant: Option<Expr>,
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
    /// The name serde reads and writes the variant under, where
    /// `#[serde(rename)]` or the enum's `rename_all` makes it other than the
    /// Rust name (design/04 §3.6). The TS `kind` stays the Rust name.
    pub wire: Option<String>,
}

impl Variant {
    pub fn wire_name(&self) -> &str {
        self.wire.as_deref().unwrap_or(self.name.as_str())
    }
}

/// How serde writes an enum (design/04 §3.6): `{"V": ..}` by default,
/// `#[serde(tag = "t")]` puts the variant name in the object beside the
/// fields, and `#[serde(tag = "t", content = "c")]` beside the content.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Tagging {
    #[default]
    External,
    Internal {
        tag: String,
    },
    Adjacent {
        tag: String,
        content: String,
    },
}

/// Field name of a newtype's single element (`struct Id(u32)` → `id.0`).
/// Named fields cannot start with a digit, so this marks the shape.
pub const NEWTYPE_FIELD: &str = "0";

/// Field name of a tuple's element `i` in `Expr::Field`, printed `t[i]`.
/// Only `check::accept` writes one, when it splits a `match` on a tuple
/// value (the source cannot write `t.0` on a tuple).
pub fn tuple_field(i: usize) -> crate::Name {
    crate::Name::new(format!("[{i}]"))
}

/// Which serde derives a struct or enum has: `#[derive(Serialize)]` and
/// `#[derive(Deserialize)]`, by the last segment of the path. Only these
/// types have a wire form (design/04 §3.2): a schema reads a type that
/// derives `Deserialize`, and `toJson` writes one that derives `Serialize`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Serde {
    pub ser: bool,
    pub de: bool,
}

impl Serde {
    pub fn any(self) -> bool {
        self.ser || self.de
    }
}

/// What a struct's or enum's `#[derive(..)]` says of its order (design/01
/// §7.25). `check::accept` gives a type that derives `Ord` the method `cmp`,
/// comparing its fields (an enum's variants first) as the derive does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    None,
    /// `PartialOrd` without `Ord`: ordering the type is refused, as `<` on a
    /// float is (a type with a float field derives no more).
    Partial,
    /// `Ord`, with `PartialOrd` derived or written as `Some(self.cmp(other))`.
    Total,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Struct {
    pub vis: Vis,
    pub name: Name,
    /// Type parameters (`Page<T>`), each `Ty::Param` in the fields.
    pub generics: Vec<Name>,
    pub fields: Vec<Field>,
    /// Some field is not `pub`. Outside the crate Rust builds the value only
    /// through the crate's functions, so the TS type is branded and its
    /// companion has no `of` (design/01 §4).
    pub closed: bool,
    /// `#[serde(try_from = "T")]`: the wire form is `T`'s, read and then
    /// checked by `impl TryFrom<T> for Self` (design/04 §5).
    pub wire_from: Option<Ty>,
    pub serde: Serde,
    pub order: Order,
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
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
    /// Type parameters, each `Ty::Param` in the variants.
    pub generics: Vec<Name>,
    pub variants: Vec<Variant>,
    /// `#[repr(u8)]` and the like: the type of the discriminants, `isize`
    /// without one (range checked as `i64`).
    pub repr: Option<crate::ty::IntTy>,
    /// std's own enum, not the crate's: `std::cmp::Ordering`, which the
    /// parser adds when the crate names it (`Enum::std_ordering`). It prints
    /// as a crate enum, but has no wire form (serde implements neither
    /// `Serialize` nor `Deserialize` for it) and rustc sees std's type.
    pub std: bool,
    pub serde: Serde,
    pub order: Order,
    pub tagging: Tagging,
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
}

/// The name `std::cmp::Ordering` is reached by in the IR.
pub const ORDERING: &str = "Ordering";

impl Enum {
    /// `std::cmp::Ordering`: `Less`, `Equal`, `Greater`, which std declares
    /// `#[repr(i8)]` as -1, 0, and 1, so `o as i32` reads those.
    pub fn std_ordering(vis: Vis) -> Self {
        let variant = |name: &str, d: i128| Variant {
            name: Name::new(name),
            fields: VariantFields::Unit,
            discriminant: Some(Expr::Lit(crate::expr::Lit::Int { value: d, ty: None, byte: false, hex: false })),
            doc: None,
            wire: None,
        };
        Enum {
            generics: Vec::new(),
            vis,
            name: Name::new(ORDERING),
            variants: vec![variant("Less", -1), variant("Equal", 0), variant("Greater", 1)],
            repr: Some(crate::ty::IntTy::I8),
            std: true,
            serde: Serde::default(),
            order: Order::None,
            tagging: Tagging::External,
            doc: None,
        }
    }
}

/// The one file that holds every `const` of a crate.
pub const CONSTS_STEM: &str = "consts";

/// `const NAME: T = expr;` at crate level, or in an inherent `impl` block
/// (`owner`). `check::accept` folds `value` to a `Lit` of `ty`: rustc
/// evaluates it at compile time, so the TS holds the result, not the
/// computation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Const {
    pub vis: Vis,
    pub name: Name,
    /// The type of `impl T { const N: u32 = 3; }`: the const is a member of
    /// `T`'s companion, read as `Expr::Var` of its `path` (`T::N`).
    pub owner: Option<Name>,
    pub ty: Ty,
    pub value: Expr,
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
    /// The `//` lines directly above the item, each without its slashes,
    /// printed above it as `//` lines.
    pub comment: Vec<String>,
}

impl Const {
    /// The name an `Expr::Var` reads it by: `N`, or `T::N` for an associated
    /// const.
    pub fn path(&self) -> String {
        match &self.owner {
            Some(owner) => assoc_path(owner.as_str(), self.name.as_str()),
            None => self.name.as_str().to_string(),
        }
    }
}

/// `T::N`, the name an associated const is read by.
pub fn assoc_path(owner: &str, name: &str) -> String {
    format!("{owner}::{name}")
}

/// The owner and the name of an associated const's path (`T::N`); `None`
/// for any other name.
pub fn split_assoc_path(path: &str) -> Option<(&str, &str)> {
    path.split_once("::")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    pub vis: Vis,
    pub name: Name,
    pub ty: Ty,
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
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
    /// Type parameters: the owner's (`impl<T> Page<T>`), then the function's
    /// own, each `Ty::Param` in the signature and body.
    pub generics: Vec<Name>,
    pub params: Vec<Param>,
    pub ret: Ty,
    pub body: Expr,
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
    /// `to_string` read from an `impl Display`'s `fmt` (design/01 §7.25):
    /// where its body is outside the subset (it prints a float, say), the
    /// method is dropped and the crate kept, as a `fmt` of another shape is
    /// skipped, rather than refused.
    pub display: bool,
}

impl Fn {
    /// The source type `S` when this is the member `impl From<S> for X`
    /// gives `X`'s companion (`from_member`): one parameter of a type the
    /// member is named after, returning `X`. rustc refuses an `X::from(s)`
    /// or a `?` that no `impl` takes, so an inherent method spelled the same
    /// way is never called in its place.
    pub fn from_source(&self) -> Option<&Ty> {
        let owner = self.owner.as_ref()?;
        let [p] = self.params.as_slice() else { return None };
        let returns_owner = matches!(&self.ret, Ty::Named(n) if n == owner);
        (returns_owner && from_member(&p.ty).as_deref() == Some(self.name.as_str())).then_some(&p.ty)
    }
}

/// The companion member `impl From<S> for X` gives `X`, named after `S` in
/// camel case (design/01 §7.25): `fromU32`, `fromString`,
/// `fromParseIntError`, `fromUuidError` (`uuid::Error`), `fromMyError`.
/// `None` for a source with no such name: an `Option`, a tuple, a reference
/// (`&str`), `()`, or a type with arguments (`Vec<u8>`).
pub fn from_member(source: &Ty) -> Option<String> {
    let name = match source {
        Ty::Prim(Prim::Str | Prim::Unit) => return None,
        Ty::Prim(Prim::Bool) => "Bool".to_string(),
        Ty::Prim(Prim::Char) => "Char".to_string(),
        Ty::Prim(Prim::String) => "String".to_string(),
        Ty::Prim(Prim::Uuid) => "Uuid".to_string(),
        Ty::Prim(Prim::UuidError) => "UuidError".to_string(),
        Ty::Prim(Prim::ParseIntError) => "ParseIntError".to_string(),
        Ty::Prim(p) => match (p.int(), p.float()) {
            (Some(t), _) => t.ts_name().to_string(),
            (_, Some(t)) => t.ts_name().to_string(),
            _ => return None,
        },
        Ty::Named(n) => upper_first(n.as_str()),
        _ => return None,
    };
    Some(format!("from{name}"))
}

/// `trait Tr: Base { .. }` (design/01 §7.25): the type of a record of
/// functions over `Self`. Its default bodies are `Fn`s owned by the trait,
/// each taking the dictionary first (`DICT_SELF`); its implementations'
/// methods are `Fn`s owned by the implementing type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trait {
    pub vis: Vis,
    pub name: Name,
    /// The crate traits written after `:`, whose members the record holds too.
    pub supertraits: Vec<Name>,
    pub methods: Vec<TraitMethod>,
    pub consts: Vec<TraitConst>,
    /// Each `impl Tr for X` in the crate, in source order.
    pub impls: Vec<TraitImpl>,
    /// The item's `///` comment, without the slashes, as JSDoc in the TS.
    pub doc: Option<String>,
}

/// A trait method's signature, `Self` as `Ty::Param(SELF)`: `self` is the
/// first parameter where it takes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitMethod {
    pub name: Name,
    pub params: Vec<Param>,
    pub ret: Ty,
    /// It has a body, printed in the trait's companion.
    pub default: bool,
    pub doc: Option<String>,
}

impl TraitMethod {
    /// Takes `self` (or `&self`).
    pub fn has_receiver(&self) -> bool {
        self.params.first().is_some_and(|p| p.name.as_str() == "self")
    }
}

/// `const N: u32;` in a trait, with or without a default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitConst {
    pub name: Name,
    pub ty: Ty,
    pub doc: Option<String>,
}

/// `impl<..> Tr for X<..>`: the implementing type as written (its
/// parameters `Ty::Param`), and the impl's own type parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitImpl {
    pub ty: Ty,
    pub generics: Vec<Name>,
}

impl TraitImpl {
    /// The implementing struct or enum.
    pub fn owner(&self) -> &Name {
        self.ty.applied().map(|(n, _)| n).expect("an impl is on a struct or enum of the crate")
    }
}

/// The type parameter a trait's own items are over.
pub const SELF: &str = "Self";

/// The dictionary parameter of a trait's default body, its first.
pub const DICT_SELF: &str = "dict";

/// The std traits whose bound carries a dictionary, as every crate trait's
/// does (design/01 §7.25). `Clone`, `Copy`, `PartialEq`, `Eq`, and `Debug`
/// stay structural, with none.
pub const STD_DICT_TRAITS: &[&str] = &[];

/// The parameter carrying the dictionary of `trait_` for the type parameter
/// `param`: `pricedT`, after the function's other parameters.
pub fn dict_param(trait_: &str, param: &str) -> Name {
    Name::new(format!("{}{param}", crate::lower_first(trait_)))
}

/// The type of a dictionary parameter: `Priced<T>`.
pub fn dict_ty(trait_: &Name, of: Ty) -> Ty {
    Ty::App { name: trait_.clone(), args: vec![of] }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Struct(Struct),
    Enum(Enum),
    Alias(Alias),
    Fn(Fn),
    Const(Const),
    Trait(Trait),
}

impl Item {
    pub fn name(&self) -> &Name {
        match self {
            Item::Struct(s) => &s.name,
            Item::Enum(e) => &e.name,
            Item::Alias(a) => &a.name,
            Item::Fn(f) => &f.name,
            Item::Const(c) => &c.name,
            Item::Trait(t) => &t.name,
        }
    }

    pub fn vis(&self) -> Vis {
        match self {
            Item::Struct(s) => s.vis,
            Item::Enum(e) => e.vis,
            Item::Alias(a) => a.vis,
            Item::Fn(f) => f.vis,
            Item::Const(c) => c.vis,
            Item::Trait(t) => t.vis,
        }
    }

    /// The type a method or an associated const belongs to.
    pub fn owner(&self) -> Option<&Name> {
        match self {
            Item::Fn(f) => f.owner.as_ref(),
            Item::Const(c) => c.owner.as_ref(),
            _ => None,
        }
    }

    /// File that owns this concept after flatten.
    /// Methods and associated consts live in the owner's file; every other
    /// const lives in `consts.ts`, so `MAX_LEN` and `fn max_len` do not meet.
    pub fn file_stem(&self) -> String {
        match self {
            Item::Const(c) => match &c.owner {
                Some(owner) => owner.file_stem(),
                None => CONSTS_STEM.to_string(),
            },
            Item::Fn(f) => match &f.owner {
                Some(owner) => owner.file_stem(),
                None => f.name.file_stem(),
            },
            other => other.name().file_stem(),
        }
    }
}
