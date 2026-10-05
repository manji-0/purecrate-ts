//! What the printers share while printing one crate, which none of them
//! is handed: its closed structs, internal and private names, the
//! temporaries made so far, and the loops and tables of the file being
//! printed.

use super::*;

/// The printer's temporaries start with `$`, which no Rust identifier has,
/// and end in `_<n>`, a count of them, so no two are one name before
/// `plain::plain_names` gives each a plain name that no other in an
/// overlapping block has. The final text does not depend on the count.
pub(super) fn temp(base: &str) -> String {
    let n = TEMPS.with(|t| {
        let n = t.get() + 1;
        t.set(n);
        n
    });
    format!("${base}_{n}")
}

/// A discriminant table: its enum and whether it holds `bigint`s, then its
/// name and declaration.
type Table = ((String, bool), String, String);

thread_local! {
    /// Temporaries made so far (`temp`).
    pub(crate) static TEMPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Closed structs of the crate `emit` is printing (design/01 §4). The
    /// expression printer has no `Crate`; `emit` sets this for its duration.
    pub(crate) static CLOSED: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// Structs of the crate, likewise: a place of one needs no `as` to
    /// undo a narrowing (`stmt::emit_let`), since a struct is no union.
    pub(crate) static STRUCTS: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// Each enum of the crate and its variants, likewise: a `match`'s `_`
    /// holds the variants no earlier arm took (`join::flow`).
    pub(crate) static ENUMS: RefCell<BTreeMap<String, Vec<Name>>> = const { RefCell::new(BTreeMap::new()) };
    /// The crate's open structs and enums that hold a string, in a field or
    /// in one of theirs (`stringy_names`), likewise: a `pub` function checks
    /// such an argument on entry (`expr::checked_strings`).
    pub(crate) static STRINGY: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// The crate's internal names (`internal_names`), likewise.
    pub(crate) static INTERNAL: RefCell<BTreeMap<Internal, String>> = const { RefCell::new(BTreeMap::new()) };
    /// Methods that are not `pub`, as (type, method), likewise.
    pub(crate) static PRIVATE: RefCell<BTreeSet<(String, String)>> = const { RefCell::new(BTreeSet::new()) };
    /// The locals of the function being printed that `push` grows: the one
    /// array the output writes, typed `Array<T>` (design/01 §7.14).
    pub(crate) static PUSHED: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// Helpers printed in another item's file (`Crate::homes`), likewise.
    pub(crate) static HOSTED: RefCell<BTreeMap<String, String>> = const { RefCell::new(BTreeMap::new()) };
    /// Whether the statement `stmt::emit_stmts` prints next is the last of
    /// its JS block, so nothing after it could meet a name it declares.
    pub(crate) static TAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The discriminant tables the file being printed reads (`expr::table`),
    /// each keyed by its enum and whether it holds `bigint`s, with its name
    /// and declaration; printed after the file's imports.
    pub(crate) static TABLES: RefCell<Vec<Table>> = const { RefCell::new(Vec::new()) };
    /// The loops being printed, innermost last, each with its label when a
    /// `break` or `continue` in it leaves it.
    pub(crate) static LOOPS: RefCell<Vec<Option<String>>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn is_closed(name: &str) -> bool {
    CLOSED.with(|c| c.borrow().contains(name))
}

pub(super) fn is_struct(name: &str) -> bool {
    STRUCTS.with(|c| c.borrow().contains(name))
}

/// Whether a value of `ty` may hold a string a TS caller wrote: a closed
/// type's came through its own `pub` functions, checked there.
pub(crate) fn holds_string(ty: &Ty) -> bool {
    match ty {
        Ty::Prim(Prim::String | Prim::Str) => true,
        Ty::Named(n) => STRINGY.with(|s| s.borrow().contains(n.as_str())),
        Ty::Fn { .. } => false,
        t => t.children().into_iter().any(holds_string),
    }
}

/// The open structs and enums of `krate` that hold a string, to a fixed
/// point, since one may hold another.
pub(super) fn stringy_names(krate: &Crate) -> BTreeSet<String> {
    let fields = |item: &Item| -> Option<(String, Vec<Ty>)> {
        match item {
            Item::Struct(st) if !st.closed => {
                Some((st.name.as_str().to_string(), st.fields.iter().map(|f| f.ty.clone()).collect()))
            }
            Item::Enum(e) => Some((
                e.name.as_str().to_string(),
                e.variants
                    .iter()
                    .flat_map(|v| match &v.fields {
                        VariantFields::Unit => Vec::new(),
                        VariantFields::Tuple(ts) => ts.clone(),
                        VariantFields::Struct(fs) => fs.iter().map(|f| f.ty.clone()).collect(),
                    })
                    .collect(),
            )),
            _ => None,
        }
    };
    let types: Vec<(String, Vec<Ty>)> = krate.items.iter().filter_map(fields).collect();
    let mut found = BTreeSet::new();
    loop {
        let next: BTreeSet<String> = scoped(&STRINGY, found.clone(), || {
            types.iter().filter(|(_, tys)| tys.iter().any(holds_string)).map(|(n, _)| n.clone()).collect()
        });
        if next == found {
            return found;
        }
        found = next;
    }
}

pub(super) fn enum_variants(ty: &str) -> Option<Vec<Name>> {
    ENUMS.with(|e| e.borrow().get(ty).cloned())
}

pub(super) fn is_private_method(ty: &str, name: &str) -> bool {
    PRIVATE.with(|p| p.borrow().contains(&(ty.to_string(), name.to_string())))
}

/// A method that is not `pub` in Rust stays off the exported companion: TS
/// callers outside the package must not reach what Rust callers cannot (a
/// private `fn raw(v) -> Yen` would build a closed type unchecked). Like a
/// closed struct's constructor, it is exported from the type's file but not
/// from `index.ts`, as `yenRaw` (`internal_names`).
pub(crate) fn private_method(ty: &str, name: &str) -> String {
    internal_name(&Internal::Method(ty.to_string(), name.to_string()))
}

/// A name the generated files share that callers do not see.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Internal {
    /// A closed struct's constructor.
    Ctor(String),
    /// A method that is not `pub`: the type and the method.
    Method(String, String),
}

