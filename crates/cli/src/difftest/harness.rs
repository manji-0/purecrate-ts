//! The Rust half of `test`: the crate's own files, each with what it needs
//! appended, compiled as one binary that draws the arguments, calls every
//! function under test, and prints one line per call.
//!
//! What a file gets is written where the item it is about is declared, so
//! private fields and the names the file has in scope are all it uses:
//!
//! - every struct and enum declared in it gets `Show` (the canonical text,
//!   as `purecrate_canon` gives the differential tests) and, where values
//!   of it can be drawn, `Arb`: an open type field by field, an enum
//!   variant by variant, a closed type through its public constructors
//!   (an associated function returning it, or `Result` / `Option` of it),
//!   since a value built field by field could break what its constructor
//!   checks;
//! - every function under test declared in it gets a case: its arguments
//!   drawn by `Arb`, whose types rustc infers from the call;
//! - `__purecrate_register`, which adds its cases and calls its child
//!   modules' (a parent sees its children's private modules).
//!
//! `Arb` gives each value with its TS spelling: a literal for an open
//! value, and for a closed one the constructor call that made it on the
//! Rust side (`__ok(pkg.Email.parse("a@b"))`), so the TS side makes it the
//! same way.

use std::collections::BTreeSet;

use purecrate_ir::{Crate, Enum, Fn, Item, Name, Prim, Struct, Ty, VariantFields, Vis, NEWTYPE_FIELD};

use super::derive;

/// A function under test.
pub struct Call {
    /// What names the call, on both sides: the TS function (`pkg.next`,
    /// `pkg.Ulid.parse`), and for a function with a bound that carries a
    /// dictionary the types it is called at (`pkg.total<Item>`).
    pub key: String,
    /// The TS function.
    pub ts: String,
    /// The dictionaries the TS call passes after the drawn arguments: the
    /// companions of the types it is called at (`pkg.Item`), and for a std
    /// bound at `i32` its record (`std_dict`).
    pub dicts: Vec<String>,
    /// The Rust name in the file that declares it: `next`, `Ulid::parse`.
    pub rust: String,
    /// Each parameter's type where the call needs it spelled.
    pub types: Vec<Option<String>>,
    pub ret: Ty,
    /// Index into the crate's files.
    pub file: usize,
}

/// A function left out, and why.
pub struct Skipped {
    pub name: String,
    pub why: String,
}

/// What `test` does with a crate: the functions it calls, those it cannot,
/// and the types it can draw.
pub struct Plan<'a> {
    pub calls: Vec<Call>,
    pub skipped: Vec<Skipped>,
    make: Makeable<'a>,
}

/// The public functions of `krate`, split into those `test` calls and
/// those it cannot.
pub fn plan<'a>(krate: &'a Crate, spans: &'a [(usize, purecrate_syntax::LineCol)]) -> Plan<'a> {
    let make = Makeable::of(krate, spans);
    let mut calls = Vec::new();
    let mut skipped = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        let Item::Fn(f) = item else { continue };
        if f.vis != Vis::Pub || !owner_is_public(krate, f) {
            continue;
        }
        let instances = match instances(krate, &make, f) {
            Ok(instances) => instances,
            Err(why) => {
                let name = match &f.owner {
                    Some(owner) => format!("{}::{}", owner.as_str(), f.name.as_str()),
                    None => f.name.as_str().to_string(),
                };
                skipped.push(Skipped { name, why });
                continue;
            }
        };
        let dicts = dicts_of(krate, f);
        for binds in instances {
            let name = rust_name(krate, f, &binds);
            let written: Vec<&purecrate_ir::Param> =
                f.params.iter().filter(|p| dict_of(krate, &p.ty).is_none()).collect();
            let ret = f.ret.subst(&binds);
            let why = if f.owner.is_none() && f.name.as_str() == "main" {
                Some("`main` is the crate's entry point".to_string())
            } else if super::printers::ts_printer(krate, &ret).is_none() {
                Some("its result cannot be printed".to_string())
            } else {
                written
                    .iter()
                    .map(|p| p.ty.subst(&binds))
                    .find(|t| !make.can(t))
                    .map(|t| format!("no value of `{}` can be drawn", ty_text(&t)))
            };
            let ts = ts_key(f);
            // At the types its bounds are instantiated at, named in the key.
            let (key, companions) = if dicts.is_empty() {
                (ts.clone(), Vec::new())
            } else {
                let at = f.generics.iter().map(|g| ty_text(&Ty::Param(g.clone()).subst(&binds))).collect::<Vec<_>>();
                let companions = dicts
                    .iter()
                    .map(|(tr, of)| match of.subst(&binds).applied() {
                        Some((owner, _)) => format!("pkg.{}", owner.as_str()),
                        None => std_dict(tr.as_str()).to_string(),
                    })
                    .collect();
                (format!("{ts}<{}>", at.join(", ")), companions)
            };
            match why {
                Some(why) => skipped.push(Skipped { name, why }),
                None => calls.push(Call {
                    key,
                    ts,
                    dicts: companions,
                    rust: name,
                    types: annotations(f, &written, &binds),
                    ret,
                    file: spans[i].0,
                }),
            }
        }
    }
    Plan { calls, skipped, make }
}

/// The trait and type of a dictionary parameter's type (`Priced<T>`).
fn dict_of<'t>(krate: &Crate, ty: &'t Ty) -> Option<(&'t Name, &'t Ty)> {
    match ty {
        Ty::App { name, args } if args.len() == 1 => {
            let named = krate.items.iter().any(|i| matches!(i, Item::Trait(t) if t.name == *name))
                || purecrate_ir::STD_DICT_TRAITS.contains(&name.as_str());
            named.then(|| (name, &args[0]))
        }
        _ => None,
    }
}

