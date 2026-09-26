/// Flattened identifier. Module paths are already erased.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Name(pub String);

impl Name {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// File stem for kamae-ts one-concept-per-file emit.
    /// `TaxiRequest` → `taxi-request`, `step` → `step`.
    pub fn file_stem(&self) -> String {
        to_kebab(&self.0)
    }
}

pub fn to_kebab(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().copied().enumerate() {
        if c == '_' {
            if !out.is_empty() && !out.ends_with('-') {
                out.push('-');
            }
            continue;
        }
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next_lower = chars
                    .get(i + 1)
                    .copied()
                    .map(|x| x.is_lowercase())
                    .unwrap_or(false);
                if prev.is_lowercase() || prev == '_' || (prev.is_uppercase() && next_lower) {
                    if !out.ends_with('-') {
                        out.push('-');
                    }
                }
            }
            for x in c.to_lowercase() {
                out.push(x);
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_examples() {
        assert_eq!(to_kebab("Event"), "event");
        assert_eq!(to_kebab("State"), "state");
        assert_eq!(to_kebab("TaxiRequest"), "taxi-request");
        assert_eq!(to_kebab("HTTPError"), "http-error");
        assert_eq!(to_kebab("step"), "step");
        assert_eq!(to_kebab("assign_driver"), "assign-driver");
    }
}
