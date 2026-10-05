pub fn parse_iban(raw: String) -> Result<Iban, IbanError> {
    Iban::parse(raw)
}

/// The value read back through `as_str`.
pub fn iban_text(raw: String) -> Result<String, IbanError> {
    let iban = Iban::parse(raw)?;
    Ok(String::from(iban.as_str()))
}
