//! Where an argument of a `pub` function holds an array `[T; N]`, and its
//! length: the shape `Slice.lengths` walks on entry (design/01 §7.27). A
//! TS caller may pass an array of any length where the type says `N`, as
//! it may pass a lone surrogate where it says `String` (`expr::checked_strings`),
//! so the length is checked wherever in the value an array is: a field, a
//! variant's payload, an element, an `Option`, a tuple, a map's value.
//!
//! A shape is `N` (an array of `N` elements, nothing inside to check), or
//! an object: `len` (an array's length), `each` (every element, or a map's
//! every value), `at` (a field or a tuple's element, by key), and `kinds`
//! (a variant's fields, by `kind`). A recursive type is a string naming
//! its entry in a table passed after the parameter's name. A closed type's
//! arrays came through its own `pub` functions, checked there, and a type
//! with no array is not walked.

use super::*;

thread_local! {
    /// The crate's open structs, enums, and aliases, by name, as the shapes
    /// read them; `emit` sets it for its duration.
    static DEFS: RefCell<BTreeMap<String, Def>> = const { RefCell::new(BTreeMap::new()) };
}

/// A type of the crate as a shape reads it: its parameters, and where its
/// values keep what they hold.
#[derive(Clone)]
pub(crate) enum Def {
    /// A struct's fields by key; a newtype's content is the value itself.
    Struct { generics: Vec<Name>, fields: Vec<(String, Ty)>, newtype: bool },
    /// A closed struct, never walked.
    Closed,
    /// Each variant's fields by key, by `kind`.
    Enum { generics: Vec<Name>, variants: Vec<(String, Vec<(String, Ty)>)> },
    Alias(Ty),
}

pub(crate) fn defs_of(krate: &Crate) -> BTreeMap<String, Def> {
    let mut out = BTreeMap::new();
    for item in &krate.items {
        let def = match item {
            Item::Struct(st) if st.closed => Def::Closed,
            Item::Struct(st) => Def::Struct {
                generics: st.generics.clone(),
                fields: st.fields.iter().map(|f| (f.name.as_str().to_string(), f.ty.clone())).collect(),
                newtype: st.newtype_inner().is_some(),
            },
            Item::Enum(e) => Def::Enum {
                generics: e.generics.clone(),
                variants: e
                    .variants
                    .iter()
                    .map(|v| {
                        let fields = match &v.fields {
                            VariantFields::Unit => Vec::new(),
                            VariantFields::Tuple(ts) if ts.len() == 1 => vec![("value".to_string(), ts[0].clone())],
                            VariantFields::Tuple(ts) => vec![("content".to_string(), Ty::Tuple(ts.clone()))],
                            VariantFields::Struct(fs) => {
                                fs.iter().map(|f| (f.name.as_str().to_string(), f.ty.clone())).collect()
                            }
                        };
                        (v.name.as_str().to_string(), fields)
                    })
                    .collect(),
            },
            Item::Alias(a) => Def::Alias(a.ty.clone()),
            _ => continue,
        };
        out.insert(item.name().as_str().to_string(), def);
    }
    out
}

/// `f` with the definitions of `krate` in scope for `shape`.
pub(crate) fn with_defs<R>(krate: &Crate, f: impl FnOnce() -> R) -> R {
    scoped(&DEFS, defs_of(krate), f)
}

enum Shape {
    Len(usize),
    /// A recursive type's entry in the table.
    Ref(String),
    Node { len: Option<usize>, each: Option<Box<Shape>>, at: Vec<(String, Shape)>, kinds: Vec<(String, Vec<(String, Shape)>)> },
}

/// The shape of `ty`, and the table of the recursive types it names,
/// printed; `None` when no array is inside.
pub(crate) fn shape(ty: &Ty) -> Option<(String, Option<String>)> {
    let mut walk = Walk { stack: Vec::new(), table: Vec::new(), recursive: BTreeSet::new() };
    let s = walk.of(ty)?;
    let table = (!walk.table.is_empty()).then(|| {
        let entries: Vec<String> = walk.table.iter().map(|(k, s)| format!("{}: {}", key(k), print(s))).collect();
        format!("{{ {} }}", entries.join(", "))
    });
    Some((print(&s), table))
}

struct Walk {
    /// The named types being walked, by their TS text.
    stack: Vec<String>,
    table: Vec<(String, Shape)>,
    /// Those met again while being walked.
    recursive: BTreeSet<String>,
}

/// Whether a value of `ty` may hold an array a TS caller wrote.
fn holds_array(ty: &Ty, seen: &mut BTreeSet<String>) -> bool {
    match ty {
        Ty::Array { .. } => true,
        Ty::Named(n) | Ty::App { name: n, .. } => {
            let args = ty.children().into_iter().any(|t| holds_array(t, seen));
            if args || !seen.insert(n.as_str().to_string()) {
                return args;
            }
            match DEFS.with(|d| d.borrow().get(n.as_str()).cloned()) {
                Some(Def::Alias(t)) => holds_array(&t, seen),
                Some(Def::Struct { fields, .. }) => fields.iter().any(|(_, t)| holds_array(t, seen)),
                Some(Def::Enum { variants, .. }) => {
                    variants.iter().any(|(_, fs)| fs.iter().any(|(_, t)| holds_array(t, seen)))
                }
                Some(Def::Closed) | None => false,
            }
        }
        Ty::Fn { .. } => false,
        t => t.children().into_iter().any(|t| holds_array(t, seen)),
    }
}

