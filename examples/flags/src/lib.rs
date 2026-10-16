//! Feature-flag evaluation as flagd defines it, so a Rust server and a
//! TypeScript browser pick the same variant for the same user.
//!
//! Semantics are pinned to open-feature/flagd at commit 670c91cdca
//! (core/pkg/evaluator: fractional.go, semver.go, string_comparison.go,
//! json.go) and https://flagd.dev/reference/flag-definitions/. Targeting is a
//! closed rule tree rather than a JSON-logic interpreter. Results carry the
//! OpenFeature reasons and error codes.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------
// MurmurHash3 x86_32
// ---------------------------------------------------------------------------

/// flagd's cap on one weight and on their sum (`math.MaxInt32`).
const MAX_WEIGHT: i64 = 2_147_483_647;

const C1: u32 = 0xcc9e_2d51;
const C2: u32 = 0x1b87_3593;

fn rotl(x: u32, r: u32) -> u32 {
    (x << r) | (x >> (32 - r))
}

fn mix_k(k: u32) -> u32 {
    rotl(k.wrapping_mul(C1), 15).wrapping_mul(C2)
}

fn fmix32(h: u32) -> u32 {
    let mut h = h;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// MurmurHash3, x86 32-bit variant (Austin Appleby's reference).
pub fn murmur3_32(data: &[u8], seed: u32) -> u32 {
    let mut h = seed;
    for block in data.chunks(4) {
        // Little-endian; a short tail block is mixed but not rotated in.
        let mut k: u32 = 0;
        for (i, b) in block.iter().enumerate() {
            k |= u32::from(*b) << (8 * i as u32);
        }
        h ^= mix_k(k);
        if block.len() == 4 {
            h = rotl(h, 13).wrapping_mul(5).wrapping_add(0xe654_6b64);
        }
    }
    h ^= data.len() as u32;
    fmix32(h)
}

// ---------------------------------------------------------------------------
// Values, context
// ---------------------------------------------------------------------------

/// A variant's value. flagd requires every variant of a flag to share a type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FlagValue {
    Bool(bool),
    String(String),
    Integer(i64),
    Float(f64),
}

/// A context attribute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Attr {
    String(String),
    Number(f64),
    Bool(bool),
}

/// The evaluation context: the targeting key plus named attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub targeting_key: Option<String>,
    pub attributes: BTreeMap<String, Attr>,
}

/// An attribute as targeting reads it. `targetingKey` and `$flagd.flagKey`
/// are the names flagd gives the targeting key and the flag key.
fn lookup(ctx: &Context, flag_key: &str, name: &str) -> Option<Attr> {
    if name == "targetingKey" {
        return ctx.targeting_key.clone().map(Attr::String);
    }
    if name == "$flagd.flagKey" {
        return Some(Attr::String(String::from(flag_key)));
    }
    ctx.attributes.get(name).cloned()
}

