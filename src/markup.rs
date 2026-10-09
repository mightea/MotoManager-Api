//! Server rules for the formatted twins of free-text columns (`description` /
//! `descriptionMarkup`, `notes` / `notesMarkup`, …; migrations 054 and 055).
//!
//! The plain column is the compatibility surface older iOS builds read and
//! write. The markup column carries the same text with `**bold**`, `*italic*`
//! and `[red]`/`[yellow]`/`[blue]` tags; clients only honour it while
//! stripping it yields the plain text. The server never parses markup — it
//! only decides whether a stored markup is still trustworthy after an update.

use serde::{Deserialize, Deserializer};

/// Empty markup is stored as NULL so "no formatting" has one representation.
pub fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// serde helper distinguishing "field absent" (`None`) from an explicit
/// `null` (`Some(None)`) for `Option<Option<String>>` fields.
pub fn double_option_string<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Some(Option::<String>::deserialize(deserializer)?))
}

/// Resolve the markup column for an update.
///
/// * markup sent (value or explicit null) → it wins (blank stored as NULL);
/// * markup absent and the resolved plain text differs from what was stored →
///   an older client edited the entry, the stale formatting is dropped;
/// * markup absent and plain text unchanged → the stored markup survives.
pub fn resolve_markup(
    sent_markup: Option<Option<String>>,
    new_plain: Option<&str>,
    existing_plain: Option<&str>,
    existing_markup: Option<String>,
) -> Option<String> {
    match sent_markup {
        Some(markup) => non_empty(markup),
        None if new_plain != existing_plain => None,
        None => existing_markup,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn sent_markup_wins_and_blank_becomes_null() {
        assert_eq!(
            resolve_markup(Some(s("**x**")), Some("y"), Some("x"), s("old")),
            s("**x**")
        );
        assert_eq!(
            resolve_markup(Some(s("  ")), Some("x"), Some("x"), s("old")),
            None
        );
        assert_eq!(
            resolve_markup(Some(None), Some("x"), Some("x"), s("old")),
            None
        );
    }

    #[test]
    fn absent_markup_follows_the_plain_text() {
        assert_eq!(
            resolve_markup(None, Some("x"), Some("x"), s("**x**")),
            s("**x**")
        );
        assert_eq!(
            resolve_markup(None, Some("x2"), Some("x"), s("**x**")),
            None
        );
        assert_eq!(resolve_markup(None, None, Some("x"), s("**x**")), None);
        assert_eq!(resolve_markup(None, None, None, None), None);
    }

    #[test]
    fn double_option_distinguishes_absent_from_null() {
        #[derive(Deserialize)]
        struct Body {
            #[serde(default, deserialize_with = "double_option_string")]
            markup: Option<Option<String>>,
        }
        let absent: Body = serde_json::from_str("{}").unwrap();
        let null: Body = serde_json::from_str(r#"{"markup":null}"#).unwrap();
        let value: Body = serde_json::from_str(r#"{"markup":"**a**"}"#).unwrap();
        assert_eq!(absent.markup, None);
        assert_eq!(null.markup, Some(None));
        assert_eq!(value.markup, Some(s("**a**")));
    }
}
