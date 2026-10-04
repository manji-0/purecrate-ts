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

/// The TS spelling of a Rust function, method, parameter, or local:
/// `compare_pre_ids` → `comparePreIds`. Only an `_` between a letter or
/// digit and a lowercase letter goes, the letter raised, so a leading or
/// trailing `_` (`_unused`, `type_`), `_` before a digit (`a_1`), and an
/// UPPER_SNAKE const stay as they are, and a name never becomes a keyword
/// it was not.
pub fn to_camel(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let joins = c == '_'
            && i > 0
            && chars[i - 1].is_ascii_alphanumeric()
            && chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
        if joins {
            out.push(chars[i + 1].to_ascii_uppercase());
            i += 2;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
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
                let next_lower = chars.get(i + 1).copied().map(|x| x.is_lowercase()).unwrap_or(false);
                if (prev.is_lowercase() || prev == '_' || (prev.is_uppercase() && next_lower)) && !out.ends_with('-') {
                    out.push('-');
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
    fn camel_examples() {
        assert_eq!(to_camel("compare_pre_ids"), "comparePreIds");
        assert_eq!(to_camel("is_pre_release"), "isPreRelease");
        assert_eq!(to_camel("step"), "step");
        assert_eq!(to_camel("_unused"), "_unused");
        assert_eq!(to_camel("type_"), "type_");
        assert_eq!(to_camel("a_1"), "a_1");
        assert_eq!(to_camel("MAX_STATE_LEN"), "MAX_STATE_LEN");
        assert_eq!(to_camel("i$1"), "i$1");
        assert_eq!(to_camel("$q1"), "$q1");
        assert_eq!(to_kebab(&to_camel("compare_pre_ids")), "compare-pre-ids");
    }

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