fn lookup_str(ctx: &Context, flag_key: &str, name: &str) -> Option<String> {
    match lookup(ctx, flag_key, name) {
        Some(Attr::String(s)) => Some(s),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Semantic versions (golang.org/x/mod/semver, as flagd's sem_ver uses it)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemVerOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    /// `^`: same major.
    Caret,
    /// `~`: same major and minor.
    Tilde,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Version {
    major: String,
    minor: String,
    patch: String,
    pre: Vec<String>,
}

fn is_ident_char(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-')
}

fn is_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| matches!(b, b'0'..=b'9'))
}

/// A numeric field: digits without a leading zero (`0` itself is fine).
fn valid_number(s: &str) -> bool {
    is_number(s) && (s == "0" || !s.starts_with("0"))
}

fn valid_pre_ident(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(is_ident_char) && (!is_number(s) || valid_number(s))
}

fn valid_build_ident(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(is_ident_char)
}

/// flagd prepends `v` (or turns a leading `V` into one); Go's semver then
/// takes `vMAJOR[.MINOR[.PATCH[-PRE][+BUILD]]]`.
fn parse_version(text: &str) -> Option<Version> {
    let s = text.strip_prefix("v").or_else(|| text.strip_prefix("V")).unwrap_or(text);
    let (s, build) = match s.split_once('+') {
        Some((a, b)) => (a, Some(b)),
        None => (s, None),
    };
    if let Some(b) = build {
        if !b.split('.').all(valid_build_ident) {
            return None;
        }
    }
    let (core, pre) = match s.split_once('-') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() > 3 || !parts.iter().all(|p| valid_number(p)) {
        return None;
    }
    if parts.len() < 3 && (pre.is_some() || build.is_some()) {
        return None;
    }
    let pre: Vec<String> = match pre {
        Some(p) => {
            if !p.split('.').all(valid_pre_ident) {
                return None;
            }
            p.split('.').map(|x| x.to_string()).collect()
        }
        None => Vec::new(),
    };
    Some(Version {
        major: parts[0].to_string(),
        minor: parts.get(1).unwrap_or(&"0").to_string(),
        patch: parts.get(2).unwrap_or(&"0").to_string(),
        pre,
    })
}

/// Digit strings without leading zeros: longer is larger.
fn cmp_digits(a: &str, b: &str) -> Ordering {
    a.len().cmp(&b.len()).then(a.cmp(b))
}

fn cmp_pre_ident(a: &str, b: &str) -> Ordering {
    match (is_number(a), is_number(b)) {
        (true, true) => cmp_digits(a, b),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => a.cmp(b),
    }
}

fn cmp_pre(a: &[String], b: &[String]) -> Ordering {
    if a.is_empty() || b.is_empty() {
        // A release is greater than any prerelease of it.
        return b.len().cmp(&a.len());
    }
    for (x, y) in a.iter().zip(b.iter()) {
        let o = cmp_pre_ident(x, y);
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}

fn cmp_version(a: &Version, b: &Version) -> Ordering {
    cmp_digits(&a.major, &b.major)
        .then(cmp_digits(&a.minor, &b.minor))
        .then(cmp_digits(&a.patch, &b.patch))
        .then(cmp_pre(&a.pre, &b.pre))
}

/// `sem_ver`: false when either side is not a version (flagd's null).
pub fn sem_ver_matches(actual: &str, op: SemVerOp, target: &str) -> bool {
    let (Some(a), Some(b)) = (parse_version(actual), parse_version(target)) else {
        return false;
    };
    let o = cmp_version(&a, &b);
    match op {
        SemVerOp::Eq => o == Ordering::Equal,
        SemVerOp::Ne => o != Ordering::Equal,
        SemVerOp::Lt => o == Ordering::Less,
        SemVerOp::Le => o != Ordering::Greater,
        SemVerOp::Gt => o == Ordering::Greater,
        SemVerOp::Ge => o != Ordering::Less,
        SemVerOp::Caret => a.major == b.major,
        SemVerOp::Tilde => a.major == b.major && a.minor == b.minor,
    }
}

// ---------------------------------------------------------------------------
// Targeting rules
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// A condition. A missing attribute, or one of the wrong type, makes every
/// test false except `!=` (JSON logic's null is falsy and unequal).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Cond {
    /// `{"op": [{"var": attr}, value]}`; ordering compares numbers only.
    Compare {
        attr: String,
        op: CmpOp,
        value: Attr,
    },
    /// `{"in": [{"var": attr}, [..]]}`: the attribute is one of the strings.
    In {
        attr: String,
        list: Vec<String>,
    },
    /// `{"in": [needle, {"var": attr}]}`: the attribute contains the text.
    Contains {
        attr: String,
        needle: String,
    },
    StartsWith {
        attr: String,
        prefix: String,
    },
    EndsWith {
        attr: String,
        suffix: String,
    },
    SemVer {
        attr: String,
        op: SemVerOp,
        version: String,
    },
    And(Vec<Cond>),
    Or(Vec<Cond>),
    Not(Box<Cond>),
}

fn compare_attr(found: Option<Attr>, op: CmpOp, value: &Attr) -> bool {
    match op {
        CmpOp::Eq => found.as_ref() == Some(value),
        CmpOp::Ne => found.as_ref() != Some(value),
        _ => {
            let (Some(Attr::Number(a)), Attr::Number(b)) = (found, value) else {
                return false;
            };
            match op {
                CmpOp::Lt => a < *b,
                CmpOp::Le => a <= *b,
                CmpOp::Gt => a > *b,
                _ => a >= *b,
            }
        }
    }
}

fn holds(cond: &Cond, ctx: &Context, flag_key: &str) -> bool {
    match cond {
        Cond::Compare { attr, op, value } => compare_attr(lookup(ctx, flag_key, attr), *op, value),
        Cond::In { attr, list } => match lookup_str(ctx, flag_key, attr) {
            Some(s) => list.contains(&s),
            None => false,
        },
        Cond::Contains { attr, needle } => lookup_str(ctx, flag_key, attr).is_some_and(|s| s.contains(needle.as_str())),
        Cond::StartsWith { attr, prefix } => {
            lookup_str(ctx, flag_key, attr).is_some_and(|s| s.starts_with(prefix.as_str()))
        }
        Cond::EndsWith { attr, suffix } => {
            lookup_str(ctx, flag_key, attr).is_some_and(|s| s.ends_with(suffix.as_str()))
        }
        Cond::SemVer { attr, op, version } => match lookup_str(ctx, flag_key, attr) {
            Some(s) => sem_ver_matches(&s, *op, version),
            None => false,
        },
        Cond::And(cs) => cs.iter().all(|c| holds(c, ctx, flag_key)),
        Cond::Or(cs) => cs.iter().any(|c| holds(c, ctx, flag_key)),
        Cond::Not(c) => !holds(c, ctx, flag_key),
    }
}

/// One piece of a `cat` bucketing expression.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Part {
    Literal(String),
    Attr(String),
}

