//! Parsing of JSON and TOML source documents into template-compatible values.
//!
//! Callers retain ownership of file-specific policy and diagnostics; this
//! module owns only the shared syntax decoding and parser-location details.

use std::path::Path;

use miette::SourceSpan;
use serde_json::Value;

use crate::source_diagnostic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Json,
    Toml,
}

impl Format {
    pub(crate) fn from_path(path: &Path) -> Option<Self> {
        match path.extension().and_then(|extension| extension.to_str()) {
            Some("json") => Some(Self::Json),
            Some("toml") => Some(Self::Toml),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Toml => "TOML",
        }
    }
}

#[derive(Debug)]
pub(crate) struct ParseError {
    pub(crate) message: String,
    pub(crate) span: SourceSpan,
}

pub(crate) fn parse(format: Format, source: &str) -> Result<Value, ParseError> {
    match format {
        Format::Json => serde_json::from_str(source).map_err(|error| ParseError {
            message: error.to_string(),
            span: source_diagnostic::json_span(source, &error),
        }),
        Format::Toml => toml::from_str::<toml::Value>(source)
            .map(|value| serde_json::to_value(value).expect("TOML values are serializable"))
            .map_err(|error| ParseError {
                message: error.message().to_owned(),
                span: error.span().unwrap_or(0..0).into(),
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_extensions_only() {
        assert_eq!(
            Format::from_path(Path::new("value.json")),
            Some(Format::Json)
        );
        assert_eq!(
            Format::from_path(Path::new("value.toml")),
            Some(Format::Toml)
        );
        assert_eq!(Format::from_path(Path::new("value.yaml")), None);
    }

    #[test]
    fn converts_toml_to_a_template_value() {
        assert_eq!(
            parse(Format::Toml, "title = 'Heine'").expect("valid TOML"),
            serde_json::json!({ "title": "Heine" })
        );
    }
}
