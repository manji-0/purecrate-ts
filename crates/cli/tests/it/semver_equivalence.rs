//! `examples/semver` agrees with the generated package, and with the same
//! rules written in idiomatic Rust (`idiomatic`, the line count design/07
//! §2 compares against), on the spec's examples, one-character edits of
//! them, `u64` edges, every version written back, and every pair's
//! precedence and equality. The spec's own ordered chain (SemVer 2.0.0 §11),
//! parse→format→parse, and the two equalities are asserted on both Rust
//! sides.

use crate::support;

purecrate_canon::fixture!(mod semver = "../../../examples/semver/src/lib.rs", "fixtures/semver_driver.rs");

/// SemVer 2.0.0 as one would write it without the subset's constraints.
/// Not converted; the reference only.
mod idiomatic {
    use std::cmp::Ordering;
    use std::fmt;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CorePart {
        Major,
        Minor,
        Patch,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum SemverError {
        Empty,
        MissingPart(CorePart),
        ExtraCorePart,
        EmptyNumber(CorePart),
        NotANumber(CorePart),
        LeadingZero(CorePart),
        NumberTooLarge(CorePart),
        EmptyPreRelease,
        InvalidPreReleaseChar,
        PreReleaseLeadingZero,
        EmptyBuild,
        InvalidBuildChar,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PreId {
        Numeric(String),
        Alpha(String),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Version {
        pub major: u64,
        pub minor: u64,
        pub patch: u64,
        pub pre: Vec<PreId>,
        pub build: Vec<String>,
    }

    fn is_ident(s: &str) -> bool {
        s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    }

    fn is_numeric(s: &str) -> bool {
        s.bytes().all(|b| b.is_ascii_digit())
    }

    fn has_leading_zero(s: &str) -> bool {
        s.len() > 1 && s.starts_with('0')
    }

    fn core_number(s: &str, part: CorePart) -> Result<u64, SemverError> {
        use SemverError::*;
        match s {
            "" => Err(EmptyNumber(part)),
            _ if !is_numeric(s) => Err(NotANumber(part)),
            _ if has_leading_zero(s) => Err(LeadingZero(part)),
            _ => s.parse().map_err(|_| NumberTooLarge(part)),
        }
    }

    fn pre_id(s: &str) -> Result<PreId, SemverError> {
        use SemverError::*;
        match s {
            "" => Err(EmptyPreRelease),
            _ if !is_ident(s) => Err(InvalidPreReleaseChar),
            _ if !is_numeric(s) => Ok(PreId::Alpha(s.to_owned())),
            _ if has_leading_zero(s) => Err(PreReleaseLeadingZero),
            _ => Ok(PreId::Numeric(s.to_owned())),
        }
    }

    fn build_id(s: &str) -> Result<String, SemverError> {
        match s {
            "" => Err(SemverError::EmptyBuild),
            _ if !is_ident(s) => Err(SemverError::InvalidBuildChar),
            _ => Ok(s.to_owned()),
        }
    }

    impl Version {
        pub fn parse(s: &str) -> Result<Self, SemverError> {
            if s.is_empty() {
                return Err(SemverError::Empty);
            }
            let (rest, build) = s.split_once('+').map_or((s, None), |(r, b)| (r, Some(b)));
            let (core, pre) = rest.split_once('-').map_or((rest, None), |(c, p)| (c, Some(p)));
            let parts = [CorePart::Major, CorePart::Minor, CorePart::Patch];
            let mut nums = [0u64; 3];
            let mut n = 0;
            for piece in core.split('.') {
                let part = *parts.get(n).ok_or(SemverError::ExtraCorePart)?;
                nums[n] = core_number(piece, part)?;
                n += 1;
            }
            if let Some(&part) = parts.get(n) {
                return Err(SemverError::MissingPart(part));
            }
            let pre = pre.map_or(Ok(vec![]), |p| p.split('.').map(pre_id).collect())?;
            let build = build.map_or(Ok(vec![]), |b| b.split('.').map(build_id).collect())?;
            let [major, minor, patch] = nums;
            Ok(Version { major, minor, patch, pre, build })
        }
    }

    impl PartialOrd for PreId {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for PreId {
        fn cmp(&self, other: &Self) -> Ordering {
            match (self, other) {
                (PreId::Numeric(x), PreId::Numeric(y)) => x.len().cmp(&y.len()).then_with(|| x.cmp(y)),
                (PreId::Numeric(_), PreId::Alpha(_)) => Ordering::Less,
                (PreId::Alpha(_), PreId::Numeric(_)) => Ordering::Greater,
                (PreId::Alpha(x), PreId::Alpha(y)) => x.cmp(y),
            }
        }
    }

    /// §2, §9, §10: `MAJOR.MINOR.PATCH[-PRE][+BUILD]`.
    impl fmt::Display for Version {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
            let pre: Vec<&str> = self
                .pre
                .iter()
                .map(|p| match p {
                    PreId::Numeric(s) | PreId::Alpha(s) => s.as_str(),
                })
                .collect();
            if !pre.is_empty() {
                write!(f, "-{}", pre.join("."))?;
            }
            if !self.build.is_empty() {
                write!(f, "+{}", self.build.join("."))?;
            }
            Ok(())
        }
    }

    /// Equal precedence; `==` is equality in every part.
    pub fn same_precedence(a: &Version, b: &Version) -> bool {
        compare(a, b).is_eq()
    }

    /// §11: core numerically, then a pre-release below none, then the
    /// identifiers left to right (`Vec`'s order: a longer run of equal
    /// identifiers is greater). Build metadata is ignored.
    pub fn compare(a: &Version, b: &Version) -> Ordering {
        (a.major, a.minor, a.patch).cmp(&(b.major, b.minor, b.patch)).then_with(|| {
            match (a.pre.is_empty(), b.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => a.pre.cmp(&b.pre),
            }
        })
    }
}

/// §11's chain, strictly increasing.
const CHAIN: [&str; 14] = [
    "1.0.0-9",
    "1.0.0-10",
    "1.0.0-99999999999999999999",
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

/// Valid versions from §9–§10 and around the edges.
const VALID: [&str; 20] = [
    "0.0.0",
    "1.9.0",
    "1.10.0",
    "1.0.0-0.3.7",
    "1.0.0-x.7.z.92",
    "1.0.0-x-y-z.--",
    "1.0.0-alpha+001",
    "1.0.0+20130313144700",
    "1.0.0-beta+exp.sha.5114f85",
    "1.0.0+21AF26D3----117B344092BD",
    "1.0.0-0A",
    "1.0.0-A.a",
    "1.0.0-a-",
    "1.0.0+0.01",
    "18446744073709551615.0.0",
    "0.18446744073709551615.0",
    "0.0.18446744073709551615",
    "1.0.0-18446744073709551615",
    "1.0.0-18446744073709551616",
    "1.0.0-99999999999999999999999.1",
];

const INVALID: [&str; 21] = [
    "",
    "1",
    "1.2",
    "1.2.3.4",
    "1..3",
    ".1.2",
    "1.2.",
    "01.2.3",
    "1.02.3",
    "1.2.03",
    "v1.2.3",
    "1.2.3-",
    "1.2.3+",
    "1.2.3-a..b",
    "1.2.3+a..b",
    "1.2.3-01",
    "1.2.3-é",
    "1.2.3+é",
    "1.2.3-a+b+c",
    "18446744073709551616.0.0",
    " 1.2.3",
];

fn inputs() -> Vec<String> {
    let seeds: Vec<&str> = CHAIN.iter().chain(&VALID).chain(&INVALID).copied().collect();
    let mut out: Vec<String> = seeds.iter().map(|s| s.to_string()).collect();
    for s in CHAIN.iter().chain(&VALID) {
        // Every one-character replacement from a small alphabet that
        // crosses each class, every deletion, and a character appended.
        for (i, c) in s.char_indices() {
            let rest = &s[i + c.len_utf8()..];
            for r in ['0', '1', 'a', 'Z', '-', '.', '+', 'é'] {
                if r != c {
                    out.push(format!("{}{r}{rest}", &s[..i]));
                }
            }
            out.push(format!("{}{rest}", &s[..i]));
        }
        out.push(format!("{s}.0"));
        out.push(format!("{s}-"));
    }
    out.sort();
    out.dedup();
    out
}

type Verdict = Result<(), String>;

fn verdict_constrained(s: &str) -> Verdict {
    semver::Version::parse(s).map(|_| ()).map_err(|e| format!("{e:?}"))
}

fn verdict_idiomatic(s: &str) -> Verdict {
    idiomatic::Version::parse(s).map(|_| ()).map_err(|e| format!("{e:?}"))
}

/// Every accepted input, so `compare` sees pre-release lists of each shape.
fn accepted() -> Vec<String> {
    inputs().into_iter().filter(|s| idiomatic::Version::parse(s).is_ok()).collect()
}

#[test]
fn the_spec_chain_is_increasing() {
    for w in CHAIN.windows(2) {
        let (a, b) = (semver::Version::parse(w[0]).unwrap(), semver::Version::parse(w[1]).unwrap());
        assert!(semver::compare(&a, &b).is_lt(), "{} < {}", w[0], w[1]);
        let (a, b) = (idiomatic::Version::parse(w[0]).unwrap(), idiomatic::Version::parse(w[1]).unwrap());
        assert!(idiomatic::compare(&a, &b).is_lt(), "{} < {}", w[0], w[1]);
    }
    for s in VALID {
        assert!(idiomatic::Version::parse(s).is_ok(), "{s} is valid");
    }
    for s in INVALID {
        assert!(idiomatic::Version::parse(s).is_err(), "{s:?} is invalid");
    }
    let plain = semver::Version::parse("1.0.0-alpha").unwrap();
    let built = semver::Version::parse("1.0.0-alpha+001").unwrap();
    assert!(semver::compare(&plain, &built).is_eq(), "build metadata is ignored");
}

#[test]
fn format_writes_back_what_parse_read() {
    let v = semver::Version::parse("1.2.3-alpha.1+build.5").unwrap();
    assert_eq!(v.format(), "1.2.3-alpha.1+build.5");
    assert_eq!(v.to_string(), "1.2.3-alpha.1+build.5", "Display is format");
    let max = semver::Version::parse("18446744073709551615.0.10").unwrap();
    assert_eq!(max.format(), "18446744073709551615.0.10");
    // Every valid string is canonical (no leading zero is accepted), so
    // format is parse's inverse on all of them.
    for s in accepted() {
        let v = semver::Version::parse(&s).unwrap();
        assert_eq!(v.format(), s, "parse then format");
        assert_eq!(semver::Version::parse(&v.format()), Ok(v), "parse, format, parse: {s}");
        assert_eq!(idiomatic::Version::parse(&s).unwrap().to_string(), s, "idiomatic {s}");
    }
}

#[test]
fn equal_and_same_precedence_are_the_two_equalities() {
    let p = |s: &str| semver::Version::parse(s).unwrap();
    assert!(semver::equal(&p("1.0.0-alpha+001"), &p("1.0.0-alpha+001")));
    assert!(!semver::equal(&p("1.0.0-alpha"), &p("1.0.0-alpha+001")), "build metadata counts");
    assert!(!semver::equal(&p("1.0.0+a.b"), &p("1.0.0+a")));
    assert!(semver::same_precedence(&p("1.0.0-alpha"), &p("1.0.0-alpha+001")), "build metadata is ignored");
    assert!(semver::same_precedence(&p("1.0.0+x"), &p("1.0.0+y")));
    assert!(!semver::same_precedence(&p("1.0.0-alpha"), &p("1.0.0")));
    assert!(!semver::same_precedence(&p("1.0.0-1"), &p("1.0.0-01a")));
    // `equal` is Rust's `==` on `Version`; `same_precedence` is `compare`
    // returning `Equal`, on both Rust sides.
    let versions = accepted();
    for a in &versions {
        for b in &versions {
            let (x, y) = (p(a), p(b));
            let (i, j) = (idiomatic::Version::parse(a).unwrap(), idiomatic::Version::parse(b).unwrap());
            assert_eq!(semver::equal(&x, &y), x == y, "{a} == {b}");
            assert_eq!(semver::equal(&x, &y), i == j, "{a} == {b}, idiomatic");
            assert_eq!(semver::same_precedence(&x, &y), semver::compare(&x, &y).is_eq(), "{a} ~ {b}");
            assert_eq!(semver::same_precedence(&x, &y), idiomatic::same_precedence(&i, &j), "{a} ~ {b}, idiomatic");
        }
    }
}

#[test]
fn constrained_rust_is_the_idiomatic_rules() {
    let differ: Vec<String> = inputs()
        .iter()
        .filter(|s| verdict_constrained(s) != verdict_idiomatic(s))
        .map(|s| format!("{s:?}: constrained {:?}, idiomatic {:?}", verdict_constrained(s), verdict_idiomatic(s)))
        .collect();
    assert!(differ.is_empty(), "parsing differs:\n{}", differ.join("\n"));

    let versions = accepted();
    let mut differ = Vec::new();
    for a in &versions {
        for b in &versions {
            let c = semver::compare(&semver::Version::parse(a).unwrap(), &semver::Version::parse(b).unwrap());
            let i = idiomatic::compare(&idiomatic::Version::parse(a).unwrap(), &idiomatic::Version::parse(b).unwrap());
            if c != i {
                differ.push(format!("{a} vs {b}: constrained {c:?}, idiomatic {i:?}"));
            }
        }
    }
    assert!(differ.is_empty(), "precedence differs:\n{}", differ.join("\n"));
}

#[test]
fn semver_matches_rust() {
    let inputs = inputs();
    let versions: Vec<&str> = CHAIN.iter().chain(&VALID).copied().collect();
    let cases = support::quietly(|| {
        let mut cases: Vec<_> = inputs.iter().map(|s| case!(semver::parse_version(s.clone()))).collect();
        cases.extend(inputs.iter().map(|s| case!(semver::format_version(s.clone()))));
        for a in &versions {
            for b in &versions {
                cases.push(case!(semver::compare_versions(a.to_string(), b.to_string())));
                cases.push(case!(semver::equal_versions(a.to_string(), b.to_string())));
            }
        }
        cases
    });
    support::assert_equivalent("semver", semver::SOURCE, &cases);
}
