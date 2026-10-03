// Semantic Versioning 2.0.0 (https://semver.org/spec/v2.0.0.html):
// parsing `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]` and precedence (§11).
//
// The grammar puts no bound on a number. A numeric pre-release identifier
// is kept as its digits, so any length is accepted and compared exactly;
// the three core numbers are `u64`, as the `semver` crate keeps them, and a
// larger one is refused (`NumberTooLarge`). `==` on `Version` compares the
// build metadata too; precedence, which ignores it, is `compare`.

use std::cmp::Ordering;

/// Which of the three core numbers a diagnostic refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorePart {
    Major,
    Minor,
    Patch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemverError {
    /// The input is the empty string.
    Empty,
    /// The core has fewer than three dot-separated numbers.
    MissingPart(CorePart),
    /// The core has more than three dot-separated numbers.
    ExtraCorePart,
    /// A core number is empty (`1..2`).
    EmptyNumber(CorePart),
    /// A core number contains a non-digit.
    NotANumber(CorePart),
    /// A core number has a leading zero (`01`).
    LeadingZero(CorePart),
    /// A core number does not fit in u64.
    NumberTooLarge(CorePart),
    /// `-` is followed by nothing, or a pre-release identifier is empty (`1.0.0-a..b`).
    EmptyPreRelease,
    /// A pre-release identifier contains a character outside `[0-9A-Za-z-]`.
    InvalidPreReleaseChar,
    /// A numeric pre-release identifier has a leading zero (`1.0.0-01`).
    PreReleaseLeadingZero,
    /// `+` is followed by nothing, or a build identifier is empty.
    EmptyBuild,
    /// A build identifier contains a character outside `[0-9A-Za-z-]`.
    InvalidBuildChar,
}

/// One pre-release identifier. A numeric one is its digits, without a
/// leading zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreId {
    Numeric(String),
    Alpha(String),
}

/// A valid semantic version. Only [`Version::parse`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<PreId>,
    build: Vec<String>,
}

impl Version {
    pub fn parse(s: &str) -> Result<Version, SemverError> {
        if s.is_empty() {
            return Err(SemverError::Empty);
        }
        let (rest, build) = match s.split_once('+') {
            Some((x, y)) => (x, Some(y)),
            None => (s, None),
        };
        let (core, pre) = match rest.split_once('-') {
            Some((x, y)) => (x, Some(y)),
            None => (rest, None),
        };
        let parts = core.split('.').collect::<Vec<&str>>();
        let major = parse_core_number(parts[0], CorePart::Major)?;
        if parts.len() < 2 {
            return Err(SemverError::MissingPart(CorePart::Minor));
        }
        let minor = parse_core_number(parts[1], CorePart::Minor)?;
        if parts.len() < 3 {
            return Err(SemverError::MissingPart(CorePart::Patch));
        }
        let patch = parse_core_number(parts[2], CorePart::Patch)?;
        if parts.len() > 3 {
            return Err(SemverError::ExtraCorePart);
        }
        let pre = match pre {
            Some(p) => p
                .split('.')
                .map(parse_pre_id)
                .collect::<Result<Vec<PreId>, SemverError>>()?,
            None => vec![],
        };
        let build = match build {
            Some(b) => b
                .split('.')
                .map(parse_build_id)
                .collect::<Result<Vec<String>, SemverError>>()?,
            None => vec![],
        };
        Ok(Version {
            major,
            minor,
            patch,
            pre,
            build,
        })
    }

    pub fn major(&self) -> u64 {
        self.major
    }

    pub fn minor(&self) -> u64 {
        self.minor
    }

    pub fn patch(&self) -> u64 {
        self.patch
    }

    pub fn pre_release(&self) -> &Vec<PreId> {
        &self.pre
    }

    pub fn build_metadata(&self) -> &Vec<String> {
        &self.build
    }

    pub fn is_pre_release(&self) -> bool {
        !self.pre.is_empty()
    }
}

fn is_ident_char(c: u8) -> bool {
    matches!(c, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-')
}

fn all_digits(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b'0'..=b'9'))
}

fn has_leading_zero(s: &str) -> bool {
    s.len() > 1 && s.starts_with("0")
}

fn parse_core_number(s: &str, part: CorePart) -> Result<u64, SemverError> {
    match s {
        "" => Err(SemverError::EmptyNumber(part)),
        _ if !all_digits(s) => Err(SemverError::NotANumber(part)),
        _ if has_leading_zero(s) => Err(SemverError::LeadingZero(part)),
        _ => s.parse::<u64>().map_err(|_| SemverError::NumberTooLarge(part)),
    }
}

fn parse_pre_id(s: &str) -> Result<PreId, SemverError> {
    match s {
        "" => Err(SemverError::EmptyPreRelease),
        _ if !s.bytes().all(is_ident_char) => Err(SemverError::InvalidPreReleaseChar),
        _ if !all_digits(s) => Ok(PreId::Alpha(String::from(s))),
        _ if has_leading_zero(s) => Err(SemverError::PreReleaseLeadingZero),
        _ => Ok(PreId::Numeric(String::from(s))),
    }
}

fn parse_build_id(s: &str) -> Result<String, SemverError> {
    match s {
        "" => Err(SemverError::EmptyBuild),
        _ if !s.bytes().all(is_ident_char) => Err(SemverError::InvalidBuildChar),
        _ => Ok(String::from(s)),
    }
}

fn compare_pre_id(a: &PreId, b: &PreId) -> Ordering {
    match (a, b) {
        // Without leading zeros, the longer number is the larger.
        (PreId::Numeric(x), PreId::Numeric(y)) => x.len().cmp(&y.len()).then(x.cmp(y)),
        (PreId::Numeric(_), PreId::Alpha(_)) => Ordering::Less,
        (PreId::Alpha(_), PreId::Numeric(_)) => Ordering::Greater,
        (PreId::Alpha(x), PreId::Alpha(y)) => x.cmp(y),
    }
}

/// Identifiers left to right; a longer list with an equal prefix is greater.
fn compare_pre_ids(a: &Vec<PreId>, b: &Vec<PreId>) -> Ordering {
    for i in 0..a.len().min(b.len()) {
        let o = compare_pre_id(&a[i], &b[i]);
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}

/// Precedence per SemVer 2.0.0 §11. Build metadata is ignored.
pub fn compare(a: &Version, b: &Version) -> Ordering {
    a.major
        .cmp(&b.major)
        .then(a.minor.cmp(&b.minor))
        .then(a.patch.cmp(&b.patch))
        .then_with(|| match (a.pre.is_empty(), b.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            // A version without pre-release has higher precedence.
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => compare_pre_ids(&a.pre, &b.pre),
        })
}
