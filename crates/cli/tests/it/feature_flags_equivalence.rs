//! `examples/flags`, flagd's flag evaluation (open-feature/flagd at
//! 670c91cdca, core/pkg/evaluator) with OpenFeature's reasons and codes:
//!
//! - the published values come out as published: MurmurHash3 x86_32 on
//!   the reference implementation's vectors (smhasher's MurmurHash3.cpp,
//!   compiled and run to confirm each); flagd's own `fractional` cases
//!   (fractional_test.go: four e-mails under `cat` with the flag key and
//!   with a custom seed, a bare `var`, the literal bucketing value, an
//!   uneven split, the targeting-key default, missing / empty targeting
//!   keys, a single entry); flagd's `sem_ver` cases (semver_test.go) and
//!   SemVer 2.0.0 §11's precedence chain; the resolution rules of json.go
//!   (STATIC, DEFAULT on null, an unknown variant as GENERAL, not found);
//! - the same rules in idiomatic Rust (`idiomatic`, the line count design/07
//!   §2 compares against: `HashMap`, `from_le_bytes`, `rotate_left`,
//!   `chunks_exact`, methods on the rule types) agree on: MurmurHash3 over
//!   every length 0..=40 and random bytes; `sem_ver` over every pair of a
//!   pool of valid and invalid versions with every operator; random rule
//!   trees, contexts, and flags through `resolve`; random weights through
//!   `Split::new` and `split_variant`; random flags through `Flag::new`;
//! - the generated package agrees with Rust on a sample of each.
//!
//! The file is not `flags_equivalence.rs`: that name is the `const` and
//! discriminant test's (`fixtures/flags.rs`).

use std::collections::BTreeMap;

use crate::support::{self, Js, Rng};

purecrate_canon::fixture!(mod flags = "../../../examples/flags/src/lib.rs", "fixtures/flags_driver.rs");

use flags::{
    Attr, BucketBy, CmpOp, Cond, Context, DefinitionError, ErrorCode, Flag, FlagSet, FlagValue, Part, Reason,
    Resolution, Rule, SemVerOp, Split, State,
};

/// flagd's evaluation as one would write it without constraint. Types carry
/// the constrained side's names, so `Debug` compares results.
mod idiomatic {
    use std::cmp::Ordering;
    use std::collections::HashMap;
    use std::mem::discriminant;

    pub use super::flags::{CmpOp, DefinitionError, ErrorCode, Reason, SemVerOp, State};

    pub fn murmur3_32(data: &[u8], seed: u32) -> u32 {
        const C1: u32 = 0xcc9e_2d51;
        const C2: u32 = 0x1b87_3593;
        let mix = |k: u32| k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        let mut h = seed;
        let (blocks, tail) = data.as_chunks::<4>();
        for block in blocks {
            h ^= mix(u32::from_le_bytes(*block));
            h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
        }
        if !tail.is_empty() {
            h ^= mix(tail.iter().rev().fold(0, |k, &b| k << 8 | u32::from(b)));
        }
        h ^= data.len() as u32;
        h ^= h >> 16;
        h = h.wrapping_mul(0x85eb_ca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2_ae35);
        h ^ (h >> 16)
    }

    struct Version<'a> {
        core: [&'a str; 3],
        pre: Vec<&'a str>,
    }

    fn numeric(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }

    fn ident(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }

    fn number(s: &str) -> bool {
        numeric(s) && (s == "0" || !s.starts_with('0'))
    }