/// The trait and the type each of `f`'s dictionaries is for, in order.
fn dicts_of(krate: &Crate, f: &Fn) -> Vec<(Name, Ty)> {
    f.params.iter().filter_map(|p| dict_of(krate, &p.ty).map(|(tr, of)| (tr.clone(), of.clone()))).collect()
}

/// A std bound's dictionary at `i32`, as the driver's JS writes it: the
/// record the crate would emit as `ordI32`, `displayI32`, or `defaultI32`,
/// which it emits only where its own calls pass one.
fn std_dict(trait_: &str) -> &'static str {
    match trait_ {
        "Display" => "{ toString: (x) => String(x) }",
        "Default" => "{ default: () => 0 }",
        _ => "{ cmp: (a, b) => ({ kind: a < b ? \"Less\" : a === b ? \"Equal\" : \"Greater\" }) }",
    }
}

/// Whether `trait_` is one of std's (`STD_DICT_TRAITS`), not the crate's.
fn is_std(krate: &Crate, trait_: &Name) -> bool {
    purecrate_ir::STD_DICT_TRAITS.contains(&trait_.as_str())
        && !krate.items.iter().any(|i| matches!(i, Item::Trait(t) if t.name == *trait_))
}

/// The types `f` is called at (design/01 §7.25): a type parameter whose
/// bounds carry a dictionary at each public struct or enum of the crate
/// that implements them all and can be drawn (a generic one at `i32`),
/// at `i32` where they are std's alone (`Ord`, `Display`, `Default`), and
/// every other one at `i32`, which every bound §7.20 takes holds of. Each
/// combination once. `Err` when no type implements a parameter's bounds.
fn instances(krate: &Crate, make: &Makeable<'_>, f: &Fn) -> Result<Vec<Vec<(Name, Ty)>>, String> {
    let mut out: Vec<Vec<(Name, Ty)>> = vec![Vec::new()];
    for g in &f.generics {
        let bounds: Vec<&Name> = f
            .params
            .iter()
            .filter_map(|p| match dict_of(krate, &p.ty) {
                Some((tr, Ty::Param(of))) if of == g => Some(tr),
                _ => None,
            })
            .collect();
        let at: Vec<Ty> = if bounds.iter().all(|b| is_std(krate, b)) {
            vec![Ty::i32()]
        } else {
            let found: Vec<Ty> = implementors(krate, &bounds).into_iter().filter(|t| make.can(t)).collect();
            if found.is_empty() {
                let names = bounds.iter().map(|b| format!("`{}`", b.as_str())).collect::<Vec<_>>().join(" and ");
                return Err(format!(
                    "no public type of the crate that can be drawn implements {names}, the bound of `{}`",
                    g.as_str()
                ));
            }
            found
        };
        out = out
            .into_iter()
            .flat_map(|binds| {
                at.iter().map(move |t| {
                    let mut binds = binds.clone();
                    binds.push((g.clone(), t.clone()));
                    binds
                })
            })
            .collect();
    }
    Ok(out)
}

/// The public structs and enums with an `impl` of each of `traits`, in
/// declaration order, a generic one at `i32` for each of its parameters.
/// A std trait's is the member its record asks for (`cmp`, a translated
/// `to_string`, `default`) on a type without parameters, whose companion
/// is the record.
fn implementors(krate: &Crate, traits: &[&Name]) -> Vec<Ty> {
    let has = |owner: &Name, name: &str, generic: bool| {
        // A derived `cmp` is made by the check, after this plan's crate.
        let derived = name == "cmp"
            && krate.items.iter().any(|i| match i {
                Item::Struct(s) => s.name == *owner && s.order == purecrate_ir::Order::Total,
                Item::Enum(e) => e.name == *owner && e.order == purecrate_ir::Order::Total,
                _ => false,
            });
        !generic
            && (derived
                || krate.items.iter().any(|i| {
                    matches!(i, Item::Fn(f) if f.owner.as_ref() == Some(owner) && f.name.as_str() == name
                        && (name != "to_string" || f.display))
                }))
    };
    let std_member = |t: &Name| match t.as_str() {
        "Display" => "to_string",
        "Default" => "default",
        _ => "cmp",
    };
    let impls = |name: &Name| -> Vec<&Name> {
        krate
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Trait(t) if t.name == *name => Some(t.impls.iter().map(|imp| imp.owner()).collect::<Vec<_>>()),
                _ => None,
            })
            .flatten()
            .collect()
    };
    krate
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Struct(s) if s.vis == Vis::Pub => Some((&s.name, s.generics.len())),
            Item::Enum(e) if e.vis == Vis::Pub && !e.std => Some((&e.name, e.generics.len())),
            _ => None,
        })
        .filter(|(name, n)| {
            traits.iter().all(
                |t| {
                    if is_std(krate, t) {
                        has(name, std_member(t), *n > 0)
                    } else {
                        impls(t).contains(name)
                    }
                },
            )
        })
        .map(|(name, n)| match n {
            0 => Ty::Named(name.clone()),
            n => Ty::App { name: name.clone(), args: vec![Ty::i32(); n] },
        })
        .collect()
}

fn owner_is_public(krate: &Crate, f: &Fn) -> bool {
    let Some(owner) = &f.owner else { return true };
    krate.items.iter().any(|i| match i {
        Item::Struct(s) => &s.name == owner && s.vis == Vis::Pub,
        Item::Enum(e) => &e.name == owner && e.vis == Vis::Pub,
        _ => false,
    })
}

