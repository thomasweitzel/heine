//! Source parsing for explicit Markdown renderer requests.
//!
//! Renderer requests are recognized before ordinary Markdown turns directive
//! syntax into prose. The parser keeps byte spans so later profile resolution
//! and execution diagnostics can identify the authored request precisely.

use std::{collections::BTreeMap, ops::Range};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Request {
    pub(super) placement: Placement,
    pub(super) profile: String,
    pub(super) header_span: Range<usize>,
    pub(super) body_span: Range<usize>,
    end: usize,
    pub(super) options: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Placement {
    Inline,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Error {
    pub(super) message: String,
    pub(super) span: Range<usize>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for Error {}

pub(super) fn parse(input: &str) -> Result<Vec<Request>, Error> {
    let mut requests = Vec::new();
    let mut offset = 0;
    let mut fenced: Option<(u8, usize)> = None;
    while offset < input.len() {
        let line_end = input[offset..]
            .find('\n')
            .map_or(input.len(), |index| offset + index);
        let line = &input[offset..line_end];
        if let Some((marker, length)) = fenced {
            if fence_run(line, marker).is_some_and(|run| run >= length) {
                fenced = None;
            }
            offset = next_line(input, line_end);
            continue;
        }
        if let Some((marker, length)) = opening_fence(line) {
            fenced = Some((marker, length));
            offset = next_line(input, line_end);
            continue;
        }
        if let Some(request) = parse_block(input, offset, line_end)? {
            offset = request.end;
            requests.push(request);
            continue;
        }
        parse_inline_line(input, offset, line_end, &mut requests)?;
        offset = next_line(input, line_end);
    }
    Ok(requests)
}

fn next_line(input: &str, line_end: usize) -> usize {
    if line_end < input.len() {
        line_end + 1
    } else {
        line_end
    }
}

fn opening_fence(line: &str) -> Option<(u8, usize)> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    let trimmed = (indent <= 3).then(|| &line[indent..])?;
    let marker = *trimmed.as_bytes().first()?;
    matches!(marker, b'`' | b'~').then_some(())?;
    let length = trimmed.bytes().take_while(|byte| *byte == marker).count();
    (length >= 3).then_some((marker, length))
}

fn fence_run(line: &str, marker: u8) -> Option<usize> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    let trimmed = (indent <= 3).then(|| &line[indent..])?;
    let length = trimmed.bytes().take_while(|byte| *byte == marker).count();
    (length > 0 && trimmed[length..].trim().is_empty()).then_some(length)
}

fn parse_block(input: &str, start: usize, line_end: usize) -> Result<Option<Request>, Error> {
    let line = &input[start..line_end];
    let run = line.bytes().take_while(|byte| *byte == b':').count();
    if run < 3 || !line[run..].starts_with("render[") {
        return Ok(None);
    }
    let header_end = header_end(line, run + "render[".len())
        .map(|index| start + index)
        .ok_or_else(|| Error {
            message: "Markdown renderer directive is missing its closing ]".into(),
            span: start..line_end,
        })?;
    if !input[header_end + 1..line_end].trim().is_empty() {
        return Err(Error {
            message: "Markdown renderer directive header must end before the line break".into(),
            span: start..line_end,
        });
    }
    let header_span = start + run + "render[".len()..header_end;
    let (profile, options) = parse_header(&input[header_span.clone()], header_span.start)?;
    let body_start = next_line(input, line_end);
    let mut cursor = body_start;
    while cursor <= input.len() {
        let close_end = input[cursor..]
            .find('\n')
            .map_or(input.len(), |index| cursor + index);
        let close_line = &input[cursor..close_end];
        let close_run = close_line.bytes().take_while(|byte| *byte == b':').count();
        if close_run >= run && close_line[close_run..].trim().is_empty() {
            let body_end = if cursor == body_start {
                cursor
            } else {
                cursor - 1
            };
            return Ok(Some(Request {
                placement: Placement::Block,
                profile,
                header_span,
                body_span: body_start..body_end,
                end: next_line(input, close_end),
                options,
            }));
        }
        if close_end == input.len() {
            break;
        }
        cursor = close_end + 1;
    }
    Err(Error {
        message: "Markdown renderer directive is missing its closing colon line".into(),
        span: start..input.len(),
    })
}

fn parse_inline_line(
    input: &str,
    start: usize,
    end: usize,
    requests: &mut Vec<Request>,
) -> Result<(), Error> {
    let line = &input[start..end];
    let mut cursor = 0;
    while cursor < line.len() {
        if line.as_bytes()[cursor] == b'`' {
            let run = line[cursor..]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            let after = cursor + run;
            if let Some(close) = line[after..].find(&"`".repeat(run)) {
                cursor = after + close + run;
                continue;
            }
            break;
        }
        if line[cursor..].starts_with(":render[") && !escaped(line, cursor) {
            let header_start = cursor + ":render[".len();
            let Some(header_end) = header_end(line, header_start) else {
                return Err(Error {
                    message: "inline Markdown renderer request is missing its closing ]".into(),
                    span: start + cursor..end,
                });
            };
            let delimiter_start = header_end + 1;
            let run = line[delimiter_start..]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            if run == 0 {
                return Err(Error {
                    message: "inline Markdown renderer request must use a backtick-delimited body"
                        .into(),
                    span: start + cursor..start + delimiter_start,
                });
            }
            let body_start = delimiter_start + run;
            let delimiter = "`".repeat(run);
            let Some(body_length) = line[body_start..].find(&delimiter) else {
                return Err(Error {
                    message:
                        "inline Markdown renderer request is missing its closing backtick delimiter"
                            .into(),
                    span: start + cursor..end,
                });
            };
            let header_span = start + header_start..start + header_end;
            let (profile, options) =
                parse_header(&line[header_start..header_end], start + header_start)?;
            let body_span = start + body_start..start + body_start + body_length;
            requests.push(Request {
                placement: Placement::Inline,
                profile,
                header_span,
                body_span,
                end: start + body_start + body_length + run,
                options,
            });
            cursor = body_start + body_length + run;
            continue;
        }
        let Some(character) = line[cursor..].chars().next() else {
            break;
        };
        cursor += character.len_utf8();
    }
    Ok(())
}

fn header_end(input: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in input[start..].char_indices() {
        match quote {
            Some(delimiter) if delimiter == '"' && character == '\\' && !escaped => {
                escaped = true;
            }
            Some(delimiter) if character == delimiter && !escaped => quote = None,
            Some(_) => escaped = false,
            None if matches!(character, '\'' | '"') => quote = Some(character),
            None if character == ']' => return Some(start + index),
            None => {}
        }
    }
    None
}

fn escaped(line: &str, offset: usize) -> bool {
    line[..offset]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

fn parse_header(
    input: &str,
    offset: usize,
) -> Result<(String, BTreeMap<String, toml::Value>), Error> {
    let mut cursor = skip_spaces(input, 0);
    let profile_start = cursor;
    cursor = identifier_end(input, cursor);
    if cursor == profile_start {
        return Err(header_error(
            "renderer request must name a profile",
            offset,
            input.len(),
        ));
    }
    let profile = input[profile_start..cursor].into();
    let mut options = BTreeMap::new();
    while cursor < input.len() {
        let next = skip_spaces(input, cursor);
        if next == cursor {
            return Err(header_error(
                "renderer request options must be separated by spaces",
                offset + cursor,
                1,
            ));
        }
        cursor = next;
        let name_start = cursor;
        cursor = identifier_end(input, cursor);
        if cursor == name_start || input.as_bytes().get(cursor) != Some(&b'=') {
            return Err(header_error(
                "renderer request option must use name = value",
                offset + name_start,
                1,
            ));
        }
        let name = &input[name_start..cursor];
        cursor += 1;
        let value_start = cursor;
        cursor = scalar_end(input, cursor).ok_or_else(|| {
            header_error(
                "renderer request option must use a TOML scalar value",
                offset + value_start,
                1,
            )
        })?;
        let value_text = &input[value_start..cursor];
        let value = toml::from_str::<toml::Value>(&format!("value = {value_text}"))
            .ok()
            .and_then(|value| value.get("value").cloned())
            .filter(is_scalar)
            .ok_or_else(|| {
                header_error(
                    "renderer request option must use a TOML scalar value",
                    offset + value_start,
                    value_text.len(),
                )
            })?;
        if options.insert(name.into(), value).is_some() {
            return Err(header_error(
                "renderer request must not repeat an option",
                offset + name_start,
                name.len(),
            ));
        }
    }
    Ok((profile, options))
}

fn is_scalar(value: &toml::Value) -> bool {
    !matches!(value, toml::Value::Array(_) | toml::Value::Table(_))
}

fn scalar_end(input: &str, start: usize) -> Option<usize> {
    let first = *input.as_bytes().get(start)?;
    if matches!(first, b'\'' | b'"') {
        let mut escaped = false;
        for (index, byte) in input[start + 1..].bytes().enumerate() {
            if first == b'"' && byte == b'\\' && !escaped {
                escaped = true;
                continue;
            }
            if byte == first && !escaped {
                return Some(start + index + 2);
            }
            escaped = false;
        }
        None
    } else {
        let end = input[start..]
            .find(char::is_whitespace)
            .map_or(input.len(), |index| start + index);
        (end > start).then_some(end)
    }
}

fn skip_spaces(input: &str, mut cursor: usize) -> usize {
    while input
        .as_bytes()
        .get(cursor)
        .is_some_and(u8::is_ascii_whitespace)
    {
        cursor += 1;
    }
    cursor
}

fn identifier_end(input: &str, mut cursor: usize) -> usize {
    let Some(first) = input.as_bytes().get(cursor) else {
        return cursor;
    };
    if !matches!(first, b'a'..=b'z' | b'A'..=b'Z' | b'_') {
        return cursor;
    }
    cursor += 1;
    while input
        .as_bytes()
        .get(cursor)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        cursor += 1;
    }
    cursor
}

fn header_error(message: &str, start: usize, length: usize) -> Error {
    Error {
        message: message.into(),
        span: start..start + length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inline_and_block_requests_with_original_body_spans() {
        let source = ":render[katex]`x^2`\n\n:::render[plantuml output=\"resource\"]\nA -> B\n:::\n:render[katex alt=\"]\"]`y`\n";
        let requests = parse(source).expect("valid requests");

        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].placement, Placement::Inline);
        assert_eq!(&source[requests[0].body_span.clone()], "x^2");
        assert_eq!(requests[1].placement, Placement::Block);
        assert_eq!(&source[requests[1].body_span.clone()], "A -> B");
        assert_eq!(requests[2].options["alt"].as_str(), Some("]"));
    }

    #[test]
    fn ignores_fenced_code_spans_and_escaped_requests() {
        let source = "`:render[katex]`x``\n\\:render[katex]`x`\n```md\n:render[katex]`x`\n```\n";

        assert!(parse(source).expect("literal source").is_empty());
    }

    #[test]
    fn requires_well_formed_headers_and_closing_delimiters() {
        let error = parse(":render[katex output]`x`").expect_err("option lacks value");
        assert!(error.message.contains("name = value"));
        let error = parse(":::render[plantuml]\nsource\n").expect_err("block lacks close");
        assert!(error.message.contains("closing colon"));
    }

    #[test]
    fn accepts_empty_block_bodies_for_named_sources() {
        let source = ":::render[report source=\"report\"]\n:::\n";
        let requests = parse(source).expect("empty body is syntactically valid");

        assert_eq!(requests.len(), 1);
        assert!(source[requests[0].body_span.clone()].is_empty());
    }
}
