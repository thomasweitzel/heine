//! Parsing and planning for Markdown block components.
//!
//! A component directive is structural Markdown syntax.  This module removes
//! only its delimiter lines before the ordinary parser sees the document, so
//! its authored body remains part of the one document-wide Markdown pass.

use std::{collections::BTreeMap, ops::Range};

use pulldown_cmark::{Event, Parser, Tag};
use serde_json::Value;

use super::{
    RenderError,
    directive::{
        BlockDirectiveKind, block_directive_opening, closing_code_fence, closing_fence,
        is_indented_code_line, is_raw_html_at, next_line, opening_code_fence, raw_html_ranges,
        trim_cr, unsupported_block_placement,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct ComponentCall {
    pub(crate) span: Range<usize>,
    pub(crate) header_span: Range<usize>,
    pub(crate) body_span: Range<usize>,
    pub(crate) name: String,
    pub(crate) arguments: BTreeMap<String, Value>,
    pub(crate) body_has_headings: bool,
}

/// A failure while Heine renders one already-planned component body.
///
/// The distinction lets the template boundary retain Tera's own source span
/// while the Markdown renderer continues to retain authored Markdown spans.
#[derive(Debug)]
pub(crate) enum ComponentRenderFailure {
    Contract(String),
    Tera(tera::Error),
}

/// A component-rendering failure with either its Markdown cause or the call
/// whose component failed.
#[derive(Debug)]
pub(crate) enum ComponentRenderError {
    Markdown(RenderError),
    Component {
        call: Box<ComponentCall>,
        failure: Box<ComponentRenderFailure>,
    },
}

/// The Markdown source with directive delimiters made inert and the calls that
/// later replace their already-rendered bodies.
#[derive(Debug, Clone)]
pub(crate) struct ComponentPlan {
    normalized: String,
    calls: Vec<ComponentCall>,
}

impl ComponentPlan {
    pub(crate) fn parse(input: &str) -> Result<Option<Self>, RenderError> {
        // Most Markdown inputs contain no block-directive prefix. Avoid a
        // structural parse and the component scan in that common case.
        if !input.contains(":::") {
            return Ok(None);
        }
        Self::check_placement(input)?;
        Self::parse_for_planning(input)
    }

    pub(crate) fn check_placement(input: &str) -> Result<(), RenderError> {
        if let Some(placement) = unsupported_block_placement(input) {
            return Err(RenderError::DirectivePlacement {
                container: placement.container,
                span: placement.span,
            });
        }
        Ok(())
    }

    /// Plans top-level components without advancing unrelated directive errors.
    pub(crate) fn parse_for_planning(input: &str) -> Result<Option<Self>, RenderError> {
        let mut calls = Vec::new();
        let mut offset = 0;
        let mut code_fence = None;
        let raw_html = if input.contains(":::component[") {
            raw_html_ranges(input)
        } else {
            Vec::new()
        };
        while offset < input.len() {
            let line_end = input[offset..]
                .find('\n')
                .map_or(input.len(), |index| offset + index);
            let line = trim_cr(&input[offset..line_end]);
            if let Some((marker, length)) = code_fence {
                if closing_code_fence(line, marker).is_some_and(|run| run >= length) {
                    code_fence = None;
                }
                offset = next_line(input, line_end);
                continue;
            }
            if is_raw_html_at(&raw_html, offset) {
                offset = next_line(input, line_end);
                continue;
            }
            if let Some(fence) = opening_code_fence(line) {
                code_fence = Some(fence);
                offset = next_line(input, line_end);
                continue;
            }
            if is_indented_code_line(line) {
                offset = next_line(input, line_end);
                continue;
            }
            if let Some(run) = component_fence(line) {
                let (call, next) = parse_call(input, offset, line_end, run)?;
                calls.push(call);
                offset = next;
                continue;
            }
            offset = next_line(input, line_end);
        }
        if calls.is_empty() {
            return Ok(None);
        }
        let mut normalized = input.as_bytes().to_vec();
        for call in calls.iter().rev() {
            blank_non_newlines(&mut normalized, call.span.start..call.body_span.start);
            blank_non_newlines(&mut normalized, call.body_span.end..call.span.end);
        }
        let normalized = String::from_utf8(normalized)
            .expect("replacing source bytes with ASCII whitespace preserves UTF-8");
        Ok(Some(Self { normalized, calls }))
    }

    /// Preserve authored header spans for diagnostics while rebasing body
    /// ranges to the Markdown text containing renderer markers.
    pub(crate) fn rebase(&mut self, offsets: &super::rebase::Rebase) -> Result<(), Range<usize>> {
        for call in &mut self.calls {
            call.body_span = offsets.range(&call.body_span, true)?;
        }
        Ok(())
    }

    pub(crate) fn normalized(&self) -> &str {
        &self.normalized
    }

    pub(crate) fn calls(&self) -> &[ComponentCall] {
        &self.calls
    }
}

fn parse_call(
    input: &str,
    start: usize,
    line_end: usize,
    fence_length: usize,
) -> Result<(ComponentCall, usize), RenderError> {
    let header_start = start + fence_length + "component[".len();
    let header_line = trim_cr(&input[start..line_end]);
    let header_end = start + header_line.len() - 1;
    let header = &input[header_start..header_end];
    let (name, arguments) = parse_header(header, header_start)?;
    let body_start = next_line(input, line_end);
    let mut offset = body_start;
    let mut code_fence = None;
    while offset < input.len() {
        let close_end = input[offset..]
            .find('\n')
            .map_or(input.len(), |index| offset + index);
        let line = trim_cr(&input[offset..close_end]);
        if let Some((marker, length)) = code_fence {
            if closing_code_fence(line, marker).is_some_and(|run| run >= length) {
                code_fence = None;
            }
            offset = next_line(input, close_end);
            continue;
        }
        if let Some(fence) = opening_code_fence(line) {
            code_fence = Some(fence);
            offset = next_line(input, close_end);
            continue;
        }
        if closing_fence(line).is_some_and(|run| run >= fence_length) {
            let body_end = if offset == body_start {
                offset
            } else {
                offset - 1
            };
            if input[body_start..body_end].trim().is_empty() {
                return Err(error(
                    "Markdown block components require an authored Markdown body",
                    body_start..body_end,
                ));
            }
            let span = start..next_line(input, close_end);
            let body_span = body_start..body_end;
            if contains_footnote_definition(&input[body_span.clone()]) {
                return Err(error(
                    "a Markdown block component body cannot define footnotes",
                    body_span,
                ));
            }
            let renderer_requests = super::renderer_request::parse(&input[body_span.clone()])
                .map_err(|_| {
                    error(
                        "Markdown block component bodies cannot contain renderer requests",
                        body_span.clone(),
                    )
                })?;
            if !renderer_requests.is_empty() {
                return Err(error(
                    "Markdown block component bodies cannot contain renderer requests",
                    body_span,
                ));
            }
            let body_has_headings = Parser::new(&input[body_span.clone()])
                .any(|event| matches!(event, Event::Start(Tag::Heading { .. })));
            return Ok((
                ComponentCall {
                    span,
                    header_span: header_start..header_end,
                    body_span,
                    name,
                    arguments,
                    body_has_headings,
                },
                next_line(input, close_end),
            ));
        }
        if component_fence(line).is_some() {
            return Err(error(
                "Markdown block component directives cannot nest",
                offset..close_end,
            ));
        }
        offset = next_line(input, close_end);
    }
    Err(error(
        "Markdown block component directive has no closing fence",
        start..line_end,
    ))
}

fn parse_header(
    header: &str,
    offset: usize,
) -> Result<(String, BTreeMap<String, Value>), RenderError> {
    let mut pieces = header.splitn(2, char::is_whitespace);
    let name = pieces.next().unwrap_or_default();
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(error(
            "component name must be a Tera component name",
            offset..offset + header.len(),
        ));
    }
    let rest = pieces.next().unwrap_or_default().trim();
    if rest.is_empty() {
        return Ok((name.to_owned(), BTreeMap::new()));
    }
    let document = toml::from_str::<toml::Table>(&format!(
        "value = {{ {} }}",
        separate_arguments(rest)
    ))
    .map_err(|_| {
        error(
            "component arguments must use literal TOML scalar values, such as kind = \"warning\"",
            offset..offset + header.len(),
        )
    })?;
    let mut arguments = BTreeMap::new();
    let values = document
        .get("value")
        .and_then(toml::Value::as_table)
        .expect("component header parses a TOML inline table");
    for (key, value) in values {
        let value = match value {
            toml::Value::String(value) => Value::String(value.clone()),
            toml::Value::Integer(value) => Value::Number((*value).into()),
            toml::Value::Float(value) if value.is_finite() => serde_json::Number::from_f64(*value)
                .map(Value::Number)
                .ok_or_else(|| {
                    error(
                        "component arguments must be finite scalar values",
                        offset..offset + header.len(),
                    )
                })?,
            toml::Value::Boolean(value) => Value::Bool(*value),
            _ => {
                return Err(error(
                    "component arguments must be strings, numbers, or booleans",
                    offset..offset + header.len(),
                ));
            }
        };
        arguments.insert(key.clone(), value);
    }
    Ok((name.to_owned(), arguments))
}

fn separate_arguments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut quoted = None;
    let mut offset = 0;
    while offset < input.len() {
        let character = input[offset..]
            .chars()
            .next()
            .expect("offset remains inside UTF-8 input");
        match (quoted, character) {
            (Some(_), '\\') => {
                output.push(character);
                offset += character.len_utf8();
                if let Some(escaped) = input[offset..].chars().next() {
                    output.push(escaped);
                    offset += escaped.len_utf8();
                }
                continue;
            }
            (Some(quote), value) if value == quote => quoted = None,
            (None, '"' | '\'') => quoted = Some(character),
            (None, value) if value.is_whitespace() && starts_next_argument(&input[offset..]) => {
                output.push(',');
                offset += value.len_utf8();
                continue;
            }
            _ => {}
        }
        output.push(character);
        offset += character.len_utf8();
    }
    output
}