/// The TS function: `pkg.next`, `pkg.Ulid.parse`.
fn ts_key(f: &Fn) -> String {
    match &f.owner {
        Some(owner) => format!("pkg.{}.{}", owner.as_str(), purecrate_ir::to_camel(f.name.as_str())),
        None => format!("pkg.{}", purecrate_ir::to_camel(f.name.as_str())),
    }
}

/// The function as the harness calls it: a generic one at the types
/// `binds` gives its parameters (`total::<Item>`), and at `i32` for any
/// other (`Page::<i32>::len`, `first::<i32>`).
fn rust_name(krate: &Crate, f: &Fn, binds: &[(Name, Ty)]) -> String {
    let texts: Vec<String> = f.generics.iter().map(|g| ty_text(&Ty::Param(g.clone()).subst(binds))).collect();
    // Rust takes no type arguments for a function with `impl Tr` as a
    // parameter's type: its arguments are typed instead (`annotations`).
    let at = |from: usize, n: usize| {
        if n == 0 || f.generics[from..from + n].iter().any(is_impl_param) {
            String::new()
        } else {
            format!("::<{}>", texts[from..from + n].join(", "))
        }
    };
    match &f.owner {
        Some(owner) => {
            let outer = krate
                .items
                .iter()
                .find(|i| i.name() == owner)
                .map(|i| match i {
                    Item::Struct(s) => s.generics.len(),
                    Item::Enum(e) => e.generics.len(),
                    _ => 0,
                })
                .unwrap_or(0);
            let outer = outer.min(f.generics.len());
            // `impl From<S>`'s member `fromS` is `X::from` in Rust.
            let name = if f.from_source().is_some() { "from" } else { f.name.as_str() };
            format!("{}{}::{}{}", owner.as_str(), at(0, outer), name, at(outer, f.generics.len() - outer))
        }
        None => format!("{}{}", f.name.as_str(), at(0, f.generics.len())),
    }
}

/// A type parameter `syntax` made for `impl Tr` as a parameter's type
/// (`Impl1`), which Rust gives no name.
fn is_impl_param(g: &Name) -> bool {
    g.as_str().strip_prefix("Impl").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn ty_text(ty: &Ty) -> String {
    match ty {
        Ty::Prim(p) => format!("{p:?}").to_lowercase(),
        Ty::Option(t) => format!("Option<{}>", ty_text(t)),
        Ty::Result { ok, err } => format!("Result<{}, {}>", ty_text(ok), ty_text(err)),
        Ty::Vec(t) => format!("Vec<{}>", ty_text(t)),
        Ty::Map { key, value } => format!("BTreeMap<{}, {}>", ty_text(key), ty_text(value)),
        Ty::Set(t) => format!("BTreeSet<{}>", ty_text(t)),
        Ty::App { name, args } => {
            format!("{}<{}>", name.as_str(), args.iter().map(ty_text).collect::<Vec<_>>().join(", "))
        }
        // A generic function is tested at `i32` (`instantiate`).
        Ty::Param(_) => "i32".to_string(),
        Ty::Tuple(ts) => format!("({})", ts.iter().map(ty_text).collect::<Vec<_>>().join(", ")),
        Ty::Named(n) => n.as_str().to_string(),
        Ty::Fn { .. } => "a closure".to_string(),
        Ty::Ignored { wrapper, inner } => format!("{}<{}>", wrapper.rust_name(), ty_text(inner)),
        Ty::Never => "!".to_string(),
    }
}

/// Which types `Arb` can draw: decided once over the whole crate, as a
/// type may need another declared after it.
struct Makeable<'a> {
    krate: &'a Crate,
    spans: &'a [(usize, purecrate_syntax::LineCol)],
    /// Structs and enums with an `Arb`.
    named: BTreeSet<String>,
}

impl<'a> Makeable<'a> {
    fn of(krate: &'a Crate, spans: &'a [(usize, purecrate_syntax::LineCol)]) -> Self {
        let mut m = Makeable { krate, spans, named: BTreeSet::new() };
        // A fixed point: each pass adds the types whose parts it already has.
        loop {
            let before = m.named.len();
            for item in &krate.items {
                let ok = match item {
                    Item::Struct(st) if !m.named.contains(st.name.as_str()) => m.struct_ok(st),
                    Item::Enum(en) if !en.std && !m.named.contains(en.name.as_str()) => {
                        en.variants.iter().any(|v| m.variant_ok(&v.fields))
                    }
                    _ => false,
                };
                if ok {
                    m.named.insert(item.name().as_str().to_string());
                }
            }
            if m.named.len() == before {
                return m;
            }
        }
    }

    fn struct_ok(&self, st: &Struct) -> bool {
        if st.closed {
            !producers(self.krate, self.spans, st).iter().all(|f| f.params.iter().any(|p| !self.can(&p.ty)))
        } else {
            st.fields.iter().all(|f| self.can(&f.ty))
        }
    }

    fn variant_ok(&self, fields: &VariantFields) -> bool {
        match fields {
            VariantFields::Unit => true,
            VariantFields::Tuple(tys) => tys.iter().all(|t| self.can(t)),
            VariantFields::Struct(fs) => fs.iter().all(|f| self.can(&f.ty)),
        }
    }