    fn parse(text: &str) -> Option<Version<'_>> {
        let s = text.strip_prefix(['v', 'V']).unwrap_or(text);
        let (s, build) = s.split_once('+').map_or((s, None), |(a, b)| (a, Some(b)));
        let (core, pre) = s.split_once('-').map_or((s, None), |(a, p)| (a, Some(p)));
        let parts: Vec<&str> = core.split('.').collect();
        let full = parts.len() == 3;
        if parts.len() > 3 || !parts.iter().all(|p| number(p)) || (!full && (pre.is_some() || build.is_some())) {
            return None;
        }
        if build.is_some_and(|b| !b.split('.').all(ident)) {
            return None;
        }
        let pre: Vec<&str> = pre.map_or(vec![], |p| p.split('.').collect());
        if !pre.iter().all(|p| ident(p) && (!numeric(p) || number(p))) {
            return None;
        }
        let at = |i: usize| parts.get(i).copied().unwrap_or("0");
        Some(Version { core: [at(0), at(1), at(2)], pre })
    }

    fn by_digits(a: &str, b: &str) -> Ordering {
        a.len().cmp(&b.len()).then_with(|| a.cmp(b))
    }

    fn compare(a: &Version, b: &Version) -> Ordering {
        let core = a.core.iter().zip(&b.core).map(|(x, y)| by_digits(x, y)).find(|o| o.is_ne());
        core.unwrap_or(match (a.pre.is_empty(), b.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                let ids = a.pre.iter().zip(&b.pre).map(|(x, y)| match (numeric(x), numeric(y)) {
                    (true, true) => by_digits(x, y),
                    (x_num, y_num) if x_num != y_num => y_num.cmp(&x_num),
                    _ => x.cmp(y),
                });
                ids.into_iter().find(|o| o.is_ne()).unwrap_or(a.pre.len().cmp(&b.pre.len()))
            }
        })
    }

    pub fn sem_ver_matches(actual: &str, op: SemVerOp, target: &str) -> bool {
        let (Some(a), Some(b)) = (parse(actual), parse(target)) else { return false };
        let o = compare(&a, &b);
        match op {
            SemVerOp::Eq => o.is_eq(),
            SemVerOp::Ne => o.is_ne(),
            SemVerOp::Lt => o.is_lt(),
            SemVerOp::Le => o.is_le(),
            SemVerOp::Gt => o.is_gt(),
            SemVerOp::Ge => o.is_ge(),
            SemVerOp::Caret => a.core[0] == b.core[0],
            SemVerOp::Tilde => a.core[..2] == b.core[..2],
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum FlagValue {
        Bool(bool),
        String(String),
        Integer(i64),
        Float(f64),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Attr {
        String(String),
        Number(f64),
        Bool(bool),
    }

    #[derive(Debug, Clone)]
    pub struct Context {
        pub targeting_key: Option<String>,
        pub attributes: HashMap<String, Attr>,
    }

    impl Context {
        fn get(&self, flag_key: &str, name: &str) -> Option<Attr> {
            match name {
                "targetingKey" => self.targeting_key.clone().map(Attr::String),
                "$flagd.flagKey" => Some(Attr::String(flag_key.to_string())),
                _ => self.attributes.get(name).cloned(),
            }
        }

        fn get_str(&self, flag_key: &str, name: &str) -> Option<String> {
            match self.get(flag_key, name)? {
                Attr::String(s) => Some(s),
                _ => None,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum Cond {
        Compare { attr: String, op: CmpOp, value: Attr },
        In { attr: String, list: Vec<String> },
        Contains { attr: String, needle: String },
        StartsWith { attr: String, prefix: String },
        EndsWith { attr: String, suffix: String },
        SemVer { attr: String, op: SemVerOp, version: String },
        And(Vec<Cond>),
        Or(Vec<Cond>),
        Not(Box<Cond>),
    }

    impl Cond {
        fn holds(&self, ctx: &Context, key: &str) -> bool {
            let text = |attr: &str, test: &dyn Fn(&str) -> bool| ctx.get_str(key, attr).is_some_and(|s| test(&s));
            match self {
                Cond::Compare { attr, op, value } => {
                    let found = ctx.get(key, attr);
                    match (op, found, value) {
                        (CmpOp::Eq, found, value) => found.as_ref() == Some(value),
                        (CmpOp::Ne, found, value) => found.as_ref() != Some(value),
                        (op, Some(Attr::Number(a)), Attr::Number(b)) => match op {
                            CmpOp::Lt => a < *b,
                            CmpOp::Le => a <= *b,
                            CmpOp::Gt => a > *b,
                            _ => a >= *b,
                        },
                        _ => false,
                    }
                }
                Cond::In { attr, list } => text(attr, &|s| list.iter().any(|x| x == s)),
                Cond::Contains { attr, needle } => text(attr, &|s| s.contains(needle.as_str())),
                Cond::StartsWith { attr, prefix } => text(attr, &|s| s.starts_with(prefix.as_str())),
                Cond::EndsWith { attr, suffix } => text(attr, &|s| s.ends_with(suffix.as_str())),
                Cond::SemVer { attr, op, version } => text(attr, &|s| sem_ver_matches(s, *op, version)),
                Cond::And(cs) => cs.iter().all(|c| c.holds(ctx, key)),
                Cond::Or(cs) => cs.iter().any(|c| c.holds(ctx, key)),
                Cond::Not(c) => !c.holds(ctx, key),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum Part {
        Literal(String),
        Attr(String),
    }

    #[derive(Debug, Clone)]
    pub enum BucketBy {
        Default,
        Literal(String),
        Attr(String),
        Cat(Vec<Part>),
    }

    impl BucketBy {
        /// `None` is flagd's null.
        fn key(&self, ctx: &Context, flag_key: &str) -> Option<String> {
            match self {
                BucketBy::Default => {
                    ctx.targeting_key.as_deref().filter(|t| !t.is_empty()).map(|t| format!("{flag_key}{t}"))
                }
                BucketBy::Literal(s) => Some(s.clone()),
                BucketBy::Attr(name) => match ctx.get(flag_key, name) {
                    None => BucketBy::Default.key(ctx, flag_key),
                    Some(Attr::String(s)) => Some(s),
                    Some(_) => None,
                },
                BucketBy::Cat(parts) => {
                    let pieces = parts.iter().map(|p| match p {
                        Part::Literal(s) => Some(s.clone()),
                        Part::Attr(name) => ctx.get_str(flag_key, name),
                    });
                    let joined = pieces.collect::<Option<String>>()?;
                    Some(if parts.len() >= 2 { joined.trim().to_string() } else { joined })
                }
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Split {
        pub bucket_by: BucketBy,
        pub shares: Vec<(String, u32)>,
    }

    impl Split {
        pub fn new(bucket_by: BucketBy, weights: Vec<(String, i64)>) -> Result<Split, DefinitionError> {
            let max = i64::from(i32::MAX);
            if weights.is_empty() {
                return Err(DefinitionError::EmptySplit);
            }
            if weights.iter().any(|(_, w)| *w > max) || weights.iter().map(|(_, w)| w.max(&0)).sum::<i64>() > max {
                return Err(DefinitionError::WeightTooLarge);
            }
            let shares = weights.into_iter().map(|(v, w)| (v, w.max(0) as u32)).collect();
            Ok(Split { bucket_by, shares })
        }

        pub fn variant(&self, ctx: &Context, flag_key: &str) -> Option<String> {
            let key = self.bucket_by.key(ctx, flag_key)?;
            let total: u64 = self.shares.iter().map(|(_, w)| u64::from(*w)).sum();
            if total == 0 {
                return None;
            }
            let bucket = (u64::from(murmur3_32(key.as_bytes(), 0)) * total) >> 32;
            let mut end = 0;
            self.shares
                .iter()
                .find(|(_, w)| {
                    end += u64::from(*w);
                    bucket < end
                })
                .map(|(v, _)| v.clone())
        }
    }

    #[derive(Debug, Clone)]
    pub enum Rule {
        Variant(String),
        Null,
        If(Box<Cond>, Box<Rule>, Box<Rule>),
        Fractional(Split),
    }

    pub enum Outcome {
        Matched(String),
        Split(String),
        Null,
    }

    impl Rule {
        fn run(&self, ctx: &Context, key: &str) -> Outcome {
            match self {
                Rule::Variant(v) => Outcome::Matched(v.clone()),
                Rule::Null => Outcome::Null,
                Rule::If(c, then, otherwise) => if c.holds(ctx, key) { then } else { otherwise }.run(ctx, key),
                Rule::Fractional(s) => s.variant(ctx, key).map_or(Outcome::Null, Outcome::Split),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Flag {
        pub key: String,
        pub state: State,
        pub variants: HashMap<String, FlagValue>,
        pub default_variant: String,
        pub targeting: Option<Rule>,
    }

    impl Flag {
        pub fn new(
            key: String,
            state: State,
            variants: HashMap<String, FlagValue>,
            default_variant: String,
            targeting: Option<Rule>,
        ) -> Result<Flag, DefinitionError> {
            let chosen = variants.get(&default_variant).ok_or(DefinitionError::UnknownDefaultVariant)?;
            if variants.values().any(|v| discriminant(v) != discriminant(chosen)) {
                return Err(DefinitionError::MixedVariantTypes);
            }
            Ok(Flag { key, state, variants, default_variant, targeting })
        }
    }

    pub struct FlagSet(HashMap<String, Flag>);

    impl FlagSet {
        pub fn new(flags: Vec<Flag>) -> Result<FlagSet, DefinitionError> {
            let mut map = HashMap::new();
            for flag in flags {
                if map.insert(flag.key.clone(), flag).is_some() {
                    return Err(DefinitionError::DuplicateKey);
                }
            }
            Ok(FlagSet(map))
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Resolution {
        pub value: FlagValue,
        pub variant: Option<String>,
        pub reason: Reason,
        pub error_code: Option<ErrorCode>,
    }

    pub fn resolve(flags: &FlagSet, key: &str, ctx: &Context, fallback: FlagValue) -> Resolution {
        let fail = |value, code| Resolution { value, variant: None, reason: Reason::Error, error_code: Some(code) };
        let Some(flag) = flags.0.get(key) else { return fail(fallback, ErrorCode::FlagNotFound) };
        if flag.state == State::Disabled {
            return Resolution { value: fallback, variant: None, reason: Reason::Disabled, error_code: None };
        }
        let (variant, reason) = match flag.targeting.as_ref().map(|r| r.run(ctx, key)) {
            None => (flag.default_variant.clone(), Reason::Static),
            Some(Outcome::Matched(v)) => (v, Reason::TargetingMatch),
            Some(Outcome::Split(v)) => (v, Reason::Split),
            Some(Outcome::Null) => (flag.default_variant.clone(), Reason::Default),
        };
        let Some(value) = flag.variants.get(&variant) else { return fail(fallback, ErrorCode::General) };
        let value = match (value, &fallback) {
            (FlagValue::Integer(n), FlagValue::Float(_)) => FlagValue::Float(*n as f64),
            (v, f) if discriminant(v) == discriminant(f) => v.clone(),
            _ => return fail(fallback, ErrorCode::TypeMismatch),
        };
        Resolution { value, variant: Some(variant), reason, error_code: None }
    }
}

// ---------------------------------------------------------------------------
// Building both sides from one description
// ---------------------------------------------------------------------------

fn s(text: &str) -> String {
    text.to_string()
}

fn ctx(targeting_key: Option<&str>, attributes: &[(&str, Attr)]) -> Context {
    Context {
        targeting_key: targeting_key.map(s),
        attributes: attributes.iter().map(|(k, v)| (s(k), v.clone())).collect(),
    }
}

fn i_attr(a: &Attr) -> idiomatic::Attr {
    match a {
        Attr::String(x) => idiomatic::Attr::String(x.clone()),
        Attr::Number(x) => idiomatic::Attr::Number(*x),
        Attr::Bool(x) => idiomatic::Attr::Bool(*x),
    }
}

fn i_ctx(c: &Context) -> idiomatic::Context {
    idiomatic::Context {
        targeting_key: c.targeting_key.clone(),
        attributes: c.attributes.iter().map(|(k, v)| (k.clone(), i_attr(v))).collect(),
    }
}

fn i_value(v: &FlagValue) -> idiomatic::FlagValue {
    match v {
        FlagValue::Bool(x) => idiomatic::FlagValue::Bool(*x),
        FlagValue::String(x) => idiomatic::FlagValue::String(x.clone()),
        FlagValue::Integer(x) => idiomatic::FlagValue::Integer(*x),
        FlagValue::Float(x) => idiomatic::FlagValue::Float(*x),
    }
}

fn i_cond(c: &Cond) -> idiomatic::Cond {
    use idiomatic::Cond as I;
    match c {
        Cond::Compare { attr, op, value } => I::Compare { attr: attr.clone(), op: *op, value: i_attr(value) },
        Cond::In { attr, list } => I::In { attr: attr.clone(), list: list.clone() },
        Cond::Contains { attr, needle } => I::Contains { attr: attr.clone(), needle: needle.clone() },
        Cond::StartsWith { attr, prefix } => I::StartsWith { attr: attr.clone(), prefix: prefix.clone() },
        Cond::EndsWith { attr, suffix } => I::EndsWith { attr: attr.clone(), suffix: suffix.clone() },
        Cond::SemVer { attr, op, version } => I::SemVer { attr: attr.clone(), op: *op, version: version.clone() },
        Cond::And(cs) => I::And(cs.iter().map(i_cond).collect()),
        Cond::Or(cs) => I::Or(cs.iter().map(i_cond).collect()),
        Cond::Not(c) => I::Not(Box::new(i_cond(c))),
    }
}

fn i_bucket(b: &BucketBy) -> idiomatic::BucketBy {
    use idiomatic::BucketBy as I;
    match b {
        BucketBy::Default => I::Default,
        BucketBy::Literal(x) => I::Literal(x.clone()),
        BucketBy::Attr(x) => I::Attr(x.clone()),
        BucketBy::Cat(parts) => I::Cat(
            parts
                .iter()
                .map(|p| match p {
                    Part::Literal(x) => idiomatic::Part::Literal(x.clone()),
                    Part::Attr(x) => idiomatic::Part::Attr(x.clone()),
                })
                .collect(),
        ),
    }
}

/// A rule as a description: `Split`s carry their weights, as both sides'
/// constructors take them.
#[derive(Debug, Clone)]
enum R {
    Variant(String),
    Null,
    If(Cond, Box<R>, Box<R>),
    Fractional(BucketBy, Vec<(String, i64)>),
}

fn rule(r: &R) -> Rule {
    match r {
        R::Variant(v) => Rule::Variant(v.clone()),
        R::Null => Rule::Null,
        R::If(c, a, b) => Rule::If(Box::new(c.clone()), Box::new(rule(a)), Box::new(rule(b))),
        R::Fractional(by, w) => Rule::Fractional(Split::new(by.clone(), w.clone()).expect("valid split")),
    }
}

fn i_rule(r: &R) -> idiomatic::Rule {
    match r {
        R::Variant(v) => idiomatic::Rule::Variant(v.clone()),
        R::Null => idiomatic::Rule::Null,
        R::If(c, a, b) => idiomatic::Rule::If(Box::new(i_cond(c)), Box::new(i_rule(a)), Box::new(i_rule(b))),
        R::Fractional(by, w) => {
            idiomatic::Rule::Fractional(idiomatic::Split::new(i_bucket(by), w.clone()).expect("valid split"))
        }
    }
}

/// A flag as a description.
#[derive(Debug, Clone)]
struct F {
    key: String,
    state: State,
    variants: Vec<(String, FlagValue)>,
    default_variant: String,
    targeting: Option<R>,
}

fn flag(f: &F) -> Result<Flag, DefinitionError> {
    let variants: BTreeMap<String, FlagValue> = f.variants.iter().cloned().collect();
    Flag::new(f.key.clone(), f.state, variants, f.default_variant.clone(), f.targeting.as_ref().map(rule))
}

fn i_flag(f: &F) -> Result<idiomatic::Flag, DefinitionError> {
    let variants = f.variants.iter().map(|(k, v)| (k.clone(), i_value(v))).collect();
    idiomatic::Flag::new(f.key.clone(), f.state, variants, f.default_variant.clone(), f.targeting.as_ref().map(i_rule))
}

fn flag_set(fs: &[F]) -> FlagSet {
    FlagSet::new(fs.iter().map(|f| flag(f).expect("valid flag")).collect()).expect("unique keys")
}

fn i_flag_set(fs: &[F]) -> idiomatic::FlagSet {
    idiomatic::FlagSet::new(fs.iter().map(|f| i_flag(f).expect("valid flag")).collect()).expect("unique keys")
}

fn colors() -> Vec<(String, FlagValue)> {
    ["red", "blue", "green", "yellow"]
        .iter()
        .zip(["#FF0000", "#0000FF", "#00FF00", "#FFFF00"])
        .map(|(n, hex)| (s(n), FlagValue::String(s(hex))))
        .collect()
}

fn color_flag(key: &str, targeting: R) -> F {
    F { key: s(key), state: State::Enabled, variants: colors(), default_variant: s("red"), targeting: Some(targeting) }
}

fn weights(ws: &[(&str, i64)]) -> Vec<(String, i64)> {
    ws.iter().map(|(v, w)| (s(v), *w)).collect()
}

fn quarters() -> Vec<(String, i64)> {
    weights(&[("red", 25), ("blue", 25), ("green", 25), ("yellow", 25)])
}

fn faas(then: R) -> R {
    let cond = Cond::Contains { attr: s("email"), needle: s("@faas.com") };
    R::If(cond, Box::new(then), Box::new(R::Null))
}

fn email(e: &str) -> Context {
    ctx(None, &[("email", Attr::String(s(e)))])
}

fn resolution(value: &str, variant: &str, reason: Reason) -> Resolution {
    Resolution { value: FlagValue::String(s(value)), variant: Some(s(variant)), reason, error_code: None }
}

fn hex(variant: &str) -> &'static str {
    match variant {
        "red" => "#FF0000",
        "blue" => "#0000FF",
        "green" => "#00FF00",
        _ => "#FFFF00",
    }
}

/// The published cases, each a flag set, a key, a context, and the variant
/// and reason flagd's test names (fractional picks are SPLIT here, where
/// flagd says TARGETING_MATCH; see the example's header).
fn published() -> Vec<(Vec<F>, &'static str, Context, &'static str, Reason)> {
    let flag_key_cat = BucketBy::Cat(vec![Part::Attr(s("$flagd.flagKey")), Part::Attr(s("email"))]);
    let seeded = BucketBy::Cat(vec![Part::Literal(s("my-seed")), Part::Attr(s("email"))]);
    let common = vec![
        color_flag("headerColor", faas(R::Fractional(flag_key_cat, quarters()))),
        color_flag("customSeededHeaderColor", faas(R::Fractional(seeded, quarters()))),
    ];
    let one = |f: F| vec![f];
    let mut out = vec![];
    for (e, v) in [("rachel", "blue"), ("monica", "yellow"), ("joey", "red"), ("ross", "blue")] {
        out.push((common.clone(), "headerColor", email(&format!("{e}@faas.com")), v, Reason::Split));
    }
    for (e, v) in [("rachel", "green"), ("monica", "red"), ("joey", "blue"), ("ross", "green")] {
        out.push((common.clone(), "customSeededHeaderColor", email(&format!("{e}@faas.com")), v, Reason::Split));
    }
    let by_email = BucketBy::Attr(s("email"));
    let footer = color_flag("footerColor", faas(R::Fractional(by_email.clone(), quarters())));
    out.push((one(footer), "footerColor", email("ross@faas.com"), "red", Reason::Split));
    let uneven = weights(&[("red", 50), ("blue", 25), ("green", 25)]);
    let literal = color_flag("headerColor", faas(R::Fractional(BucketBy::Literal(s("email")), uneven)));
    out.push((one(literal), "headerColor", email("test4@faas.com"), "green", Reason::Split));
    let no_email = color_flag("headerColor", R::Fractional(by_email.clone(), quarters()));
    out.push((one(no_email), "headerColor", ctx(None, &[]), "red", Reason::Default));
    let halves = color_flag("headerColor", R::Fractional(by_email.clone(), weights(&[("red", 25), ("blue", 25)])));
    out.push((one(halves), "headerColor", email("foo@foo.com"), "blue", Reason::Split));
    let unweighted = color_flag("headerColor", R::Fractional(by_email.clone(), weights(&[("red", 1), ("blue", 1)])));
    out.push((one(unweighted), "headerColor", email("foo@foo.com"), "blue", Reason::Split));
    let by_key = color_flag("headerColor", R::Fractional(BucketBy::Default, weights(&[("blue", 50), ("green", 50)])));
    out.push((one(by_key.clone()), "headerColor", ctx(Some("foo@foo.com"), &[]), "green", Reason::Split));
    out.push((one(by_key.clone()), "headerColor", ctx(None, &[]), "red", Reason::Default));
    out.push((one(by_key), "headerColor", ctx(Some(""), &[]), "red", Reason::Default));
    let fallback = color_flag("headerColor", R::Fractional(by_email, weights(&[("red", 50), ("blue", 50)])));
    out.push((one(fallback), "headerColor", ctx(Some("foo@foo.com"), &[]), "blue", Reason::Split));
    let single = color_flag("headerColor", R::Fractional(BucketBy::Default, weights(&[("blue", 1)])));
    out.push((one(single), "headerColor", ctx(Some("any-user"), &[]), "blue", Reason::Split));
    let single = color_flag("headerColor", R::Fractional(BucketBy::Attr(s("email")), weights(&[("green", 100)])));
    out.push((one(single), "headerColor", email("any@user.com"), "green", Reason::Split));
    // semver_test.go: `sem_ver` on an attribute and on literals.
    let sem = |attr: &str, op, v: &str| Cond::SemVer { attr: s(attr), op, version: s(v) };
    let if_red =
        |c: Cond| color_flag("headerColor", R::If(c, Box::new(R::Variant(s("red"))), Box::new(R::Variant(s("green")))));
    let version = |v: &str| ctx(None, &[("version", Attr::String(s(v)))]);
    for (v, op, target, want) in [
        ("1.0.1", SemVerOp::Gt, "1.0.0", "red"),
        ("0.0.1", SemVerOp::Gt, "1.0.0", "green"),
        ("v1.0.0", SemVerOp::Lt, "v1.2", "red"),
        ("not-a-version", SemVerOp::Eq, "1.0.0", "green"),
        ("1.2.3", SemVerOp::Caret, "1.5.6", "red"),
        ("2.2.3", SemVerOp::Caret, "1.2.3", "green"),
        ("1.2.3", SemVerOp::Tilde, "1.2.6", "red"),
        ("1.3.3", SemVerOp::Tilde, "1.2.6", "green"),
        ("invalid", SemVerOp::Gt, "1.0.0", "green"),
        ("1.0.0", SemVerOp::Gt, "invalid", "green"),
    ] {
        out.push((vec![if_red(sem("version", op, target))], "headerColor", version(v), want, Reason::TargetingMatch));
    }
    out
}

#[test]
fn the_published_values_come_out_as_published() {
    // smhasher's MurmurHash3_x86_32, run on each input.
    for (text, seed, want) in [
        ("", 0, 0u32),
        ("", 1, 0x514e_28b7),
        ("hello", 0, 0x248b_fa47),
        ("Hello, world!", 0, 0xc036_3e43),
        ("Hello, world!", 1234, 0xfaf6_cdb3),
        ("a", 0, 0x3c25_69b2),
        ("ab", 0, 0x9bbf_d75f),
        ("abc", 0, 0xb3dd_93fa),
        ("abcd", 0, 0x43ed_676a),
        ("abcde", 0, 0xe89b_9af6),
        ("The quick brown fox jumps over the lazy dog", 0, 0x2e4f_f723),
        ("headerColorrachel@faas.com", 0, 0x6b9f_c846),
    ] {
        assert_eq!(flags::murmur3_32(text.as_bytes(), seed), want, "{text:?} seed {seed}");
        assert_eq!(idiomatic::murmur3_32(text.as_bytes(), seed), want, "{text:?} seed {seed}");
    }
    for (flags_, key, context, variant, reason) in published() {
        let got = flags::resolve(&flag_set(&flags_), key, &context, FlagValue::String(s("fallback")));
        assert_eq!(got, resolution(hex(variant), variant, reason), "{key} {context:?}");
    }
    // semver_test.go's operator table (`v` and `V` optional).
    use SemVerOp::*;
    for (a, op, b, want) in [
        ("invalid", Gt, "v1.0.0", false),
        ("v1.0.0-preview.1.2", Gt, "v1.0.0", false),
        ("v1.0.0-preview.1.3", Gt, "v1.0.0-preview.1.2", true),
        ("0.0.1", Gt, "v0.0.2", false),
        ("v0.0.1", Eq, "0.0.1", true),
        ("V1.0.0", Eq, "1.0.0", true),
        ("v1234.0.1", Lt, "v1235.0.2", true),
        ("v1.0", Lt, "v1.2", true),
        ("v0.0.1", Lt, "v0.0.1", false),
        ("v0.0.1", Le, "v0.0.2", true),
        ("v0.0.1", Le, "v0.0.1", true),
        ("v0.0.2", Ne, "v0.0.1", true),
        ("v0.0.1", Ne, "v0.0.1", false),
        ("v0.0.2", Ge, "v0.0.1", true),
        ("v0.0.1", Ge, "v0.0.2", false),
        ("1.0.0", Gt, "1.0.0", false),
        ("1", Eq, "v1.0.0", true),
        ("1.2", Eq, "1.2", true),
        ("1.0.0+build.1", Eq, "1.0.0+build.2", true),
        ("01.0.0", Eq, "1.0.0", false),
        ("1.0.0-01", Lt, "1.0.0", false),
        ("1.2-rc.1", Lt, "1.2.0", false),
        ("99999999999999999999999.0.0", Gt, "99999999999999999999998.0.0", true),
    ] {
        assert_eq!(flags::sem_ver_matches(a, op, b), want, "{a} {op:?} {b}");
    }
    // SemVer 2.0.0 §11: each precedes the next.
    let chain = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
        "2.0.0",
        "2.1.0",
        "2.1.1",
    ];
    for (i, a) in chain.iter().enumerate() {
        for (j, b) in chain.iter().enumerate() {
            assert_eq!(flags::sem_ver_matches(a, Lt, b), i < j, "{a} < {b}");
            assert_eq!(flags::sem_ver_matches(a, Eq, b), i == j, "{a} = {b}");
        }
    }
    // json.go: STATIC without targeting, DEFAULT on null, GENERAL for an
    // unknown variant, FLAG_NOT_FOUND; OpenFeature: DISABLED, TYPE_MISMATCH.
    let static_flag = F { targeting: None, ..color_flag("static", R::Null) };
    let null_flag = color_flag("null", R::Null);
    let unknown = color_flag("unknown", R::Variant(s("purple")));
    let disabled = F { state: State::Disabled, ..color_flag("off", R::Variant(s("blue"))) };
    let number = F {
        key: s("number"),
        state: State::Enabled,
        variants: vec![(s("one"), FlagValue::Integer(1)), (s("two"), FlagValue::Integer(2))],
        default_variant: s("two"),
        targeting: None,
    };
    let set = flag_set(&[static_flag, null_flag, unknown, disabled, number]);
    let none = ctx(None, &[]);
    let text = FlagValue::String(s("fb"));
    let err =
        |value: FlagValue, code| Resolution { value, variant: None, reason: Reason::Error, error_code: Some(code) };
    assert_eq!(flags::resolve(&set, "static", &none, text.clone()), resolution("#FF0000", "red", Reason::Static));
    assert_eq!(flags::resolve(&set, "null", &none, text.clone()), resolution("#FF0000", "red", Reason::Default));
    assert_eq!(flags::resolve(&set, "unknown", &none, text.clone()), err(text.clone(), ErrorCode::General));
    assert_eq!(flags::resolve(&set, "nope", &none, text.clone()), err(text.clone(), ErrorCode::FlagNotFound));
    let off = Resolution { value: text.clone(), variant: None, reason: Reason::Disabled, error_code: None };
    assert_eq!(flags::resolve(&set, "off", &none, text.clone()), off);
    assert_eq!(flags::resolve(&set, "number", &none, text.clone()), err(text, ErrorCode::TypeMismatch));
    let float = flags::resolve(&set, "number", &none, FlagValue::Float(0.5));
    assert_eq!(float.value, FlagValue::Float(2.0));
    let int =
        flags::resolve(&flag_set(&[F { targeting: None, ..float_flag() }]), "float", &none, FlagValue::Integer(0));
    assert_eq!(int.error_code, Some(ErrorCode::TypeMismatch));
    // Weights: negative ones count as 0, the cap is i32::MAX each and summed.
    let big = |ws: &[(&str, i64)]| Split::new(BucketBy::Default, weights(ws)).map(|_| ());
    assert_eq!(big(&[("a", i64::from(i32::MAX))]), Ok(()));
    assert_eq!(big(&[("a", i64::from(i32::MAX) + 1)]), Err(DefinitionError::WeightTooLarge));
    assert_eq!(big(&[("a", i64::from(i32::MAX)), ("b", 1)]), Err(DefinitionError::WeightTooLarge));
    assert_eq!(big(&[("a", i64::from(i32::MAX)), ("b", -5)]), Ok(()));
    assert_eq!(big(&[]), Err(DefinitionError::EmptySplit));
    let zero = Split::new(BucketBy::Default, weights(&[("a", 0), ("b", -1)])).unwrap();
    assert_eq!(flags::split_variant(&zero, &ctx(Some("u"), &[]), "k"), None);
}

fn float_flag() -> F {
    F {
        key: s("float"),
        state: State::Enabled,
        variants: vec![(s("half"), FlagValue::Float(0.5))],
        default_variant: s("half"),
        targeting: None,
    }
}

// ---------------------------------------------------------------------------
// Generated inputs
// ---------------------------------------------------------------------------

const NAMES: &[&str] = &["email", "country", "age", "version", "beta", "targetingKey", "$flagd.flagKey", "absent"];

const TEXTS: &[&str] = &[
    "",
    " ",
    "rachel@faas.com",
    "joey@faas.com",
    "user@example.org",
    "us",
    "ca",
    "1.2.3",
    "v2.0.0-rc.1",
    "1.2",
    "01.2.3",
    "1.0.0+build",
    " padded ",
    "\u{a0}nbsp\u{3000}",
    "日本語",
    "emoji \u{1f600}",
    "headerColor",
];

const VERSIONS: &[&str] = &[
    "0.0.0",
    "1.0.0",
    "v1.0.0",
    "V1.0.0",
    "1",
    "1.2",
    "v1.2",
    "1.2.3",
    "1.2.10",
    "1.10.3",
    "2.0.0",
    "1.0.0-alpha",
    "1.0.0-alpha.1",
    "1.0.0-alpha.beta",
    "1.0.0-beta.11",
    "1.0.0-beta.2",
    "1.0.0-rc.1+build.5",
    "1.0.0+20130313144700",
    "1.0.0-0a",
    "1.0.0-01",
    "1.0.0-",
    "1.0.0+",
    "1.0.0-a..b",
    "1.2-rc",
    "01.0.0",
    "1.0.0.0",
    "",
    "v",
    "vv1.0.0",
    "Vv1.0.0",
    "1.0.0-x-y-z.--",
    "1.0.0+a_b",
    "18446744073709551616.0.0",
    "18446744073709551615.0.0",
    "1.0.0 ",
    "１.0.0",
];

fn text(rng: &mut Rng) -> String {
    s(rng.pick(TEXTS))
}

fn attr(rng: &mut Rng) -> Attr {
    match rng.below(6) {
        0 | 1 => Attr::String(text(rng)),
        2 => Attr::String(s(rng.pick(VERSIONS))),
        3 => Attr::Number(rng.pick(&[0.0, -0.0, 1.0, 1.5, 18.0, 65.0, -3.25, 1e21, f64::NAN, f64::INFINITY])),
        _ => Attr::Bool(rng.below(2) == 0),
    }
}

fn context(rng: &mut Rng) -> Context {
    let targeting_key = match rng.below(4) {
        0 => None,
        1 => Some(s("")),
        _ => Some(format!("user-{}", rng.below(1000))),
    };
    let attributes = (0..rng.below(5)).map(|_| (s(rng.pick(NAMES)), attr(rng))).collect();
    Context { targeting_key, attributes }
}

fn cond(rng: &mut Rng, depth: u32) -> Cond {
    let name = s(rng.pick(NAMES));
    let pick = if depth == 0 { rng.below(6) } else { rng.below(9) };
    match pick {
        0 => {
            let op = rng.pick(&[CmpOp::Eq, CmpOp::Ne, CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge]);
            Cond::Compare { attr: name, op, value: attr(rng) }
        }
        1 => Cond::In { attr: name, list: (0..rng.below(4)).map(|_| text(rng)).collect() },
        2 => Cond::Contains { attr: name, needle: s(rng.pick(&["@faas.com", "", "a", "日"])) },
        3 => Cond::StartsWith { attr: name, prefix: s(rng.pick(&["rachel", "", "1.", " "])) },
        4 => Cond::EndsWith { attr: name, suffix: s(rng.pick(&[".com", "", "rc.1", " "])) },
        5 => {
            use SemVerOp::*;
            let op = rng.pick(&[Eq, Ne, Lt, Le, Gt, Ge, Caret, Tilde]);
            Cond::SemVer { attr: name, op, version: s(rng.pick(VERSIONS)) }
        }
        6 => Cond::And((0..rng.below(3)).map(|_| cond(rng, depth - 1)).collect()),
        7 => Cond::Or((0..rng.below(3)).map(|_| cond(rng, depth - 1)).collect()),
        _ => Cond::Not(Box::new(cond(rng, depth - 1))),
    }
}

const VARIANT_NAMES: &[&str] = &["on", "off", "a", "b", "purple"];

fn bucket_by(rng: &mut Rng) -> BucketBy {
    match rng.below(4) {
        0 => BucketBy::Default,
        1 => BucketBy::Literal(text(rng)),
        2 => BucketBy::Attr(s(rng.pick(NAMES))),
        _ => BucketBy::Cat(
            (0..rng.below(4))
                .map(|_| if rng.below(2) == 0 { Part::Literal(text(rng)) } else { Part::Attr(s(rng.pick(NAMES))) })
                .collect(),
        ),
    }
}

fn split_weights(rng: &mut Rng) -> Vec<(String, i64)> {
    (0..rng.below(5))
        .map(|_| {
            let w = match rng.below(8) {
                0 => -rng.pick(&[1, 7, i64::MAX]),
                1 => 0,
                2 => rng.pick(&[i64::from(i32::MAX), i64::from(i32::MAX) + 1, 1 << 30]),
                _ => rng.below(100) as i64,
            };
            (s(rng.pick(VARIANT_NAMES)), w)
        })
        .collect()
}

fn valid_split(rng: &mut Rng) -> (BucketBy, Vec<(String, i64)>) {
    loop {
        let (by, ws) = (bucket_by(rng), split_weights(rng));
        if Split::new(by.clone(), ws.clone()).is_ok() {
            return (by, ws);
        }
    }
}

fn rule_desc(rng: &mut Rng, depth: u32) -> R {
    match if depth == 0 { rng.below(3) } else { rng.below(6) } {
        0 => R::Variant(s(rng.pick(VARIANT_NAMES))),
        1 => R::Null,
        2 => {
            let (by, ws) = valid_split(rng);
            R::Fractional(by, ws)
        }
        _ => R::If(cond(rng, 2), Box::new(rule_desc(rng, depth - 1)), Box::new(rule_desc(rng, depth - 1))),
    }
}

fn flag_value(rng: &mut Rng, kind: u64) -> FlagValue {
    match kind {
        0 => FlagValue::Bool(rng.below(2) == 0),
        1 => FlagValue::String(text(rng)),
        2 => FlagValue::Integer(rng.pick(&[0, -1, 7, i64::MAX, i64::MIN, 1 << 53, (1 << 53) + 1])),
        _ => FlagValue::Float(rng.pick(&[0.0, -0.0, 0.5, 1e300, f64::NAN, -2.75])),
    }
}

fn flag_desc(rng: &mut Rng, key: &str) -> F {
    let kind = rng.below(4);
    let mixed = rng.below(10) == 0;
    let mut variants = vec![];
    for n in ["on", "off", "a", "b"] {
        if rng.below(4) != 0 {
            let k = if mixed { rng.below(4) } else { kind };
            variants.push((s(n), flag_value(rng, k)));
        }
    }
    F {
        key: s(key),
        state: if rng.below(8) == 0 { State::Disabled } else { State::Enabled },
        variants,
        default_variant: s(rng.pick(&["on", "off", "a", "b", "missing"])),
        targeting: if rng.below(5) == 0 { None } else { Some(rule_desc(rng, 3)) },
    }
}

/// A set of valid flags under the keys `f0`..`f3`.
fn flag_descs(rng: &mut Rng) -> Vec<F> {
    let mut out = vec![];
    for k in 0..4 {
        loop {
            let f = flag_desc(rng, &format!("f{k}"));
            if flag(&f).is_ok() {
                out.push(f);
                break;
            }
        }
    }
    out
}

fn fallback(rng: &mut Rng) -> FlagValue {
    let kind = rng.below(4);
    flag_value(rng, kind)
}

fn bytes(rng: &mut Rng, len: usize) -> Vec<u8> {
    (0..len).map(|_| rng.below(256) as u8).collect()
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let mut rng = Rng::new(0xf1a9);
    for len in 0..=40 {
        for seed in [0, 1, 1234, u32::MAX] {
            let data = bytes(&mut rng, len);
            assert_eq!(flags::murmur3_32(&data, seed), idiomatic::murmur3_32(&data, seed), "{data:?} {seed}");
        }
    }
    for _ in 0..2000 {
        let len = rng.below(64) as usize;
        let data = bytes(&mut rng, len);
        let seed = rng.next() as u32;
        assert_eq!(flags::murmur3_32(&data, seed), idiomatic::murmur3_32(&data, seed));
    }
    use SemVerOp::*;
    for a in VERSIONS {
        for b in VERSIONS {
            for op in [Eq, Ne, Lt, Le, Gt, Ge, Caret, Tilde] {
                assert_eq!(flags::sem_ver_matches(a, op, b), idiomatic::sem_ver_matches(a, op, b), "{a} {op:?} {b}");
            }
        }
    }
    for _ in 0..3000 {
        let (by, ws) = (bucket_by(&mut rng), split_weights(&mut rng));
        let model = Split::new(by.clone(), ws.clone());
        let reference = idiomatic::Split::new(i_bucket(&by), ws.clone());
        assert_eq!(model.as_ref().err(), reference.as_ref().err(), "{ws:?}");
        if let (Ok(m), Ok(r)) = (model, reference) {
            for _ in 0..4 {
                let c = context(&mut rng);
                let key = rng.pick(&["headerColor", "", "f0"]);
                assert_eq!(flags::split_variant(&m, &c, key), r.variant(&i_ctx(&c), key), "{by:?} {ws:?} {c:?}");
            }
        }
    }
    for _ in 0..3000 {
        let f = flag_desc(&mut rng, "k");
        assert_eq!(flag(&f).err(), i_flag(&f).err(), "{f:?}");
    }
    let mut reasons = BTreeMap::new();
    for _ in 0..400 {
        let fs = flag_descs(&mut rng);
        let (model, reference) = (flag_set(&fs), i_flag_set(&fs));
        for _ in 0..25 {
            let c = context(&mut rng);
            let key = rng.pick(&["f0", "f1", "f2", "f3", "nope"]);
            let fb = fallback(&mut rng);
            let got = flags::resolve(&model, key, &c, fb.clone());
            let want = idiomatic::resolve(&reference, key, &i_ctx(&c), i_value(&fb));
            assert_eq!(format!("{got:?}"), format!("{want:?}"), "{key} {c:?} {fs:?}");
            *reasons.entry(format!("{:?}/{:?}", got.reason, got.error_code)).or_insert(0) += 1;
        }
    }
    // Every reason and error code the rules can give comes up.
    for r in [
        "Static/None",
        "Default/None",
        "TargetingMatch/None",
        "Split/None",
        "Disabled/None",
        "Error/Some(FlagNotFound)",
        "Error/Some(TypeMismatch)",
        "Error/Some(General)",
    ] {
        assert!(reasons.get(r).is_some_and(|n| *n > 20), "{r}: {reasons:?}");
    }
}

// ---------------------------------------------------------------------------
// Rust against the generated package
// ---------------------------------------------------------------------------

fn flag_case(f: &F) -> support::Case {
    let variants: BTreeMap<String, FlagValue> = f.variants.iter().cloned().collect();
    let targeting = f.targeting.as_ref().map(rule);
    let call = format!(
        "pkg.Flag.new({}, {}, {}, {}, {})",
        f.key.js(),
        f.state.js(),
        variants.js(),
        f.default_variant.js(),
        targeting.js()
    );
    let f = f.clone();
    support::run("show_flag", call, move || flag(&f))
}

fn split_case(by: BucketBy, ws: Vec<(String, i64)>) -> support::Case {
    let call = format!("pkg.Split.new({}, {})", by.js(), ws.js());
    support::run("show_split", call, move || Split::new(by, ws))
}

#[test]
fn flags_match_rust() {
    let mut rng = Rng::new(0x5eed);
    let cases = support::cases(|cases| {
        for text in ["", "a", "ab", "abc", "abcd", "abcde", "hello", "Hello, world!", "日本語 \u{1f600}"] {
            let data = text.as_bytes().to_vec();
            for seed in [0u32, 1, 1234, u32::MAX] {
                cases.push(case!(flags::murmur3_32(data.as_slice(), seed)));
            }
        }
        for _ in 0..60 {
            let len = rng.below(40) as usize;
            let data = bytes(&mut rng, len);
            let seed = rng.next() as u32;
            cases.push(case!(flags::murmur3_32(data.as_slice(), seed)));
        }
        use SemVerOp::*;
        for (i, a) in VERSIONS.iter().enumerate() {
            for (j, b) in VERSIONS.iter().enumerate() {
                if (i * 7 + j) % 5 == 0 {
                    let op = [Eq, Ne, Lt, Le, Gt, Ge, Caret, Tilde][(i + j) % 8];
                    cases.push(case!(flags::sem_ver_matches(*a, op, *b)));
                }
            }
        }
        for (fs, key, context, _, _) in published() {
            let set = flag_set(&fs);
            cases.push(case!(flags::resolve(&set, key, &context, FlagValue::String(s("fallback")))));
        }
        for _ in 0..80 {
            let (by, ws) = (bucket_by(&mut rng), split_weights(&mut rng));
            cases.push(split_case(by.clone(), ws.clone()));
            if let Ok(split) = Split::new(by, ws) {
                for _ in 0..3 {
                    let c = context(&mut rng);
                    cases.push(case!(flags::split_variant(&split, &c, "headerColor")));
                }
            }
        }
        // Float equality as Rust's `==`: -0 equals 0, NaN equals nothing.
        for (target, op) in [(0.0, CmpOp::Eq), (f64::NAN, CmpOp::Eq), (f64::NAN, CmpOp::Ne), (0.0, CmpOp::Le)] {
            let cond = Cond::Compare { attr: s("age"), op, value: Attr::Number(target) };
            let set = flag_set(&[color_flag(
                "headerColor",
                R::If(cond, Box::new(R::Variant(s("blue"))), Box::new(R::Variant(s("green")))),
            )]);
            for age in [0.0, -0.0, f64::NAN] {
                let c = ctx(None, &[("age", Attr::Number(age))]);
                cases.push(case!(flags::resolve(&set, "headerColor", &c, FlagValue::String(s("fb")))));
            }
        }
        for _ in 0..80 {
            let f = flag_desc(&mut rng, "k");
            cases.push(flag_case(&f));
        }
        for _ in 0..30 {
            let fs = flag_descs(&mut rng);
            let set = flag_set(&fs);
            for _ in 0..12 {
                let c = context(&mut rng);
                let key = rng.pick(&["f0", "f1", "f2", "f3", "nope"]);
                let fb = fallback(&mut rng);
                cases.push(case!(flags::resolve(&set, key, &c, fb)));
            }
        }
    });
    assert!((800..3000).contains(&cases.len()), "{} cases", cases.len());
    support::assert_equivalent("flags", flags::SOURCE, &cases);
}
