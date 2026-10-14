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
    /// Read through the TS spelling, so a function's name gives one stem
    /// before renaming and after (`mk_A_b` and `mk_AB`).
    pub fn file_stem(&self) -> String {
        to_kebab(&to_camel(&self.0))
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

/// `s` with its first letter lowercased: `Yen` → `yen`.
pub fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().chain(c).collect()).unwrap_or_default()
}

/// `s` with its first letter uppercased: `yen` → `Yen`.
pub fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
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
                // After a digit too: `to_camel` raises the letter after
                // `q62_`, and `q62ModPath` must be `q62_mod_path`'s file.
                let after_word = prev.is_lowercase() || prev.is_ascii_digit() || prev == '_';
                if (after_word || (prev.is_uppercase() && next_lower)) && !out.ends_with('-') {
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
        assert_eq!(to_kebab("Base64Error"), "base64-error");
        assert_eq!(to_kebab("HTTP2Error"), "http2-error");
        assert_eq!(to_kebab("V2"), "v2");
    }

    /// A function's file is named after its Rust name before renaming and
    /// its camelCase name after (`Crate::homes` is found both ways): the
    /// two must give one stem, digits next to letters included.
    #[test]
    fn kebab_of_camel_is_kebab() {
        for name in [
            "q62_mod_path",
            "base64_decode",
            "utf8_len",
            "sha256_sum",
            "a1_b2_c",
            "a_1_b",
            "a_1b",
            "x2y_z",
            "parse_v2",
            "v2_parse",
            "compare_pre_ids",
            "get_HTTP_code",
        ] {
            assert_eq!(to_kebab(&to_camel(name)), to_kebab(name), "{name}");
        }
    }
}