    fn can(&self, ty: &Ty) -> bool {
        match ty {
            Ty::Prim(Prim::Uuid | Prim::UuidError | Prim::ParseIntError) => false,
            Ty::Prim(_) => true,
            Ty::Option(t) | Ty::Vec(t) | Ty::Set(t) | Ty::Ignored { inner: t, .. } => self.can(t),
            Ty::Map { key, value } => self.can(key) && self.can(value),
            Ty::Param(_) => true,
            Ty::App { name, args } => self.named.contains(name.as_str()) && args.iter().all(|t| self.can(t)),
            Ty::Result { ok, err } => self.can(ok) && self.can(err),
            Ty::Tuple(ts) => (1..=5).contains(&ts.len()) && ts.iter().all(|t| self.can(t)),
            Ty::Named(n) => match self.krate.items.iter().find(|i| i.name() == n) {
                Some(Item::Alias(al)) => self.can(&al.ty),
                Some(Item::Enum(en)) if en.std => true,
                _ => self.named.contains(n.as_str()),
            },
            Ty::Fn { .. } | Ty::Never => false,
        }
    }
}

/// How a constructor gives the value: as it is, or in `Ok` / `Some`.
#[derive(Clone, Copy)]
enum Gives {
    Plain,
    Ok,
    Some,
}

fn gives(st: &Struct, ret: &Ty) -> Option<Gives> {
    let is = |t: &Ty| matches!(t, Ty::Named(n) if n == &st.name);
    match ret {
        t if is(t) => Some(Gives::Plain),
        Ty::Result { ok, .. } if is(ok) => Some(Gives::Ok),
        Ty::Option(t) if is(t) => Some(Gives::Some),
        _ => None,
    }
}

/// The public functions that make a closed `st` and do not take one: its
/// associated functions, and the free functions of the file that declares
/// it (where they are in scope by name).
fn producers<'a>(krate: &'a Crate, spans: &[(usize, purecrate_syntax::LineCol)], st: &Struct) -> Vec<&'a Fn> {
    let home = krate.items.iter().position(|i| matches!(i, Item::Struct(s) if s.name == st.name)).map(|i| spans[i].0);
    krate
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| match item {
            Item::Fn(f)
                if (f.owner.as_ref() == Some(&st.name) || (f.owner.is_none() && Some(spans[i].0) == home))
                    && f.vis == Vis::Pub
                    && gives(st, &f.ret).is_some()
                    && !f.params.iter().any(|p| p.ty.any(&|t| matches!(t, Ty::Named(n) if n == &st.name))) =>
            {
                Some(f)
            }
            _ => None,
        })
        .collect()
}

/// What to append to each of `files` (by index), with `root` the crate
/// root's index. `modules[i]` is file `i`'s module path.
pub fn appended(plan: &Plan<'_>, modules: &[Vec<String>], words: &Words) -> Vec<String> {
    let Plan { calls, make, .. } = plan;
    let (krate, spans) = (make.krate, make.spans);
    let mut out = vec![String::new(); modules.len()];
    for (i, item) in krate.items.iter().enumerate() {
        let file = spans[i].0;
        match item {
            Item::Struct(st) => {
                out[file].push_str(&derive::show_struct(st, SHOW, ATTRS));
                if make.named.contains(st.name.as_str()) {
                    out[file].push_str(&if st.closed { arb_closed(make, st) } else { arb_open(st) });
                }
            }
            Item::Enum(en) if !en.std => {
                out[file].push_str(&derive::show_enum(en, SHOW, ATTRS));
                if make.named.contains(en.name.as_str()) {
                    out[file].push_str(&arb_enum(make, en));
                }
            }
            _ => {}
        }
    }
    for (file, module) in modules.iter().enumerate() {
        let mut body = String::new();
        // Child modules: the files whose nearest enclosing file is this one.
        for (other, path) in modules.iter().enumerate() {
            if other != file && parent_file(modules, other) == Some(file) {
                let rel = &path[module.len()..];
                body.push_str(&format!("    {}::__purecrate_register(__purecrate_cases);\n", rel.join("::")));
            }
        }
        for call in calls.iter().filter(|c| c.file == file) {
            body.push_str(&case(call));
        }
        out[file].push_str(&format!(
            "\n#[doc(hidden)]\n#[allow(warnings, clippy::all)]\npub(crate) fn __purecrate_register(__purecrate_cases: &mut Vec<crate::__purecrate::Case>) {{\n{body}}}\n"
        ));
    }
    let sizes = words.sizes.iter().map(u64::to_string).collect::<Vec<_>>().join(", ");
    out[0].push_str(
        &PRELUDE
            .replace("/*WORDS*/", &words.strings.join(", "))
            .replace("/*LETTERS*/", &words.chars.join(", "))
            .replace("/*SIZES*/", &sizes),
    );
    out[0].push_str("\nfn main() {\n    crate::__purecrate::main(__purecrate_register);\n}\n");
    out
}

/// The file `index` sits in: the one with the longest module path that is a
/// proper prefix of its own.
fn parent_file(modules: &[Vec<String>], index: usize) -> Option<usize> {
    let path = &modules[index];
    modules
        .iter()
        .enumerate()
        .filter(|(i, m)| *i != index && m.len() < path.len() && path.starts_with(m))
        .max_by_key(|(_, m)| m.len())
        .map(|(i, _)| i)
}

const ARB: &str = "crate::__purecrate::Arb::arb(__r)?";

/// `let (__a0, __j0) = Arb::arb(__r)?;` for each parameter, typed where
/// `types` says, and the TS arguments.
fn draw(types: &[Option<String>], indent: &str) -> (String, String) {
    let lets = types
        .iter()
        .enumerate()
        .map(|(i, t)| match t {
            Some(t) => format!("{indent}let (__a{i}, __j{i}): ({t}, String) = {ARB};\n"),
            None => format!("{indent}let (__a{i}, __j{i}) = {ARB};\n"),
        })
        .collect();
    let js = if types.is_empty() {
        "String::new()".to_string()
    } else {
        format!("[{}].join(\", \")", (0..types.len()).map(|i| format!("__j{i}")).collect::<Vec<_>>().join(", "))
    };
    (lets, js)
}

