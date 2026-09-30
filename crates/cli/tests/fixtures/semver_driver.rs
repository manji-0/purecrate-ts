pub fn parse_version(raw: String) -> Result<Version, SemverError> {
    Version::parse(raw.as_str())
}

pub fn compare_versions(a: String, b: String) -> Result<Ordering, SemverError> {
    let x = Version::parse(a.as_str())?;
    let y = Version::parse(b.as_str())?;
    Ok(compare(&x, &y))
}