fn starts_next_argument(input: &str) -> bool {
    let trimmed = input.trim_start();
    let Some(first) = trimmed.chars().next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    let name_end = trimmed
        .char_indices()
        .find_map(|(index, character)| {
            (!(character.is_ascii_alphanumeric() || character == '_')).then_some(index)
        })
        .unwrap_or(trimmed.len());
    trimmed[name_end..].trim_start().starts_with('=')
}

fn contains_footnote_definition(body: &str) -> bool {
    let mut code_fence = None;
    for line in body.lines().map(trim_cr) {
        if let Some((marker, length)) = code_fence {
            if closing_code_fence(line, marker).is_some_and(|run| run >= length) {
                code_fence = None;
            }
            continue;
        }
        if let Some(fence) = opening_code_fence(line) {
            code_fence = Some(fence);
            continue;
        }
        if is_indented_code_line(line) {
            continue;
        }
        let line = line.trim_start();
        if line.starts_with("[^") && line.contains("]:") {
            return true;
        }
    }
    false
}

fn blank_non_newlines(input: &mut [u8], range: Range<usize>) {
    // Directive syntax is ASCII. Replacing every other byte preserves UTF-8
    // because the range never includes the authored body.
    for byte in &mut input[range] {
        if *byte != b'\n' && *byte != b'\r' {
            *byte = b' ';
        }
    }
}

