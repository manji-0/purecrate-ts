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

use purecrate_ir::{Crate, Enum, Fn, Item, Prim, Struct, Ty, VariantFields, Vis, NEWTYPE_FIELD};

/// A function under test.
pub struct Call {
    /// The TS function: `pkg.next`, `pkg.Ulid.parse`.
    pub key: String,
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

/// The public functions of `krate`, split into those `test` calls and
/// those it cannot.
pub fn plan(krate: &Crate, spans: &[(usize, purecrate_syntax::LineCol)]) -> (Vec<Call>, Vec<Skipped>) {
    let make = Makeable::of(krate, spans);
    let mut calls = Vec::new();
    let mut skipped = Vec::new();
    for (i, item) in krate.items.iter().enumerate() {
        let Item::Fn(f) = item else { continue };
        if f.vis != Vis::Pub || !owner_is_public(krate, f) {
            continue;
        }
        let name = rust_name(f);
        let why = if f.owner.is_none() && f.name.as_str() == "main" {
            Some("`main` is the crate's entry point".to_string())
        } else if super::printers::ts_printer(krate, &f.ret).is_none() {
            Some("its result cannot be printed".to_string())
        } else {
            f.params.iter().find(|p| !make.can(&p.ty)).map(|p| format!("no value of `{}` can be drawn", ty_text(&p.ty)))
        };
        match why {
            Some(why) => skipped.push(Skipped { name, why }),
            None => calls.push(Call {
                key: ts_key(f),
                rust: name,
                types: annotations(f),
                ret: f.ret.clone(),
                file: spans[i].0,
            }),
        }
    }
    (calls, skipped)
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

fn rust_name(f: &Fn) -> String {
    match &f.owner {
        Some(owner) => format!("{}::{}", owner.as_str(), f.name.as_str()),
        None => f.name.as_str().to_string(),
    }
}

fn ty_text(ty: &Ty) -> String {
    match ty {
        Ty::Prim(p) => format!("{p:?}").to_lowercase(),
        Ty::Option(t) => format!("Option<{}>", ty_text(t)),
        Ty::Result { ok, err } => format!("Result<{}, {}>", ty_text(ok), ty_text(err)),
        Ty::Vec(t) => format!("Vec<{}>", ty_text(t)),
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
            Ty::Option(t) | Ty::Vec(t) | Ty::Ignored { inner: t, .. } => self.can(t),
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
pub fn appended(
    krate: &Crate,
    spans: &[(usize, purecrate_syntax::LineCol)],
    modules: &[Vec<String>],
    calls: &[Call],
    words: &Words,
) -> Vec<String> {
    let make = Makeable::of(krate, spans);
    let mut out = vec![String::new(); modules.len()];
    for (i, item) in krate.items.iter().enumerate() {
        let file = spans[i].0;
        match item {
            Item::Struct(st) => {
                out[file].push_str(&show_struct(st));
                if make.named.contains(st.name.as_str()) {
                    out[file].push_str(&if st.closed { arb_closed(krate, &make, st) } else { arb_open(st) });
                }
            }
            Item::Enum(en) if !en.std => {
                out[file].push_str(&show_enum(en));
                if make.named.contains(en.name.as_str()) {
                    out[file].push_str(&arb_enum(&make, en));
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
/// implemented for several.
fn annotations(f: &Fn) -> Vec<Option<String>> {
    let generic = f.owner.is_some() && matches!(f.name.as_str(), "try_from" | "from");
    f.params.iter().map(|p| if generic { rust_ty(&p.ty) } else { None }).collect()
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
        Ty::Vec(_) | Ty::Ignored { .. } | Ty::Fn { .. } | Ty::Never => return None,
    })
}

fn args(n: usize) -> String {
    (0..n).map(|i| format!("__a{i}")).collect::<Vec<_>>().join(", ")
}

fn case(call: &Call) -> String {
    let (lets, js) = draw(&call.types, "            ");
    format!(
        "    __purecrate_cases.push(crate::__purecrate::Case {{\n        key: {key:?},\n        run: |__r| {{\n{lets}            let __js = {js};\n            Some((__js, crate::__purecrate::call(move || {f}({args}))))\n        }},\n    }});\n",
        key = call.key,
        f = call.rust,
        args = args(call.types.len()),
    )
}

fn show_struct(st: &Struct) -> String {
    let name = st.name.as_str();
    let body = match st.fields.as_slice() {
        [] => format!("{name:?}.to_string()"),
        [f] if f.name.as_str() == NEWTYPE_FIELD => {
            format!("format!(\"{name}({{}})\", crate::__purecrate::Show::show(&self.0))")
        }
        fields => {
            let pattern = fields.iter().map(|f| format!("{}: {{}}", f.name.as_str())).collect::<Vec<_>>().join(", ");
            let args = fields
                .iter()
                .map(|f| format!("crate::__purecrate::Show::show(&self.{})", f.name.as_str()))
                .collect::<Vec<_>>()
                .join(", ");
            format!("format!(\"{name} {{{{ {pattern} }}}}\", {args})")
        }
    };
    show_impl(name, &body)
}

fn show_enum(en: &Enum) -> String {
    let name = en.name.as_str();
    let mut arms = String::new();
    for v in &en.variants {
        let var = v.name.as_str();
        let arm = match &v.fields {
            VariantFields::Unit => format!("{name}::{var} => \"{name}::{var}\".to_string(),"),
            VariantFields::Tuple(tys) => {
                let holes = vec!["{}"; tys.len()].join(", ");
                let args = (0..tys.len())
                    .map(|i| format!("crate::__purecrate::Show::show(f{i})"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name}::{var}({}) => format!(\"{name}::{var}({holes})\", {args}),", binds(tys.len()))
            }
            VariantFields::Struct(fields) => {
                let names = fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>();
                let pattern = names.iter().map(|n| format!("{n}: {{}}")).collect::<Vec<_>>().join(", ");
                let args =
                    names.iter().map(|n| format!("crate::__purecrate::Show::show({n})")).collect::<Vec<_>>().join(", ");
                format!(
                    "{name}::{var} {{ {} }} => format!(\"{name}::{var} {{{{ {pattern} }}}}\", {args}),",
                    names.join(", ")
                )
            }
        };
        arms.push_str(&format!("            {arm}\n"));
    }
    show_impl(name, &format!("match self {{\n{arms}        }}"))
}

fn binds(n: usize) -> String {
    (0..n).map(|i| format!("f{i}")).collect::<Vec<_>>().join(", ")
}

fn show_impl(name: &str, body: &str) -> String {
    format!(
        "\n#[doc(hidden)]\n#[allow(warnings, clippy::all)]\nimpl crate::__purecrate::Show for {name} {{\n    fn show(&self) -> String {{\n        {body}\n    }}\n}}\n"
    )
}

fn arb_impl(name: &str, body: &str) -> String {
    format!(
        "\n#[doc(hidden)]\n#[allow(warnings, clippy::all)]\nimpl crate::__purecrate::Arb for {name} {{\n    fn arb(__r: &mut crate::__purecrate::Rng) -> Option<(Self, String)> {{\n        __r.nest(|__r| {{\n{body}        }})\n    }}\n}}\n"
    )
}

/// `(value, "({ a: .., b: .. })")` from a draw per field, or for a newtype
/// its content, as brands exist only in types.
fn arb_open(st: &Struct) -> String {
    let name = st.name.as_str();
    let (lets, _) = draw(&untyped(st.fields.len()), "            ");
    let body = match st.fields.as_slice() {
        [] => format!("            Some(({name}, \"({{}})\".to_string()))\n"),
        [f] if f.name.as_str() == NEWTYPE_FIELD => format!("{lets}            Some(({name}(__a0), __j0))\n"),
        fields => {
            let inits =
                fields.iter().enumerate().map(|(i, f)| format!("{}: __a{i}", f.name.as_str())).collect::<Vec<_>>();
            let js = fields.iter().map(|f| format!("{}: {{}}", f.name.as_str())).collect::<Vec<_>>().join(", ");
            let js_args = (0..fields.len()).map(|i| format!("__j{i}")).collect::<Vec<_>>().join(", ");
            format!(
                "{lets}            Some(({name} {{ {} }}, format!(\"({{{{ {js} }}}})\", {js_args})))\n",
                inits.join(", ")
            )
        }
    };
    arb_impl(name, &body)
}

/// Up to 16 tries: a constructor drawn, its arguments drawn, and the
/// value kept when it gives one.
fn arb_closed(krate: &Crate, make: &Makeable<'_>, st: &Struct) -> String {
    let name = st.name.as_str();
    let ways: Vec<&Fn> =
        producers(krate, make.spans, st).into_iter().filter(|f| f.params.iter().all(|p| make.can(&p.ty))).collect();
    let mut arms = String::new();
    for (k, f) in ways.iter().enumerate() {
        let n = f.params.len();
        let (lets, js) = draw(&annotations(f), "                        ");
        let call = format!("{}({})", rust_name(f), args(n));
        let ts = ts_key(f);
        let (unwrap, wrap) = match gives(st, &f.ret).expect("a producer") {
            Gives::Plain => ("Some(__v)", "{}"),
            Gives::Ok => ("__v.ok()", "__ok({})"),
            Gives::Some => ("__v", "__some({})"),
        };
        let recipe = wrap.replace("{}", &format!("{ts}({{}})"));
        arms.push_str(&format!(
            "                    {k} => (|| {{\n{lets}                        let __js = {js};\n                        let __v = crate::__purecrate::attempt(move || {call})?;\n                        Some(({unwrap}?, format!({recipe:?}, __js)))\n                    }})(),\n"
        ));
    }
    let body = format!(
        "            for _ in 0..16 {{\n                let made = match __r.below({}) {{\n{arms}                    _ => None,\n                }};\n                if made.is_some() {{\n                    return made;\n                }}\n            }}\n            None\n",
        ways.len()
    );
    arb_impl(name, &body)
}

/// A variant drawn, up to 8 times until its fields can be.
fn arb_enum(make: &Makeable<'_>, en: &Enum) -> String {
    let name = en.name.as_str();
    let mut arms = String::new();
    let ways: Vec<_> = en.variants.iter().filter(|v| make.variant_ok(&v.fields)).collect();
    for (k, v) in ways.iter().enumerate() {
        let var = v.name.as_str();
        let made = match &v.fields {
            VariantFields::Unit => format!("Some(({name}::{var}, \"({{ kind: \\\"{var}\\\" }})\".to_string()))"),
            VariantFields::Tuple(tys) => {
                let (lets, _) = draw(&untyped(tys.len()), "                        ");
                let js = if tys.len() == 1 {
                    format!("format!(\"({{{{ kind: \\\"{var}\\\", value: {{}} }}}})\", __j0)")
                } else {
                    let holes = vec!["{}"; tys.len()].join(", ");
                    let js_args = (0..tys.len()).map(|i| format!("__j{i}")).collect::<Vec<_>>().join(", ");
                    format!("format!(\"({{{{ kind: \\\"{var}\\\", content: [{holes}] }}}})\", {js_args})")
                };
                format!(
                    "(|| {{\n{lets}                        Some(({name}::{var}({}), {js}))\n                    }})()",
                    args(tys.len())
                )
            }
            VariantFields::Struct(fields) => {
                let (lets, _) = draw(&untyped(fields.len()), "                        ");
                let inits =
                    fields.iter().enumerate().map(|(i, f)| format!("{}: __a{i}", f.name.as_str())).collect::<Vec<_>>();
                let js = fields.iter().map(|f| format!("{}: {{}}", f.name.as_str())).collect::<Vec<_>>().join(", ");
                let js_args = (0..fields.len()).map(|i| format!("__j{i}")).collect::<Vec<_>>().join(", ");
                format!(
                    "(|| {{\n{lets}                        Some(({name}::{var} {{ {} }}, format!(\"({{{{ kind: \\\"{var}\\\", {js} }}}})\", {js_args})))\n                    }})()",
                    inits.join(", ")
                )
            }
        };
        arms.push_str(&format!("                    {k} => {made},\n"));
    }
    let body = format!(
        "            for _ in 0..8 {{\n                let made = match __r.below({}) {{\n{arms}                    _ => None,\n                }};\n                if made.is_some() {{\n                    return made;\n                }}\n            }}\n            None\n",
        ways.len()
    );
    arb_impl(name, &body)
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
/// canonical text of std's types, and the loop that runs the cases.
const PRELUDE: &str = r##"
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

    /// The canonical text both sides print (design/01 §8).
    pub trait Show {
        fn show(&self) -> String;
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
            impl Show for $t {
                fn show(&self) -> String {
                    self.to_string()
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

    impl Show for usize {
        fn show(&self) -> String {
            self.to_string()
        }
    }

    impl Arb for bool {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = r.below(2) == 0;
            Some((v, v.to_string()))
        }
    }

    impl Show for bool {
        fn show(&self) -> String {
            self.to_string()
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

    fn js_float(x: f64) -> String {
        if x.is_infinite() {
            if x > 0.0 { "Infinity" } else { "-Infinity" }.into()
        } else {
            format!("{x:?}")
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

    impl Show for f32 {
        fn show(&self) -> String {
            f64::from(*self).to_bits().to_string()
        }
    }

    impl Show for f64 {
        fn show(&self) -> String {
            self.to_bits().to_string()
        }
    }

    /// Rust's escapes, but U+0000 as `\u{0}`: `{:?}` writes `\0`, which
    /// before a digit is a legacy octal escape node refuses.
    fn js_str(s: &str) -> String {
        let body: String =
            s.chars().map(|c| if c == '\0' { "\\u{0}".to_string() } else { c.escape_debug().to_string() }).collect();
        format!("\"{body}\"")
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

    /// Printable ASCII but `"` and `\` as is, any other code point `\u{hex}`.
    impl Show for str {
        fn show(&self) -> String {
            let mut out = String::from("\"");
            for c in self.chars() {
                if (' '..='~').contains(&c) && c != '"' && c != '\\' {
                    out.push(c);
                } else {
                    out.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
                }
            }
            out.push('"');
            out
        }
    }

    impl Show for String {
        fn show(&self) -> String {
            self.as_str().show()
        }
    }

    impl Show for char {
        fn show(&self) -> String {
            format!("'{}'", self.to_string().show())
        }
    }

    impl Arb for () {
        fn arb(_: &mut Rng) -> Option<(Self, String)> {
            Some(((), "undefined".into()))
        }
    }

    impl Show for () {
        fn show(&self) -> String {
            "()".into()
        }
    }

    impl Show for std::num::ParseIntError {
        fn show(&self) -> String {
            "ParseIntError".into()
        }
    }

    impl Arb for std::cmp::Ordering {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let v = r.pick(&[std::cmp::Ordering::Less, std::cmp::Ordering::Equal, std::cmp::Ordering::Greater]);
            Some((v, format!("({{ kind: \"{v:?}\" }})")))
        }
    }

    impl Show for std::cmp::Ordering {
        fn show(&self) -> String {
            format!("Ordering::{self:?}")
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

    impl<T: Show> Show for Option<T> {
        fn show(&self) -> String {
            match self {
                Some(v) => format!("Some({})", v.show()),
                None => "None".into(),
            }
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

    impl<T: Show, E: Show> Show for Result<T, E> {
        fn show(&self) -> String {
            match self {
                Ok(v) => format!("Ok({})", v.show()),
                Err(e) => format!("Err({})", e.show()),
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

    impl<T: Arb + 'static> Arb for &'static [T] {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = Vec::<T>::arb(r)?;
            Some((Box::leak(v.into_boxed_slice()), js))
        }
    }

    impl<T: Show> Show for [T] {
        fn show(&self) -> String {
            format!("[{}]", self.iter().map(Show::show).collect::<Vec<_>>().join(", "))
        }
    }

    impl<T: Show> Show for Vec<T> {
        fn show(&self) -> String {
            self.as_slice().show()
        }
    }

    impl<T: Arb + 'static> Arb for &'static T {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((Box::leak(Box::new(v)), js))
        }
    }

    impl<T: Show + ?Sized> Show for &T {
        fn show(&self) -> String {
            (**self).show()
        }
    }

    impl<T: Arb> Arb for Box<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((Box::new(v), js))
        }
    }

    impl<T: Show + ?Sized> Show for Box<T> {
        fn show(&self) -> String {
            (**self).show()
        }
    }

    impl<T: Arb> Arb for std::sync::Arc<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((std::sync::Arc::new(v), js))
        }
    }

    impl<T: Show + ?Sized> Show for std::sync::Arc<T> {
        fn show(&self) -> String {
            (**self).show()
        }
    }

    impl<T: Arb> Arb for std::sync::Mutex<T> {
        fn arb(r: &mut Rng) -> Option<(Self, String)> {
            let (v, js) = T::arb(r)?;
            Some((std::sync::Mutex::new(v), js))
        }
    }

    impl<T: Show> Show for std::sync::Mutex<T> {
        fn show(&self) -> String {
            self.lock().map(|v| v.show()).unwrap_or_else(|_| "poisoned".into())
        }
    }

    macro_rules! tuple {
        ($($t:ident $i:tt $j:ident),+) => {
            impl<$($t: Arb),+> Arb for ($($t,)+) {
                fn arb(r: &mut Rng) -> Option<(Self, String)> {
                    $(let ($t, $j) = $t::arb(r)?;)+
                    Some((($($t,)+), format!("[{}]", [$($j),+].join(", "))))
                }
            }
            impl<$($t: Show),+> Show for ($($t,)+) {
                fn show(&self) -> String {
                    format!("({})", [$(self.$i.show()),+].join(", "))
                }
            }
        };
    }
    tuple!(A 0 ja);
    tuple!(A 0 ja, B 1 jb);
    tuple!(A 0 ja, B 1 jb, C 2 jc);
    tuple!(A 0 ja, B 1 jb, C 2 jc, D 3 jd);
    tuple!(A 0 ja, B 1 jb, C 2 jc, D 3 jd, E 4 je);

    /// One function under test: its TS name, and a call on drawn arguments
    /// that gives the TS arguments and the printed result.
    pub struct Case {
        pub key: &'static str,
        pub run: fn(&mut Rng) -> Option<(String, String)>,
    }

    fn message(p: Box<dyn std::any::Any + Send>) -> String {
        p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
    }

    /// `f`'s result printed, or `panic(message)`.
    pub fn call<T: Show>(f: impl FnOnce() -> T) -> String {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(v) => v.show(),
            Err(p) => format!("panic({})", message(p)),
        }
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
"##;
