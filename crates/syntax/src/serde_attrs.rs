//! The `#[serde(...)]` keys the wire module models (design/04 §3.6), read
//! where each may be written; any other key changes the JSON in a way the
//! schemas and `toJson` would not follow, so it is refused with its place.

use purecrate_ir::Reason;
use syn::spanned::Spanned;

use crate::item::ParseError;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Struct,
    Enum,
    Field,
    Variant,
}

impl Place {
    fn keys(self) -> &'static [&'static str] {
        match self {
            Place::Struct => &["rename", "rename_all", "try_from"],
            Place::Enum => &["rename", "rename_all", "tag", "content"],
            Place::Field => &["rename"],
            Place::Variant => &["rename", "rename_all"],
        }
    }

    fn what(self) -> &'static str {
        match self {
            Place::Struct => "a struct",
            Place::Enum => "an enum",
            Place::Field => "a field",
            Place::Variant => "a variant",
        }
    }
}

#[derive(Default)]
pub(crate) struct SerdeAttrs {
    /// `rename = "x"`: on a field or a variant, its JSON name; on the
    /// container, only the name in serde's messages.
    pub rename: Option<String>,
    pub rename_all: Option<RenameRule>,
    pub tag: Option<String>,
    pub content: Option<String>,
    pub try_from: Option<syn::Type>,
}

const MODELED: &str =
    "the wire module models `rename`, `rename_all`, `tag`, `content`, and `try_from` (design/04 §3.6)";

/// The serde keys of `attrs`, written at `place`.
pub(crate) fn serde_attrs(attrs: &[syn::Attribute], place: Place) -> Result<SerdeAttrs, ParseError> {
    let mut out = SerdeAttrs::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let mut failure: Option<ParseError> = None;
        let parsed = attr.parse_nested_meta(|meta| {
            let key = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            let refuse = |message: String| ParseError::new(Reason::SerdeAttr, message).or_at(meta.path.span());
            if !place.keys().contains(&key.as_str()) {
                let message = if ["rename", "rename_all", "tag", "content", "try_from"].contains(&key.as_str()) {
                    format!(
                        "`#[serde({key})]` on {} is not modeled; {MODELED}, each where serde reads it",
                        place.what()
                    )
                } else {
                    format!(
                        "`#[serde({})]` changes the JSON in a way the wire module does not follow; {MODELED}",
                        if key.is_empty() { "..".to_string() } else { key.clone() }
                    )
                };
                failure = Some(refuse(message));
                return Ok(());
            }
            // `rename(serialize = .., deserialize = ..)` names the two
            // directions apart, which one schema and `toJson` cannot.
            if !meta.input.peek(syn::Token![=]) {
                failure = Some(refuse(format!(
                    "`#[serde({key} = \"..\")]` is modeled with one string, not split by direction"
                )));
                return Ok(());
            }
            let value: syn::LitStr = meta.value()?.parse()?;
            let twice = |set: bool| if set { Some(refuse(format!("`#[serde({key})]` is given twice"))) } else { None };
            match key.as_str() {
                "rename" => {
                    failure = twice(out.rename.is_some());
                    out.rename = Some(value.value());
                }
                "rename_all" => {
                    failure = twice(out.rename_all.is_some());
                    match RenameRule::of(&value.value()) {
                        Some(rule) => out.rename_all = Some(rule),
                        None => {
                            failure = Some(refuse(format!(
                                "unknown rule `rename_all = {:?}`; serde's are {}",
                                value.value(),
                                RenameRule::NAMES.map(|(n, _)| format!("{n:?}")).join(", ")
                            )))
                        }
                    }
                }
                "tag" => {
                    failure = twice(out.tag.is_some());
                    out.tag = Some(value.value());
                }
                "content" => {
                    failure = twice(out.content.is_some());
                    out.content = Some(value.value());
                }
                "try_from" => {
                    failure = twice(out.try_from.is_some());
                    match value.parse::<syn::Type>() {
                        Ok(ty) => out.try_from = Some(ty),
                        Err(_) => {
                            failure = Some(refuse(format!("`try_from = {:?}` does not name a type", value.value())))
                        }
                    }
                }
                _ => unreachable!("a key `place.keys()` lists"),
            }
            Ok(())
        });
        if let Some(e) = failure {
            return Err(e);
        }
        parsed.map_err(|e| {
            ParseError::new(Reason::SerdeAttr, format!("`#[serde(...)]` does not parse: {e}; {MODELED}"))
                .or_at(attr.span())
        })?;
    }
    if out.content.is_some() && out.tag.is_none() {
        let at = attrs.iter().find(|a| a.path().is_ident("serde")).map(|a| a.span());
        let e = ParseError::new(Reason::SerdeAttr, "`#[serde(content = \"..\")]` needs `tag` beside it, as serde does");
        return Err(match at {
            Some(span) => e.or_at(span),
            None => e,
        });
    }
    Ok(out)
}

/// serde's `rename_all` rules, applied as `serde_derive` applies them
/// (`internals/case.rs`): to a variant, from its PascalCase name; to a
/// field, from its snake_case name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names, reason = "serde_derive's own names for the rules")]
pub(crate) enum RenameRule {
    LowerCase,
    UpperCase,
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
}

