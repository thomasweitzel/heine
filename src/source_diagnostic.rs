//! Shared source-location support for structured user-authored files.

use miette::{NamedSource, SourceSpan};
use std::path::Path;

pub(crate) fn named_source(
    path: &Path,
    source: impl Into<String>,
    language: &'static str,
) -> NamedSource<String> {
    NamedSource::new(path.display().to_string(), source.into()).with_language(language)
}

pub(crate) fn json_span(source: &str, error: &serde_json::Error) -> SourceSpan {
    let line_start = source
        .lines()
        .take(error.line().saturating_sub(1))
        .map(|line| line.len() + 1)
        .sum::<usize>();
    let offset = line_start + error.column().saturating_sub(1);
    (offset, 1).into()
}