/// What `fractional` hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BucketBy {
    /// No first argument: flag key followed by the targeting key.
    Default,
    /// A literal string.
    Literal(String),
    /// `{"var": name}`: missing falls back to `Default`.
    Attr(String),
    /// `{"cat": [..]}`: parts joined; JSON logic trims a join of two or more.
    Cat(Vec<Part>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SplitWire")]
pub struct Split {
    bucket_by: BucketBy,
    shares: Vec<(String, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitWire {
    pub bucket_by: BucketBy,
    pub weights: Vec<(String, i64)>,
}

impl Split {
    /// Weights as flagd reads them: negative ones count as 0, none may exceed
    /// `i32::MAX`, nor may their sum.
    pub fn new(bucket_by: BucketBy, weights: Vec<(String, i64)>) -> Result<Split, DefinitionError> {
        if weights.is_empty() {
            return Err(DefinitionError::EmptySplit);
        }
        let mut shares: Vec<(String, u32)> = Vec::new();
        let mut total: i64 = 0;
        for (variant, weight) in &weights {
            if *weight > MAX_WEIGHT {
                return Err(DefinitionError::WeightTooLarge);
            }
            let w = (*weight).max(0);
            total += w;
            shares.push((variant.clone(), w as u32));
        }
        if total > MAX_WEIGHT {
            return Err(DefinitionError::WeightTooLarge);
        }
        Ok(Split { bucket_by, shares })
    }
}

impl TryFrom<SplitWire> for Split {
    type Error = DefinitionError;
    fn try_from(w: SplitWire) -> Result<Split, DefinitionError> {
        Split::new(w.bucket_by, w.weights)
    }
}

/// What the bucketing expression yields.
enum BucketKey {
    Hash(String),
    /// flagd's `fractional` returns null.
    Null,
}

fn key_for(by: &BucketBy, ctx: &Context, flag_key: &str) -> BucketKey {
    match by {
        BucketBy::Literal(s) => BucketKey::Hash(s.clone()),
        BucketBy::Attr(name) => match lookup(ctx, flag_key, name) {
            Some(Attr::String(s)) => BucketKey::Hash(s),
            Some(_) => BucketKey::Null,
            None => key_for(&BucketBy::Default, ctx, flag_key),
        },
        BucketBy::Cat(parts) => {
            let mut joined = String::new();
            for part in parts {
                match part {
                    Part::Literal(s) => joined.push_str(s),
                    Part::Attr(name) => match lookup_str(ctx, flag_key, name) {
                        Some(s) => joined.push_str(&s),
                        None => return BucketKey::Null,
                    },
                }
            }
            if parts.len() >= 2 {
                BucketKey::Hash(joined.trim().to_string())
            } else {
                BucketKey::Hash(joined)
            }
        }
        BucketBy::Default => match &ctx.targeting_key {
            Some(t) if !t.is_empty() => BucketKey::Hash(format!("{flag_key}{t}")),
            _ => BucketKey::Null,
        },
    }
}

/// `fractional`: the hash scaled into `0..total`, then the first share whose
/// running end exceeds it. `None` is flagd's null.
pub fn split_variant(split: &Split, ctx: &Context, flag_key: &str) -> Option<String> {
    let BucketKey::Hash(key) = key_for(&split.bucket_by, ctx, flag_key) else {
        return None;
    };
    let total: u64 = split.shares.iter().map(|(_, w)| u64::from(*w)).sum();
    if total == 0 {
        return None;
    }
    let hash = murmur3_32(key.as_bytes(), 0);
    let bucket = (u64::from(hash) * total) >> 32;
    let mut end: u64 = 0;
    for (variant, weight) in &split.shares {
        end += u64::from(*weight);
        if bucket < end {
            return Some(variant.clone());
        }
    }
    None
}

/// A targeting rule: what JSON logic's `if` chains over flagd's operations
/// return.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Rule {
    Variant(String),
    /// JSON `null`: use the default variant.
    Null,
    If(Box<Cond>, Box<Rule>, Box<Rule>),
    Fractional(Split),
}

enum Outcome {
    Matched(String),
    Split(String),
    Null,
}

fn run_rule(rule: &Rule, ctx: &Context, flag_key: &str) -> Outcome {
    match rule {
        Rule::Variant(v) => Outcome::Matched(v.clone()),
        Rule::Null => Outcome::Null,
        Rule::If(c, then, otherwise) => {
            if holds(c, ctx, flag_key) {
                run_rule(then, ctx, flag_key)
            } else {
                run_rule(otherwise, ctx, flag_key)
            }
        }
        Rule::Fractional(s) => match split_variant(s, ctx, flag_key) {
            Some(v) => Outcome::Split(v),
            None => Outcome::Null,
        },
    }
}

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum State {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefinitionError {
    MixedVariantTypes,
    UnknownDefaultVariant,
    EmptySplit,
    WeightTooLarge,
    DuplicateKey,
}

impl fmt::Display for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DefinitionError::MixedVariantTypes => f.write_str("variants of a flag must share a type"),
            DefinitionError::UnknownDefaultVariant => f.write_str("the default variant is not a variant"),
            DefinitionError::EmptySplit => f.write_str("fractional needs at least one variant"),
            DefinitionError::WeightTooLarge => f.write_str("fractional weights exceed 2147483647"),
            DefinitionError::DuplicateKey => f.write_str("two flags share a key"),
        }
    }
}