impl RenameRule {
    const NAMES: [(&'static str, RenameRule); 8] = [
        ("lowercase", RenameRule::LowerCase),
        ("UPPERCASE", RenameRule::UpperCase),
        ("PascalCase", RenameRule::PascalCase),
        ("camelCase", RenameRule::CamelCase),
        ("snake_case", RenameRule::SnakeCase),
        ("SCREAMING_SNAKE_CASE", RenameRule::ScreamingSnakeCase),
        ("kebab-case", RenameRule::KebabCase),
        ("SCREAMING-KEBAB-CASE", RenameRule::ScreamingKebabCase),
    ];

    fn of(name: &str) -> Option<RenameRule> {
        Self::NAMES.iter().find(|(n, _)| *n == name).map(|(_, r)| *r)
    }

    pub(crate) fn apply_to_variant(self, variant: &str) -> String {
        use RenameRule::*;
        match self {
            PascalCase => variant.to_owned(),
            LowerCase => variant.to_ascii_lowercase(),
            UpperCase => variant.to_ascii_uppercase(),
            CamelCase => variant[..1].to_ascii_lowercase() + &variant[1..],
            SnakeCase => {
                let mut snake = String::new();
                for (i, ch) in variant.char_indices() {
                    if i > 0 && ch.is_uppercase() {
                        snake.push('_');
                    }
                    snake.push(ch.to_ascii_lowercase());
                }
                snake
            }
            ScreamingSnakeCase => SnakeCase.apply_to_variant(variant).to_ascii_uppercase(),
            KebabCase => SnakeCase.apply_to_variant(variant).replace('_', "-"),
            ScreamingKebabCase => ScreamingSnakeCase.apply_to_variant(variant).replace('_', "-"),
        }
    }

    pub(crate) fn apply_to_field(self, field: &str) -> String {
        use RenameRule::*;
        match self {
            LowerCase | SnakeCase => field.to_owned(),
            UpperCase | ScreamingSnakeCase => field.to_ascii_uppercase(),
            PascalCase => {
                let mut pascal = String::new();
                let mut capitalize = true;
                for ch in field.chars() {
                    if ch == '_' {
                        capitalize = true;
                    } else if capitalize {
                        pascal.push(ch.to_ascii_uppercase());
                        capitalize = false;
                    } else {
                        pascal.push(ch);
                    }
                }
                pascal
            }
            CamelCase => {
                let pascal = PascalCase.apply_to_field(field);
                pascal[..1].to_ascii_lowercase() + &pascal[1..]
            }
            KebabCase => field.replace('_', "-"),
            ScreamingKebabCase => ScreamingSnakeCase.apply_to_field(field).replace('_', "-"),
        }
    }
}

/// The JSON name of a field or variant, where it is not `ident` as
/// written: its own `rename`, else the container's rule applied to the
/// name without `r#`, else that name when `r#` was written.
pub(crate) fn wire_name(
    ident: &syn::Ident,
    own: Option<String>,
    rule: Option<RenameRule>,
    variant: bool,
) -> Option<String> {
    let name = syn::ext::IdentExt::unraw(ident).to_string();
    let wire = own.unwrap_or_else(|| match rule {
        Some(r) if variant => r.apply_to_variant(&name),
        Some(r) => r.apply_to_field(&name),
        None => name,
    });
    (*ident != wire).then_some(wire)
}

#[cfg(test)]
mod tests {
    use super::RenameRule::*;

    /// serde_derive's own table (`internals/case.rs`, `rename_variants` and
    /// `rename_fields`).
    #[test]
    fn rules_match_serde() {
        for (original, lower, upper, camel, snake, screaming, kebab, screaming_kebab) in [
            ("Outcome", "outcome", "OUTCOME", "outcome", "outcome", "OUTCOME", "outcome", "OUTCOME"),
            (
                "VeryTasty",
                "verytasty",
                "VERYTASTY",
                "veryTasty",
                "very_tasty",
                "VERY_TASTY",
                "very-tasty",
                "VERY-TASTY",
            ),
            ("A", "a", "A", "a", "a", "A", "a", "A"),
            ("Z42", "z42", "Z42", "z42", "z42", "Z42", "z42", "Z42"),
        ] {
            assert_eq!(LowerCase.apply_to_variant(original), lower);
            assert_eq!(UpperCase.apply_to_variant(original), upper);
            assert_eq!(PascalCase.apply_to_variant(original), original);
            assert_eq!(CamelCase.apply_to_variant(original), camel);
            assert_eq!(SnakeCase.apply_to_variant(original), snake);
            assert_eq!(ScreamingSnakeCase.apply_to_variant(original), screaming);
            assert_eq!(KebabCase.apply_to_variant(original), kebab);
            assert_eq!(ScreamingKebabCase.apply_to_variant(original), screaming_kebab);
        }
        for (original, upper, pascal, camel, screaming, kebab, screaming_kebab) in [
            ("outcome", "OUTCOME", "Outcome", "outcome", "OUTCOME", "outcome", "OUTCOME"),
            ("very_tasty", "VERY_TASTY", "VeryTasty", "veryTasty", "VERY_TASTY", "very-tasty", "VERY-TASTY"),
            ("a", "A", "A", "a", "A", "a", "A"),
            ("z42", "Z42", "Z42", "z42", "Z42", "z42", "Z42"),
        ] {
            assert_eq!(UpperCase.apply_to_field(original), upper);
            assert_eq!(PascalCase.apply_to_field(original), pascal);
            assert_eq!(CamelCase.apply_to_field(original), camel);
            assert_eq!(SnakeCase.apply_to_field(original), original);
            assert_eq!(ScreamingSnakeCase.apply_to_field(original), screaming);
            assert_eq!(KebabCase.apply_to_field(original), kebab);
            assert_eq!(ScreamingKebabCase.apply_to_field(original), screaming_kebab);
        }
    }
}