fn internal_name(key: &Internal) -> String {
    INTERNAL.with(|m| m.borrow().get(key).cloned()).unwrap_or_else(|| internal_base(key))
}

fn internal_base(key: &Internal) -> String {
    match key {
        Internal::Ctor(ty) => format!("unsafeMake{ty}"),
        Internal::Method(ty, m) => format!("{}{}", purecrate_ir::lower_first(ty), purecrate_ir::upper_first(m)),
    }
}

/// The internal names of a crate, each told apart from the crate's own
/// top-level names and from the others by a number where they meet
/// (`unsafeMakeYen2` beside a `fn unsafe_make_yen`).
pub(crate) fn internal_names(krate: &Crate) -> BTreeMap<Internal, String> {
    let mut taken: BTreeSet<String> = krate
        .items
        .iter()
        .filter(|item| !matches!(item, Item::Fn(f) if f.owner.is_some()))
        .map(|item| item.name().as_str().to_string())
        .collect();
    let keys = closed_names(krate)
        .into_iter()
        .map(Internal::Ctor)
        .chain(private_methods(krate).into_iter().map(|(t, m)| Internal::Method(t, m)));
    let mut out = BTreeMap::new();
    for key in keys {
        let base = internal_base(&key);
        let name = (1..)
            .map(|n| if n == 1 { base.clone() } else { format!("{base}{n}") })
            .find(|n| !taken.contains(n))
            .expect("a free name");
        taken.insert(name.clone());
        out.insert(key, name);
    }
    out
}

/// Runs `f` with the crate's internal names in place for `closed_ctor` and
/// `private_method`.
pub(crate) fn with_internal_names<T>(krate: &Crate, f: impl FnOnce() -> T) -> T {
    scoped(&INTERNAL, internal_names(krate), f)
}

/// Runs `f` with `key` holding `value`, then puts back what it held.
pub(crate) fn scoped<V: 'static, T>(
    key: &'static std::thread::LocalKey<RefCell<V>>,
    value: V,
    f: impl FnOnce() -> T,
) -> T {
    let previous = key.with(|k| k.replace(value));
    let out = f();
    key.with(|k| k.replace(previous));
    out
}

pub(super) fn private_methods(krate: &Crate) -> BTreeSet<(String, String)> {
    krate
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) if f.vis == Vis::Internal => {
                f.owner.as_ref().map(|o| (o.as_str().to_string(), f.name.as_str().to_string()))
            }
            _ => None,
        })
        .collect()
}

/// The package-internal constructor of a closed struct (`unsafeMakeYen`).
/// It is exported from the type's file for the other generated files, but
/// not from `index.ts`, and the package's `exports` reach no other file.
pub(crate) fn closed_ctor(name: &str) -> String {
    internal_name(&Internal::Ctor(name.to_string()))
}

pub(crate) fn closed_names(krate: &Crate) -> BTreeSet<String> {
    krate
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Struct(st) if st.closed => Some(st.name.as_str().to_string()),
            _ => None,
        })
        .collect()
}
