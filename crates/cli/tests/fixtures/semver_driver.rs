pub fn parse_version(raw: String) -> Result<Version, SemverError> {
    Version::parse(raw.as_str())
}

pub fn compare_versions(a: String, b: String) -> Result<Ordering, SemverError> {
    let x = Version::parse(a.as_str())?;
    let y = Version::parse(b.as_str())?;
    Ok(compare(&x, &y))
}

pub fn format_version(raw: String) -> Result<String, SemverError> {
    let v = Version::parse(raw.as_str())?;
    Ok(v.format())
}

pub fn equal_versions(a: String, b: String) -> Result<(bool, bool), SemverError> {
    let x = Version::parse(a.as_str())?;
    let y = Version::parse(b.as_str())?;
    Ok((equal(&x, &y), same_precedence(&x, &y)))
}