/// `n` parameters rustc infers from the call.
fn untyped(n: usize) -> Vec<Option<String>> {
    vec![None; n]
}

/// The parameter types a call needs spelled: a conversion trait's
/// (`T::try_from(x)`) cannot be inferred from the call, as the trait is
/// implemented for several. `written` are the parameters drawn: all but
/// the dictionaries.
/// One of `impl Tr`, which no type argument names, is typed at what
/// `binds` gives it.
fn annotations(f: &Fn, written: &[&purecrate_ir::Param], binds: &[(Name, Ty)]) -> Vec<Option<String>> {
    let generic = f.owner.is_some() && (matches!(f.name.as_str(), "try_from" | "from") || f.from_source().is_some());
    let anonymous = |t: &Ty| t.any(&|x| matches!(x, Ty::Param(g) if is_impl_param(g)));
    written
        .iter()
        .map(|p| match () {
            _ if generic => rust_ty(&p.ty),
            _ if anonymous(&p.ty) => rust_ty(&p.ty.subst(binds)),
            _ => None,
        })
        .collect()
}

/// `ty` as Rust spells it, where the IR has kept enough to: a borrow is
/// erased, so a `Vec` might have been a slice and a named type a reference.
fn rust_ty(ty: &Ty) -> Option<String> {
    Some(match ty {
        Ty::Prim(Prim::Str) => "&'static str".to_string(),
        Ty::Prim(Prim::String) => "String".to_string(),
        Ty::Prim(Prim::Unit) => "()".to_string(),
        Ty::Prim(p) => format!("{p:?}").to_lowercase(),
        Ty::Option(t) => format!("Option<{}>", rust_ty(t)?),
        Ty::Result { ok, err } => format!("Result<{}, {}>", rust_ty(ok)?, rust_ty(err)?),
        Ty::Tuple(ts) => format!("({},)", ts.iter().map(rust_ty).collect::<Option<Vec<_>>>()?.join(", ")),
        Ty::Named(n) => n.as_str().to_string(),
        Ty::Param(_) => "i32".to_string(),
        Ty::App { name, args } => {
            format!("{}<{}>", name.as_str(), args.iter().map(rust_ty).collect::<Option<Vec<_>>>()?.join(", "))
        }
        Ty::Vec(_) | Ty::Map { .. } | Ty::Set(_) | Ty::Ignored { .. } | Ty::Fn { .. } | Ty::Never => return None,
    })
}

fn args(n: usize) -> String {
    (0..n).map(|i| format!("__a{i}")).collect::<Vec<_>>().join(", ")
}

fn case(call: &Call) -> String {
    let (lets, js) = draw(&call.types, "            ");
    format!(
        "    __purecrate_cases.push(crate::__purecrate::Case {{\n        key: {key:?},\n        run: |__r| {{\n{lets}            let __js = {js};\n            Some((__js, crate::__purecrate::shown(move || {f}({args}))))\n        }},\n    }});\n",
        key = call.key,
        f = call.rust,
        args = args(call.types.len()),
    )
}

/// What every impl the harness appends carries: the crate's lints are not
/// the harness's business.
const ATTRS: &str = "#[doc(hidden)]\n#[allow(warnings, clippy::all)]\n";
const SHOW: &str = "crate::__purecrate::Show";

fn arb_impl(name: &str, generics: &[purecrate_ir::Name], body: &str) -> String {
    let sig = "fn arb(__r: &mut crate::__purecrate::Rng) -> Option<(Self, String)>";
    derive::impl_block(
        ATTRS,
        "crate::__purecrate::Arb",
        name,
        generics,
        sig,
        &format!("__r.nest(|__r| {{\n{body}        }})"),
    )
}

/// `__j0, __j1, ..`: the TS of `n` drawn values.
fn js_args(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("__j{i}")).collect()
}

/// `(value, "({ a: .., b: .. })")` from a draw per field, or for a newtype
/// its content, as brands exist only in types.
fn arb_open(st: &Struct) -> String {
    let name = st.name.as_str();
    let (lets, _) = draw(&untyped(st.fields.len()), "            ");
    let body = match st.fields.as_slice() {
        [f] if f.name.as_str() == NEWTYPE_FIELD => format!("{lets}            Some(({name}(__a0), __j0))\n"),
        fields => {
            let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            let inits = names.iter().enumerate().map(|(i, n)| format!("{n}: __a{i}")).collect::<Vec<_>>();
            // `S {}` builds a struct of no fields whether it was declared
            // `struct S;` or `struct S {}`, where `S` names only the first.
            let value =
                if fields.is_empty() { format!("{name} {{}}") } else { format!("{name} {{ {} }}", inits.join(", ")) };
            let js = derive::js_literal(None, &derive::Fields::Named(names), &js_args(fields.len()));
            format!("{lets}            Some(({value}, {js}))\n")
        }
    };
    arb_impl(name, &st.generics, &body)
}

/// Up to `tries` times: one of `ways` drawn, each an expression giving an
/// `Option` of the value and its TS.
fn retry(tries: u32, ways: &[String]) -> String {
    let arms: String = ways.iter().enumerate().map(|(k, way)| format!("                    {k} => {way},\n")).collect();
    format!(
        "            for _ in 0..{tries} {{\n                let made = match __r.below({}) {{\n{arms}                    _ => None,\n                }};\n                if made.is_some() {{\n                    return made;\n                }}\n            }}\n            None\n",
        ways.len()
    )
}

