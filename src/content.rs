use std::{collections::BTreeMap, fs, path::Path};

use miette::{Diagnostic, NamedSource, SourceSpan};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::structured;

#[derive(Debug, Diagnostic, Error)]
pub(crate) enum Error {
    #[error("could not read content file {path}")]
    Read {
        path: Box<Path>,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    #[diagnostic(transparent)]
    Invalid(#[from] Box<ContentDiagnostic>),
}

/// Parsed content plus source retained for validations that need the resolved
/// site asset catalog.
pub(crate) struct LoadedContent {
    pub(crate) value: Value,
    pub(crate) markdown_source: Option<MarkdownSource>,
}

#[derive(Debug, Clone)]
pub(crate) struct MarkdownSource {
    pub(crate) display_path: Box<Path>,
    pub(crate) source: String,
}

#[derive(Debug, Diagnostic, Error)]
#[error("invalid {format} content in {path}")]
#[diagnostic(code(heine::content::invalid))]
pub(crate) struct ContentDiagnostic {
    path: Box<Path>,
    format: &'static str,
    message: String,
    #[source_code]
    source_code: NamedSource<String>,
    #[label("{message}")]
    span: SourceSpan,
}

pub(crate) fn load(
    path: &Path,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<LoadedContent, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.into(),
        source,
    })?;
    let value = match structured::Format::from_path(path) {
        Some(format) => structured::parse(format, &text).map_err(|error| {
            invalid_diagnostic(
                path,
                format.name(),
                &text,
                error.message,
                error.span,
                diagnostics,
            )
        }),
        None => process_text(&text, path, diagnostics),
    }?;
    Ok(LoadedContent {
        value,
        markdown_source: path
            .extension()
            .is_some_and(|extension| extension == "md")
            .then(|| MarkdownSource {
                display_path: diagnostics.display_path(path).into_boxed_path(),
                source: text,
            }),
    })
}

fn invalid_diagnostic(
    path: &Path,
    format: &'static str,
    source: &str,
    message: String,
    span: SourceSpan,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Error {
    Error::Invalid(Box::new(ContentDiagnostic {
        path: diagnostics.display_path(path).into_boxed_path(),
        format,
        message,
        source_code: diagnostics.named_source(path, source, format),
        span,
    }))
}

fn process_text(
    text: &str,
    path: &Path,
    diagnostics: &crate::source_diagnostic::DiagnosticPaths,
) -> Result<Value, Error> {
    let mut lines = text.split_inclusive('\n');
    let Some(first_raw) = lines.next() else {
        return Ok(Value::String(text.into()));
    };
    let first = line_without_ending(first_raw);
    let Some(identifier) = segment_marker(first) else {
        return Ok(Value::String(text.into()));
    };
    let mut segments: BTreeMap<String, (Vec<String>, SourceSpan)> = BTreeMap::new();
    let mut current = identifier.to_owned();
    let mut current_span = (0, first.len()).into();
    let mut body = String::new();
    let mut offset = first_raw.len();
    for raw_line in lines {
        let line = line_without_ending(raw_line);
        if let Some(next) = segment_marker(line) {
            segments
                .entry(current.clone())
                .or_insert_with(|| (Vec::new(), current_span))
                .0
                .push(body.trim().to_owned());
            current = next.to_owned();
            current_span = (offset, line.len()).into();
            body.clear();
        } else {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(line);
        }
        offset += raw_line.len();
    }
    segments
        .entry(current)
        .or_insert_with(|| (Vec::new(), current_span))
        .0
        .push(body.trim().to_owned());
    let mut root = Map::new();
    for (key, (values, marker_span)) in segments {
        let value = if values.len() == 1 {
            Value::String(values.into_iter().next().expect("one item"))
        } else {
            Value::Array(values.into_iter().map(Value::String).collect())
        };
        insert_dotted(&mut root, &key, value).ok_or_else(|| {
            invalid_diagnostic(
                path,
                "segmented text",
                text,
                format!("segment identifier {key:?} is not a valid dotted TOML key"),
                marker_span,
                diagnostics,
            )
        })?;
    }
    Ok(Value::Object(root))
}

fn line_without_ending(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn segment_marker(line: &str) -> Option<&str> {
    let line = line.trim();
    Some(
        line.strip_prefix("<!--")?
            .strip_suffix("-->")?
            .trim()
            .strip_prefix("segment:")?
            .trim(),
    )
}

fn insert_dotted(root: &mut Map<String, Value>, key: &str, value: Value) -> Option<()> {
    let keys: Vec<_> = key.split('.').collect();
    if keys.is_empty()
        || keys.iter().any(|key| {
            key.is_empty() || !key.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
        })
    {
        return None;
    }
    let (last, parents) = keys.split_last()?;
    let mut map = root;
    for key in parents {
        if !map.contains_key(*key) {
            map.insert((*key).to_owned(), Value::Object(Map::new()));
        }
        map = map.get_mut(*key)?.as_object_mut()?;
    }
    map.insert((*last).to_owned(), value);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segmented_content_becomes_structured_values() {
        let value = process_text(
            "<!-- segment: a.b -->\none\n<!-- segment: a.b -->\ntwo",
            Path::new("test.md"),
            &crate::source_diagnostic::DiagnosticPaths::new(Path::new("")),
        )
        .unwrap();
        assert_eq!(value["a"]["b"], serde_json::json!(["one", "two"]));
    }
}