fn same_kind(a: &FlagValue, b: &FlagValue) -> bool {
    matches!(
        (a, b),
        (FlagValue::Bool(_), FlagValue::Bool(_))
            | (FlagValue::String(_), FlagValue::String(_))
            | (FlagValue::Integer(_), FlagValue::Integer(_))
            | (FlagValue::Float(_), FlagValue::Float(_))
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FlagWire", rename_all = "camelCase")]
pub struct Flag {
    key: String,
    state: State,
    variants: BTreeMap<String, FlagValue>,
    default_variant: String,
    targeting: Option<Rule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlagWire {
    pub key: String,
    pub state: State,
    pub variants: BTreeMap<String, FlagValue>,
    pub default_variant: String,
    pub targeting: Option<Rule>,
}

impl Flag {
    pub fn new(
        key: String,
        state: State,
        variants: BTreeMap<String, FlagValue>,
        default_variant: String,
        targeting: Option<Rule>,
    ) -> Result<Flag, DefinitionError> {
        let Some(chosen) = variants.get(&default_variant) else {
            return Err(DefinitionError::UnknownDefaultVariant);
        };
        if !variants.values().all(|v| same_kind(v, chosen)) {
            return Err(DefinitionError::MixedVariantTypes);
        }
        Ok(Flag { key, state, variants, default_variant, targeting })
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl TryFrom<FlagWire> for Flag {
    type Error = DefinitionError;
    fn try_from(w: FlagWire) -> Result<Flag, DefinitionError> {
        Flag::new(w.key, w.state, w.variants, w.default_variant, w.targeting)
    }
}

/// Flags by key.
#[derive(Debug, Clone, PartialEq)]
pub struct FlagSet {
    flags: BTreeMap<String, Flag>,
}

impl FlagSet {
    pub fn new(flags: Vec<Flag>) -> Result<FlagSet, DefinitionError> {
        let mut map: BTreeMap<String, Flag> = BTreeMap::new();
        for flag in flags {
            if map.contains_key(&flag.key) {
                return Err(DefinitionError::DuplicateKey);
            }
            map.insert(flag.key.clone(), flag);
        }
        Ok(FlagSet { flags: map })
    }
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Reason {
    Static,
    Default,
    TargetingMatch,
    Split,
    Disabled,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    FlagNotFound,
    ParseError,
    TypeMismatch,
    TargetingKeyMissing,
    General,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub value: FlagValue,
    pub variant: Option<String>,
    pub reason: Reason,
    pub error_code: Option<ErrorCode>,
}

fn failed(fallback: FlagValue, code: ErrorCode) -> Resolution {
    Resolution { value: fallback, variant: None, reason: Reason::Error, error_code: Some(code) }
}

/// The variant's value as the caller's type: a float fallback takes an integer
/// variant, nothing else converts.
fn as_requested(value: &FlagValue, fallback: &FlagValue) -> Option<FlagValue> {
    match (value, fallback) {
        (FlagValue::Integer(n), FlagValue::Float(_)) => Some(FlagValue::Float(*n as f64)),
        _ if same_kind(value, fallback) => Some(value.clone()),
        _ => None,
    }
}

/// Resolve `key` for `ctx`; `fallback` is returned on any error and sets the
/// type the caller expects (OpenFeature's typed resolvers).
pub fn resolve(flags: &FlagSet, key: &str, ctx: &Context, fallback: FlagValue) -> Resolution {
    let Some(flag) = flags.flags.get(key) else {
        return failed(fallback, ErrorCode::FlagNotFound);
    };
    if flag.state == State::Disabled {
        return Resolution { value: fallback, variant: None, reason: Reason::Disabled, error_code: None };
    }
    let (variant, reason) = match &flag.targeting {
        None => (flag.default_variant.clone(), Reason::Static),
        Some(rule) => match run_rule(rule, ctx, &flag.key) {
            Outcome::Matched(v) => (v, Reason::TargetingMatch),
            Outcome::Split(v) => (v, Reason::Split),
            Outcome::Null => (flag.default_variant.clone(), Reason::Default),
        },
    };
    let Some(value) = flag.variants.get(&variant) else {
        return failed(fallback, ErrorCode::General);
    };
    match as_requested(value, &fallback) {
        Some(v) => Resolution { value: v, variant: Some(variant), reason, error_code: None },
        None => failed(fallback, ErrorCode::TypeMismatch),
    }
}