/// Up to 16 tries: a constructor drawn, its arguments drawn, and the
/// value kept when it gives one.
fn arb_closed(make: &Makeable<'_>, st: &Struct) -> String {
    let ways: Vec<String> = producers(make.krate, make.spans, st)
        .into_iter()
        .filter(|f| f.params.iter().all(|p| make.can(&p.ty)))
        .map(|f| {
            let (lets, js) = draw(&annotations(f, &f.params.iter().collect::<Vec<_>>(), &[]), "                        ");
            let call = format!("{}({})", rust_name(make.krate, f, &[]), args(f.params.len()));
            let ts = ts_key(f);
            let (unwrap, wrap) = match gives(st, &f.ret).expect("a producer") {
                Gives::Plain => ("Some(__v)", "{}"),
                Gives::Ok => ("__v.ok()", "__ok({})"),
                Gives::Some => ("__v", "__some({})"),
            };
            let recipe = wrap.replace("{}", &format!("{ts}({{}})"));
            format!(
                "(|| {{\n{lets}                        let __js = {js};\n                        let __v = crate::__purecrate::attempt(move || {call})?;\n                        Some(({unwrap}?, format!({recipe:?}, __js)))\n                    }})()"
            )
        })
        .collect();
    arb_impl(st.name.as_str(), &st.generics, &retry(16, &ways))
}

/// A variant drawn, up to 8 times until its fields can be.
fn arb_enum(make: &Makeable<'_>, en: &Enum) -> String {
    let name = en.name.as_str();
    let ways: Vec<String> = en
        .variants
        .iter()
        .filter(|v| make.variant_ok(&v.fields))
        .map(|v| {
            let var = v.name.as_str();
            let fields = derive::Fields::of(&v.fields);
            let (value, n) = match &fields {
                derive::Fields::Tuple(0) => (format!("{name}::{var}"), 0),
                derive::Fields::Tuple(n) => (format!("{name}::{var}({})", args(*n)), *n),
                derive::Fields::Named(names) => {
                    let inits = names.iter().enumerate().map(|(i, n)| format!("{n}: __a{i}")).collect::<Vec<_>>();
                    (format!("{name}::{var} {{ {} }}", inits.join(", ")), names.len())
                }
            };
            let js = derive::js_literal(Some(var), &fields, &js_args(n));
            if n == 0 {
                format!("Some(({value}, {js}))")
            } else {
                let (lets, _) = draw(&untyped(n), "                        ");
                format!("(|| {{\n{lets}                        Some(({value}, {js}))\n                    }})()")
            }
        })
        .collect();
    arb_impl(name, &en.generics, &retry(8, &ways))
}

/// What the crate itself spells, drawn from more often than the fixed
/// alphabet: the string and `char` literals of its source, as Rust source,
/// and its small integer literals, which are often a length it checks.
#[derive(Default)]
pub struct Words {
    pub strings: Vec<String>,
    pub chars: Vec<String>,
    pub sizes: Vec<u64>,
}

impl Words {
    /// The short literals of `source`: `"…"` (not raw, not byte strings)
    /// and `'…'`, kept as written.
    pub fn add(&mut self, source: &str) {
        let b = source.as_bytes();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'/' if b.get(i + 1) == Some(&b'/') => {
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                }
                b'"' => {
                    let prefixed = i > 0 && (b[i - 1] == b'b' || b[i - 1] == b'r' || b[i - 1] == b'#');
                    let end = literal_end(b, i, b'"');
                    let text = &source[i..end];
                    if !prefixed && end - i <= 34 && !text.contains('\n') {
                        self.strings.push(text.to_string());
                    }
                    i = end;
                    continue;
                }
                b'\'' => {
                    // A lifetime or a label has no closing quote within reach.
                    let end = literal_end(b, i, b'\'');
                    let text = &source[i..end];
                    if end - i <= 12 && text.ends_with('\'') && text.len() > 2 {
                        // A byte literal is consumed too, so its closing quote
                        // does not open another.
                        if !(i > 0 && b[i - 1] == b'b') {
                            self.chars.push(text.to_string());
                        }
                        i = end;
                        continue;
                    }
                }
                c if c.is_ascii_digit() && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) => {
                    let end = (i..b.len()).find(|&j| !b[j].is_ascii_digit() && b[j] != b'_').unwrap_or(b.len());
                    if let Ok(n) = source[i..end].replace('_', "").parse::<u64>() {
                        if n <= 64 {
                            self.sizes.push(n);
                        }
                    }
                    i = end;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        self.sizes.sort();
        self.sizes.dedup();
        self.strings.sort();
        self.strings.dedup();
        self.chars.sort();
        self.chars.dedup();
    }
}

/// Past the literal opened at `start` by `quote`, or the end of the line
/// for a `'` that opens none.
fn literal_end(b: &[u8], start: usize, quote: u8) -> usize {
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            c if c == quote => return i + 1,
            b'\n' if quote == b'\'' => return i,
            _ => i += 1,
        }
    }
    b.len()
}