fn component_fence(line: &str) -> Option<usize> {
    block_directive_opening(line)
        .filter(|header| header.kind == BlockDirectiveKind::Component)
        .map(|header| header.fence_length)
}
fn error(message: impl Into<String>, span: Range<usize>) -> RenderError {
    RenderError::Component {
        message: message.into(),
        span,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_body_offsets_and_literal_arguments() {
        let source = "Before\n\n::::component[site.note kind = \"warning note\" count = 2]\n## Inside\n::::\n";
        let plan = ComponentPlan::parse(source)
            .expect("component syntax is valid")
            .expect("source has one component");
        let call = &plan.calls()[0];
        assert_eq!(&source[call.body_span.clone()], "## Inside");
        assert_eq!(call.arguments["kind"], Value::String("warning note".into()));
        assert_eq!(call.arguments["count"], Value::Number(2.into()));
        assert_eq!(
            &plan.normalized()[call.body_span.clone()],
            &source[call.body_span.clone()]
        );
        assert!(!plan.normalized().contains("component["));
    }

    #[test]
    fn component_body_keeps_citation_text_in_document_order_with_source_offsets() {
        let source = "Before [@first].\n\n:::component[site.note]\nInside [@second].\n:::\n\nAfter [@third].\n";
        let plan = ComponentPlan::parse(source)
            .expect("valid component")
            .expect("component is present");
        let found: Vec<_> = Parser::new(plan.normalized())
            .into_offset_iter()
            .filter_map(|(event, span)| match event {
                Event::Text(text) if text.contains('@') => {
                    Some((text.into_string(), source[span].to_owned()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(found.len(), 3, "{found:?}");
        for (index, key) in ["@first", "@second", "@third"].iter().enumerate() {
            assert!(found[index].0.contains(key), "{found:?}");
            assert!(found[index].1.contains(key), "{found:?}");
        }
    }

    #[test]
    fn accepts_crlf_component_fences_and_headers() {
        let source = ":::component[site.note kind = \"warning\"]\r\nBody\r\n:::\r\n";
        let plan = ComponentPlan::parse(source)
            .expect("CRLF component syntax is valid")
            .expect("source has one component");
        let call = &plan.calls()[0];
        assert_eq!(call.name, "site.note");
        assert_eq!(call.arguments["kind"], Value::String("warning".into()));
        assert_eq!(&source[call.body_span.clone()], "Body\r");
    }

    #[test]
    fn ignores_directives_inside_a_code_fence_and_rejects_empty_bodies() {
        let literal = "```markdown\n:::component[site.note]\ntext\n:::\n```\n";
        assert!(ComponentPlan::parse(literal).unwrap().is_none());

        let indented_literal = "    :::component[site.note]\n    text\n    :::\n";
        assert!(ComponentPlan::parse(indented_literal).unwrap().is_none());

        let raw_html = "<aside>\n:::component[site.note]\ntext\n:::\n</aside>\n";
        assert!(ComponentPlan::parse(raw_html).unwrap().is_none());

        let empty = ":::component[site.note]\n\n:::\n";
        assert!(matches!(
            ComponentPlan::parse(empty),
            Err(RenderError::Component { .. })
        ));
    }

    #[test]
    fn requires_a_closing_colon_fence_line_and_keeps_code_examples_literal() {
        let source = ":::component[site.note]\n::: this remains body text\n::::\n";
        let plan = ComponentPlan::parse(source)
            .expect("the longer empty closing fence is valid")
            .expect("source has one component");
        assert_eq!(
            &source[plan.calls()[0].body_span.clone()],
            "::: this remains body text"
        );

        let code = ":::component[site.note]\n```markdown\n[^literal]: not a definition\n```\n:::\n";
        assert!(
            ComponentPlan::parse(code).is_ok(),
            "a definition-looking line in a code block remains literal"
        );
    }
}
