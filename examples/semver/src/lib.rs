// Semantic Versioning 2.0.0 (https://semver.org/spec/v2.0.0.html):
// parsing `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]` and precedence (§11).

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
    /// A numeric pre-release identifier does not fit in u64.
    PreReleaseTooLarge,
    /// `+` is followed by nothing, or a build identifier is empty.
    EmptyBuild,
    /// A build identifier contains a character outside `[0-9A-Za-z-]`.
    InvalidBuildChar,
}

/// One pre-release identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreId {
    Numeric(u64),
    Alpha(String),
}

/// Dot-separated pre-release identifiers, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreIds {
    Nil,
    Cons(PreId, Box<PreIds>),
}

/// Dot-separated build identifiers, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildIds {
    Nil,
    Cons(String, Box<BuildIds>),
}

/// A valid semantic version. Only [`Version::parse`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: PreIds,
    build: BuildIds,
}

impl Version {
    pub fn parse(s: &str) -> Result<Version, SemverError> {
        if s.is_empty() {
            return Err(SemverError::Empty);
        }
        let plus = s.bytes().position(|b| b == b'+');
        let rest = match plus {
            Some(i) => &s[..i],
            None => s,
        };
        let dash = rest.bytes().position(|b| b == b'-');
        let core = match dash {
            Some(i) => &rest[..i],
            None => rest,
        };
        let mut major: Option<u64> = None;
        let mut minor: Option<u64> = None;
        let mut patch: Option<u64> = None;
        let mut n: u32 = 0;
        for piece in core.split('.') {
            match n {
                0 => major = Some(parse_core_number(piece, CorePart::Major)?),
                1 => minor = Some(parse_core_number(piece, CorePart::Minor)?),
                2 => patch = Some(parse_core_number(piece, CorePart::Patch)?),
                _ => return Err(SemverError::ExtraCorePart),
            }
            n += 1;
        }
        let major = major.ok_or(SemverError::MissingPart(CorePart::Major))?;
        let minor = minor.ok_or(SemverError::MissingPart(CorePart::Minor))?;
        let patch = patch.ok_or(SemverError::MissingPart(CorePart::Patch))?;
        let pre = match dash {
            Some(i) => parse_pre_ids(&rest[i + 1..])?,
            None => PreIds::Nil,
        };
        let build = match plus {
            Some(i) => parse_build_ids(&s[i + 1..])?,
            None => BuildIds::Nil,
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

    pub fn pre_release(&self) -> &PreIds {
        &self.pre
    }

    pub fn build_metadata(&self) -> &BuildIds {
        &self.build
    }

    pub fn is_pre_release(&self) -> bool {
        !matches!(self.pre, PreIds::Nil)
    }
}

fn is_ident_char(c: u8) -> bool {
    matches!(c, b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-')
}

fn all_digits(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b'0'..=b'9'))
}

/// Digits only, already checked non-empty. `None` on overflow.
fn digits_to_u64(s: &str) -> Option<u64> {
    let mut acc: u64 = 0;
    for b in s.bytes() {
        acc = acc.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    Some(acc)
}

fn parse_core_number(s: &str, part: CorePart) -> Result<u64, SemverError> {
    if s.is_empty() {
        return Err(SemverError::EmptyNumber(part));
    }
    if !all_digits(s) {
        return Err(SemverError::NotANumber(part));
    }
    if s.len() > 1 && s.starts_with("0") {
        return Err(SemverError::LeadingZero(part));
    }
    digits_to_u64(s).ok_or(SemverError::NumberTooLarge(part))
}

fn parse_pre_id(s: &str) -> Result<PreId, SemverError> {
    if s.is_empty() {
        return Err(SemverError::EmptyPreRelease);
    }
    if !s.bytes().all(is_ident_char) {
        return Err(SemverError::InvalidPreReleaseChar);
    }
    if all_digits(s) {
        if s.len() > 1 && s.starts_with("0") {
            return Err(SemverError::PreReleaseLeadingZero);
        }
        return match digits_to_u64(s) {
            Some(n) => Ok(PreId::Numeric(n)),
            None => Err(SemverError::PreReleaseTooLarge),
        };
    }
    Ok(PreId::Alpha(String::from(s)))
}

fn parse_pre_ids(s: &str) -> Result<PreIds, SemverError> {
    match s.bytes().position(|b| b == b'.') {
        Some(i) => {
            let head = parse_pre_id(&s[..i])?;
            let tail = parse_pre_ids(&s[i + 1..])?;
            Ok(PreIds::Cons(head, Box::new(tail)))
        }
        None => Ok(PreIds::Cons(parse_pre_id(s)?, Box::new(PreIds::Nil))),
    }
}

fn parse_build_id(s: &str) -> Result<String, SemverError> {
    if s.is_empty() {
        return Err(SemverError::EmptyBuild);
    }
    if !s.bytes().all(is_ident_char) {
        return Err(SemverError::InvalidBuildChar);
    }
    Ok(String::from(s))
}

fn parse_build_ids(s: &str) -> Result<BuildIds, SemverError> {
    match s.bytes().position(|b| b == b'.') {
        Some(i) => {
            let head = parse_build_id(&s[..i])?;
            let tail = parse_build_ids(&s[i + 1..])?;
            Ok(BuildIds::Cons(head, Box::new(tail)))
        }
        None => Ok(BuildIds::Cons(parse_build_id(s)?, Box::new(BuildIds::Nil))),
    }
}

/// The result of comparing two versions by precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precedence {
    Less,
    Equal,
    Greater,
}

fn compare_u64(a: u64, b: u64) -> Precedence {
    if a < b {
        Precedence::Less
    } else if a > b {
        Precedence::Greater
    } else {
        Precedence::Equal
    }
}

/// `first`, or `next` when `first` is `Equal` (both already computed).
fn then(first: Precedence, next: Precedence) -> Precedence {
    match first {
        Precedence::Equal => next,
        _ => first,
    }
}

/// Lexical comparison in ASCII order, byte by byte; a proper prefix is less.
fn compare_ascii(a: &str, b: &str) -> Precedence {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    for i in 0..x.len().min(y.len()) {
        let c = compare_u64(u64::from(x[i]), u64::from(y[i]));
        if !matches!(c, Precedence::Equal) {
            return c;
        }
    }
    match (x.len() < y.len(), x.len() > y.len()) {
        (true, _) => Precedence::Less,
        (_, true) => Precedence::Greater,
        _ => Precedence::Equal,
    }
}

fn compare_pre_id(a: &PreId, b: &PreId) -> Precedence {
    match (a, b) {
        (PreId::Numeric(x), PreId::Numeric(y)) => compare_u64(*x, *y),
        (PreId::Numeric(_), PreId::Alpha(_)) => Precedence::Less,
        (PreId::Alpha(_), PreId::Numeric(_)) => Precedence::Greater,
        (PreId::Alpha(x), PreId::Alpha(y)) => compare_ascii(x, y),
    }
}

/// Identifiers left to right; a longer list with an equal prefix is greater.
fn compare_pre_ids(a: &PreIds, b: &PreIds) -> Precedence {
    match (a, b) {
        (PreIds::Nil, PreIds::Nil) => Precedence::Equal,
        (PreIds::Nil, _) => Precedence::Less,
        (_, PreIds::Nil) => Precedence::Greater,
        (PreIds::Cons(x, xs), PreIds::Cons(y, ys)) => then(compare_pre_id(x, y), compare_pre_ids(xs, ys)),
    }
}

/// Precedence per SemVer 2.0.0 §11. Build metadata is ignored.
pub fn compare(a: &Version, b: &Version) -> Precedence {
    let pre = match (&a.pre, &b.pre) {
        (PreIds::Nil, PreIds::Nil) => Precedence::Equal,
        // A version without pre-release has higher precedence.
        (PreIds::Nil, _) => Precedence::Greater,
        (_, PreIds::Nil) => Precedence::Less,
        (x, y) => compare_pre_ids(x, y),
    };
    then(compare_u64(a.major, b.major), then(compare_u64(a.minor, b.minor), then(compare_u64(a.patch, b.patch), pre)))
}