/// The harness's own module, appended to the crate root: the draws, the
/// canonical text of std's types (`show.rs`), and the loop that runs the
/// cases.
const PRELUDE: &str = concat!(
    r##"
#[doc(hidden)]
#[allow(warnings, clippy::all)]
pub(crate) mod __purecrate {
    use std::io::Write;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// The crate's own string and `char` literals.
    static WORDS: &[&str] = &[/*WORDS*/];
    static LETTERS: &[char] = &[/*LETTERS*/];
    /// Its integer literals up to 64.
    static SIZES: &[u64] = &[/*SIZES*/];
    /// Characters every string may hold: ASCII the parsers of a crate
    /// usually test for, whitespace, controls, and code points of two,
    /// three, and four bytes.
    static ALPHABET: &[char] = &[
        'a', 'b', 'z', 'A', 'Z', '0', '1', '9', ' ', '.', '-', '_', '+', ':', '/', '@', '#', '%', '=', ',', ';', '"',
        '\\', '\'', '\t', '\n', '\0', '\u{7f}', '\u{e9}', '\u{df}', '\u{20ac}', '\u{4e2d}', '\u{fffd}', '\u{1f600}',
        '\u{10ffff}',
    ];
    const MAX_DEPTH: u32 = 5;

    /// SplitMix64, so a seed always draws the same arguments.
    pub struct Rng {
        state: u64,
        depth: u32,
    }

    impl Rng {
        pub fn next(&mut self) -> u64 {
            self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.state;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        pub fn below(&mut self, n: u64) -> u64 {
            if n == 0 { 0 } else { self.next() % n }
        }

        fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
            xs[self.below(xs.len() as u64) as usize]
        }

        /// `f` one level down, or `None` past the deepest level, so a
        /// recursive type ends.
        pub fn nest<T>(&mut self, f: impl FnOnce(&mut Rng) -> Option<T>) -> Option<T> {
            if self.depth >= MAX_DEPTH {
                return None;
            }
            self.depth += 1;
            let v = f(self);
            self.depth -= 1;
            v
        }

        fn deep(&self) -> bool {
            self.depth + 1 >= MAX_DEPTH
        }

        fn char(&mut self) -> char {
            match self.below(4) {
                0 if !LETTERS.is_empty() => self.pick(LETTERS),
                1 => char::from(0x20 + self.below(95) as u8),
                _ => self.pick(ALPHABET),
            }
        }

        /// A length: one the crate spells a third of the time, else short.
        fn len(&mut self, short: u64) -> u64 {
            if !SIZES.is_empty() && self.below(3) == 0 {
                self.pick(SIZES)
            } else {
                self.below(short + 1)
            }
        }

        fn text(&mut self) -> String {
            if !WORDS.is_empty() && self.below(4) == 0 {
                return self.pick(WORDS).to_string();
            }
            if self.below(3) == 0 {
                // Two to four characters, repeated: digits and dots make a
                // version, letters and a separator a token list, where a
                // draw from the whole alphabet would almost never parse.
                let mut few = Vec::new();
                for _ in 0..2 + self.below(3) {
                    let c = match self.below(3) {
                        0 => char::from(b'0' + self.below(10) as u8),
                        _ => self.char(),
                    };
                    few.push(c);
                }
                return (0..1 + self.len(11)).map(|_| self.pick(&few)).collect();
            }
            let mut s = String::new();
            for _ in 0..self.len(7) {
                match self.below(4) {
                    0 if !WORDS.is_empty() => s.push_str(self.pick(WORDS)),
                    _ => s.push(self.char()),
                }
            }
            s
        }
    }

    /// A value and its TS spelling.
    pub trait Arb: Sized {
        fn arb(r: &mut Rng) -> Option<(Self, String)>;
    }

    macro_rules! int {
        ($($t:ty, $js:literal;)*) => {$(
            impl Arb for $t {
                fn arb(r: &mut Rng) -> Option<(Self, String)> {
                    let edges: [$t; 8] =
                        [<$t>::MIN, <$t>::MIN.wrapping_add(1), (0 as $t).wrapping_sub(1), 0, 1, 2, <$t>::MAX - 1, <$t>::MAX];
                    let v: $t = match r.below(4) {
                        0 | 1 => r.pick(&edges),
                        2 => r.len(20) as $t,
                        _ => ((u128::from(r.next()) << 64) | u128::from(r.next())) as $t,
                    };
                    Some((v, format!($js, v)))
                }
            }
        )*};
    }
    int! {
        i8, "{}"; i16, "{}"; i32, "{}"; u8, "{}"; u16, "{}"; u32, "{}";
        i64, "{}n"; i128, "{}n"; u64, "{}n"; u128, "{}n";
    }

    /// Mostly a length or an index; never past 2^53 - 1, where the TS
    /// `number` stops being exact (design/01 §3).
    impl Arb for usize {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = match r.below(8) {
                0 => r.pick(&[(1 << 53) - 1, (1 << 53) - 2, u32::MAX as usize, u32::MAX as usize + 1]),
                1 => r.below(1 << 53) as usize,
                _ => r.len(20) as usize,
            };
            Some((v, v.to_string()))
        }
    }

    impl Arb for bool {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = r.below(2) == 0;
            Some((v, v.to_string()))
        }
    }

    const F64S: [f64; 22] = [
        0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, 2.5, -2.5, 0.1, 100.0, 1e10, 1e-10, 9007199254740993.0,
        f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX, f64::MIN, f64::MIN_POSITIVE, f64::EPSILON, 1e300,
    ];

    fn float(r: &mut Rng) -> f64 {
        match r.below(4) {
            0 | 1 => r.pick(&F64S),
            2 => (r.below(1601) as f64 - 800.0) / 8.0,
            _ => f64::from_bits(r.next()),
        }
    }

    impl Arb for f64 {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = float(r);
            Some((v, js_float(v)))
        }
    }

    impl Arb for f32 {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = if r.below(4) == 0 { f32::from_bits(r.next() as u32) } else { float(r) as f32 };
            Some((v, format!("Math.fround({})", js_float(f64::from(v)))))
        }
    }

    impl Arb for String {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = r.text();
            let js = js_str(&v);
            Some((v, js))
        }
    }

    impl Arb for &'static str {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = String::arb(r)?;
            Some((Box::leak(v.into_boxed_str()), js))
        }
    }

    impl Arb for char {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = r.char();
            Some((v, js_str(&v.to_string())))
        }
    }

    impl Arb for () {
        fn arb(_: &mut Rng) -> Option<(Self, String)> {
            Some(((), "undefined".into()))
        }
    }

    impl Arb for std::cmp::Ordering {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = r.pick(&[std::cmp::Ordering::Less, std::cmp::Ordering::Equal, std::cmp::Ordering::Greater]);
            Some((v, format!("({{ kind: \"{v:?}\" }})")))
        }
    }

    impl<T: Arb> Arb for Option<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            if r.deep() || r.below(4) == 0 {
                return Some((None, "null".into()));
            }
            let (v, js) = r.nest(T::arb)?;
            Some((Some(v), js))
        }
    }

    impl<T: Arb, E: Arb> Arb for Result<T, E> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            if r.below(2) == 0 {
                let (v, js) = r.nest(T::arb)?;
                Some((Ok(v), format!("{{ kind: \"Ok\", value: {js} }}")))
            } else {
                let (e, js) = r.nest(E::arb)?;
                Some((Err(e), format!("{{ kind: \"Err\", error: {js} }}")))
            }
        }
    }

    impl<T: Arb> Arb for Vec<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let n = if r.deep() { 0 } else { r.len(4) };
            let mut vs = Vec::new();
            let mut js = Vec::new();
            for _ in 0..n {
                let (v, j) = r.nest(T::arb)?;
                vs.push(v);
                js.push(j);
            }
            Some((vs, format!("[{}]", js.join(", "))))
        }
    }

    /// Drawn in any order, as a TS caller may fill one; a key drawn twice
    /// keeps its last value on both sides.
    impl<V: Arb> Arb for std::collections::BTreeMap<String, V> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let n = if r.deep() { 0 } else { r.len(4) };
            let mut m = std::collections::BTreeMap::new();
            let mut js = Vec::new();
            for _ in 0..n {
                let (k, jk) = r.nest(String::arb)?;
                let (v, jv) = r.nest(V::arb)?;
                m.insert(k, v);
                js.push(format!("[{jk}, {jv}]"));
            }
            Some((m, format!("new Map([{}])", js.join(", "))))
        }
    }

    impl Arb for std::collections::BTreeSet<String> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let n = if r.deep() { 0 } else { r.len(4) };
            let mut s = std::collections::BTreeSet::new();
            let mut js = Vec::new();
            for _ in 0..n {
                let (k, jk) = r.nest(String::arb)?;
                s.insert(k);
                js.push(jk);
            }
            Some((s, format!("new Set([{}])", js.join(", "))))
        }
    }

    impl<T: Arb + 'static> Arb for &'static [T] {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = Vec::<T>::arb(r)?;
            Some((Box::leak(v.into_boxed_slice()), js))
        }
    }

    impl<T: Arb + 'static> Arb for &'static T {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((Box::leak(Box::new(v)), js))
        }
    }

    impl<T: Arb> Arb for Box<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((Box::new(v), js))
        }
    }

    impl<T: Arb> Arb for std::sync::Arc<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((std::sync::Arc::new(v), js))
        }
    }

    impl<T: Arb> Arb for std::sync::Mutex<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((std::sync::Mutex::new(v), js))
        }
    }

    macro_rules! tuple {
        ($($t:ident $j:ident),+) => {
            impl<$($t: Arb),+> Arb for ($($t,)+) {
                fn arb(r: &mut Rng) -> Option<(Self, String)> {
                    $(let ($t, $j) = $t::arb(r)?;)+
                    Some((($($t,)+), format!("[{}]", [$($j),+].join(", "))))
                }
            }
        };
    }
    tuple!(A ja);
    tuple!(A ja, B jb);
    tuple!(A ja, B jb, C jc);
    tuple!(A ja, B jb, C jc, D jd);
    tuple!(A ja, B jb, C jc, D jd, E je);

