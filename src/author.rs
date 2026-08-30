//! Authored human names shared by pages and Atom feed fallbacks.

use serde::Serialize;

/// A nonblank display name. Heine deliberately does not model contact or
/// profile data until a concrete author-registry feature needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(crate) struct AuthorName(String);

impl AuthorName {
    pub(crate) fn parse(value: String) -> Result<Self, String> {
        if value.trim().is_empty() {
            Err("author names must not be empty or whitespace".into())
        } else {
            Ok(Self(value))
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::AuthorName;

    #[test]
    fn rejects_blank_names_without_rewriting_authored_names() {
        assert!(AuthorName::parse("   ".into()).is_err());
        assert_eq!(
            AuthorName::parse("Ada Lovelace".into())
                .expect("non-blank name")
                .as_str(),
            "Ada Lovelace"
        );
    }
}
