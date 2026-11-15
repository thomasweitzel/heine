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
    pub(crate) markdown_source: Option<MarkdownInput>,
}

/// The authored source is kept separately only when content is segmented.
#[derive(Debug, Clone)]
pub(crate) struct MarkdownInput {
    pub(crate) display_path: Box<Path>,
    separate_source: Option<String>,
}

impl MarkdownInput {
    pub(crate) fn text<'a>(&'a self, value: &'a Value) -> &'a str {
        self.separate_source.as_deref().unwrap_or_else(|| {
            value
                .as_str()
                .expect("unsegmented Markdown content remains a string")
        })
    }
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
    let is_markdown = path.extension().is_some_and(|extension| extension == "md");
    if is_markdown
        && text
            .split_inclusive('\n')
            .next()
            .is_none_or(|line| segment_marker(line_without_ending(line)).is_none())
    {
        return Ok(LoadedContent {
            value: Value::String(text),
            markdown_source: Some(MarkdownInput {
                display_path: diagnostics.display_path(path).into_boxed_path(),
                separate_source: None,
            }),
        });
    }
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
    let markdown_source = is_markdown.then(|| MarkdownInput {
        display_path: diagnostics.display_path(path).into_boxed_path(),
        separate_source: Some(text),
    });
    Ok(LoadedContent {
        value,
        markdown_source,
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
    let Some(source_segments) = source_segments(text) else {
        return Ok(Value::String(text.into()));
    };
    let mut segments: BTreeMap<String, (Vec<String>, SourceSpan)> = BTreeMap::new();
    for segment in source_segments {
        let mut body = String::new();
        for raw_line in text[segment.body].split_inclusive('\n') {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(line_without_ending(raw_line));
        }
        segments
            .entry(segment.name.to_owned())
            .or_insert_with(|| (Vec::new(), segment.marker.into()))
            .0
            .push(body.trim().to_owned());
    }
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

struct SourceSegment<'a> {
    name: &'a str,
    marker: std::ops::Range<usize>,
    body: std::ops::Range<usize>,
}

fn source_segments(text: &str) -> Option<Vec<SourceSegment<'_>>> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    let first_line = line_without_ending(first);
    let mut name = segment_marker(first_line)?;
    let mut marker = 0..first_line.len();
    let mut body_start = first.len();
    let mut offset = body_start;
    let mut segments = Vec::new();
    for raw_line in lines {
        let line = line_without_ending(raw_line);
        if let Some(next) = segment_marker(line) {
            segments.push(SourceSegment {
                name,
                marker,
                body: body_start..offset,
            });
            name = next;
            marker = offset..offset + line.len();
            body_start = offset + raw_line.len();
        }
        offset += raw_line.len();
    }
    segments.push(SourceSegment {
        name,
        marker,
        body: body_start..text.len(),
    });
    Some(segments)
}

/// Source ranges for segment bodies, in authored order.
pub(crate) fn segment_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    source_segments(text)
        .unwrap_or_default()
        .into_iter()
        .map(|segment| {
            let body = &text[segment.body.clone()];
            let trimmed_start = body.trim_start();
            let start = segment.body.start + body.len() - trimmed_start.len();
            start..start + trimmed_start.trim_end().len()
        })
        .collect()
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
    fn markdown_keeps_one_source_for_plain_text_and_preserves_segmented_source() {
        let root = tempfile::tempdir().expect("content directory");
        let diagnostics = crate::source_diagnostic::DiagnosticPaths::new(root.path());
        let path = root.path().join("content.md");
        std::fs::write(&path, "Ordinary Markdown.\n").expect("plain source");
        let plain = load(&path, &diagnostics).expect("plain content");
        let plain_source = plain.markdown_source.as_ref().expect("Markdown input");
        assert!(plain_source.separate_source.is_none());
        assert_eq!(plain_source.text(&plain.value), "Ordinary Markdown.\n");
        assert_eq!(
            plain_source.text(&plain.value).as_ptr(),
            plain.value.as_str().unwrap().as_ptr()
        );

        let original = "<!-- segment: intro -->\nSegment text.\n";
        std::fs::write(&path, original).expect("segmented source");
        let segmented = load(&path, &diagnostics).expect("segmented content");
        let segmented_source = segmented.markdown_source.as_ref().expect("Markdown input");
        assert_eq!(segmented_source.text(&segmented.value), original);
        assert_eq!(segmented.value["intro"], "Segment text.");
    }

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

    #[test]
    fn segment_ranges_match_rendered_values_with_crlf_and_empty_segments() {
        let text = "<!-- segment: items -->\r\n  First\r\n\r\nSecond  \r\n<!-- segment: items -->\r\n\r\n<!-- segment: items -->\r\nLast";
        let value = process_text(
            text,
            Path::new("test.md"),
            &crate::source_diagnostic::DiagnosticPaths::new(Path::new("")),
        )
        .expect("segmented content");
        let bodies: Vec<_> = segment_ranges(text)
            .into_iter()
            .map(|range| text[range].replace("\r\n", "\n"))
            .collect();
        assert_eq!(value["items"], serde_json::json!(bodies));
    }
}