"##,
    include_str!("show.rs"),
    r##"
    /// One function under test: its TS name, and a call on drawn arguments
    /// that gives the TS arguments and the printed result.
    pub struct Case {
        pub key: &'static str,
        pub run: fn(&mut Rng) -> Option<(String, String)>,
    }

    /// `f`'s result, or `None` if it panics: a constructor that panics
    /// makes no value to test with.
    pub fn attempt<T>(f: impl FnOnce() -> T) -> Option<T> {
        catch_unwind(AssertUnwindSafe(f)).ok()
    }

    /// One line per line of text, as the TS side escapes its results too.
    fn escape(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
    }

    /// `case <key> <TS arguments> <result>`, tab-separated, `cases` per
    /// function, each function on its own stream of the seed.
    pub fn main(register: fn(&mut Vec<Case>)) {
        std::panic::set_hook(Box::new(|_| {}));
        let args: Vec<String> = std::env::args().skip(1).collect();
        let seed: u64 = args[0].parse().expect("seed");
        let cases: usize = args[1].parse().expect("cases");
        let mut all = Vec::new();
        register(&mut all);
        let out = std::io::stdout();
        let mut out = std::io::BufWriter::new(out.lock());
        for case in &all {
            // FNV-1a of the key: a function's arguments do not move when
            // another function is added.
            let stream = case.key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3));
            let mut r = Rng { state: seed ^ stream, depth: 0 };
            let mut drawn = 0;
            for _ in 0..cases * 20 {
                if drawn == cases {
                    break;
                }
                if let Some((js, result)) = (case.run)(&mut r) {
                    writeln!(out, "case\t{}\t{}\t{}", case.key, js, escape(&result)).expect("write");
                    drawn += 1;
                }
            }
        }
    }
}
"##
);
