//! Validated locale identities used throughout Heine's locale-aware pipeline.

use std::{fmt, str::FromStr};

use serde::Deserialize;
use unic_langid::LanguageIdentifier;

/// The base writing direction configured for one locale.
///
/// A BCP-47 identifier does not reliably establish direction: a language can
/// be written in more than one script. Sites therefore choose it explicitly
/// instead of relying on a heuristic.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TextDirection {
    Ltr,
    Rtl,
}

impl TextDirection {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ltr => "ltr",
            Self::Rtl => "rtl",
        }
    }
}

/// A canonical BCP-47 locale configured by a Heine site.
///
/// This is deliberately distinct from filesystem and URL paths. Its canonical
/// string form is used only after configuration validation has established the
/// finite set of locales a site supports.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Locale(LanguageIdentifier);

impl fmt::Display for Locale {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for Locale {
    type Err = unic_langid::LanguageIdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_valid_bcp47_identifiers() {
        assert_eq!("de-at".parse::<Locale>().unwrap().to_string(), "de-AT");
    }

    #[test]
    fn rejects_invalid_bcp47_identifiers() {
        assert!("not a locale".parse::<Locale>().is_err());
    }

    #[test]
    fn text_directions_have_html_values() {
        assert_eq!(TextDirection::Ltr.as_str(), "ltr");
        assert_eq!(TextDirection::Rtl.as_str(), "rtl");
    }
}