impl Walk {
    fn of(&mut self, ty: &Ty) -> Option<Shape> {
        if !holds_array(ty, &mut BTreeSet::new()) {
            return None;
        }
        match ty {
            Ty::Array { elem, len } => Some(match self.of(elem) {
                None => Shape::Len(*len),
                Some(inner) => {
                    Shape::Node { len: Some(*len), each: Some(Box::new(inner)), at: Vec::new(), kinds: Vec::new() }
                }
            }),
            Ty::Vec(t) | Ty::Map { value: t, .. } => {
                let inner = self.of(t)?;
                Some(Shape::Node { len: None, each: Some(Box::new(inner)), at: Vec::new(), kinds: Vec::new() })
            }
            // `None` is `null`, which the walk passes.
            Ty::Option(t) | Ty::Ignored { inner: t, .. } => self.of(t),
            Ty::Tuple(ts) => {
                let at: Vec<(String, Shape)> =
                    ts.iter().enumerate().filter_map(|(i, t)| Some((i.to_string(), self.of(t)?))).collect();
                node_at(at)
            }
            Ty::Result { ok, err } => {
                let kinds: Vec<(String, Vec<(String, Shape)>)> = [("Ok", "value", ok), ("Err", "error", err)]
                    .into_iter()
                    .filter_map(|(k, f, t)| Some((k.to_string(), vec![(f.to_string(), self.of(t)?)])))
                    .collect();
                (!kinds.is_empty()).then_some(Shape::Node { len: None, each: None, at: Vec::new(), kinds })
            }
            Ty::Named(n) => self.named(n, &[], ty),
            Ty::App { name, args } => self.named(name, args, ty),
            Ty::Prim(_) | Ty::Set(_) | Ty::Param(_) | Ty::Fn { .. } | Ty::Never => None,
        }
    }

    fn named(&mut self, name: &Name, args: &[Ty], ty: &Ty) -> Option<Shape> {
        let def = DEFS.with(|d| d.borrow().get(name.as_str()).cloned())?;
        let text = emit_ty(ty);
        if self.stack.contains(&text) {
            self.recursive.insert(text.clone());
            return Some(Shape::Ref(text));
        }
        self.stack.push(text.clone());
        let bind = |generics: &[Name], t: &Ty| {
            let binds: Vec<(Name, Ty)> = generics.iter().cloned().zip(args.iter().cloned()).collect();
            t.subst(&binds)
        };
        let found = match def {
            Def::Closed => None,
            Def::Alias(t) => self.of(&t),
            Def::Struct { generics, fields, newtype: true } => self.of(&bind(&generics, &fields[0].1)),
            Def::Struct { generics, fields, newtype: false } => {
                let at = fields.iter().filter_map(|(k, t)| Some((k.clone(), self.of(&bind(&generics, t))?))).collect();
                node_at(at)
            }
            Def::Enum { generics, variants } => {
                let kinds: Vec<(String, Vec<(String, Shape)>)> = variants
                    .iter()
                    .filter_map(|(kind, fields)| {
                        let at: Vec<(String, Shape)> = fields
                            .iter()
                            .filter_map(|(k, t)| Some((k.clone(), self.of(&bind(&generics, t))?)))
                            .collect();
                        (!at.is_empty()).then(|| (kind.clone(), at))
                    })
                    .collect();
                (!kinds.is_empty()).then_some(Shape::Node { len: None, each: None, at: Vec::new(), kinds })
            }
        };
        self.stack.pop();
        let found = found?;
        if self.recursive.contains(&text) {
            self.table.push((text.clone(), found));
            return Some(Shape::Ref(text));
        }
        Some(found)
    }
}

fn node_at(at: Vec<(String, Shape)>) -> Option<Shape> {
    (!at.is_empty()).then_some(Shape::Node { len: None, each: None, at, kinds: Vec::new() })
}

/// An object literal's key: a computed one for `__proto__`, which a plain
/// or quoted key would set as the prototype.
fn key(k: &str) -> String {
    if k == purecrate_ir::PROTO_KEY {
        format!("[{k:?}]")
    } else if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        && !k.starts_with(|c: char| c.is_ascii_digit())
        || k.chars().all(|c| c.is_ascii_digit())
    {
        k.to_string()
    } else {
        format!("{k:?}")
    }
}

fn print(s: &Shape) -> String {
    match s {
        Shape::Len(n) => n.to_string(),
        Shape::Ref(name) => format!("{name:?}"),
        Shape::Node { len, each, at, kinds } => {
            let fields = |fs: &[(String, Shape)]| {
                fs.iter().map(|(k, s)| format!("{}: {}", key(k), print(s))).collect::<Vec<_>>().join(", ")
            };
            let mut parts = Vec::new();
            if let Some(n) = len {
                parts.push(format!("len: {n}"));
            }
            if let Some(e) = each {
                parts.push(format!("each: {}", print(e)));
            }
            if !at.is_empty() {
                parts.push(format!("at: {{ {} }}", fields(at)));
            }
            if !kinds.is_empty() {
                let ks: Vec<String> = kinds.iter().map(|(k, fs)| format!("{}: {{ {} }}", key(k), fields(fs))).collect();
                parts.push(format!("kinds: {{ {} }}", ks.join(", ")));
            }
            format!("{{ {} }}", parts.join(", "))
        }
    }
}
