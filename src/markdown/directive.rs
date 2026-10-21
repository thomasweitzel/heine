//! Lexical boundaries shared by Heine's explicit Markdown directives.
//!
//! The directive-specific modules own their headers and bodies. This module
//! only decides whether colon syntax appears where Markdown leaves it
//! executable: outside code fences, inline code, raw-HTML blocks, and raw HTML
//! syntax, with consistent colon-fence and CRLF rules.

use std::ops::Range;

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

/// A block-directive form whose surrounding Markdown structure has no current
/// directive contract, but could reasonably gain one later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedBlockPlacement {
    pub(crate) container: &'static str,
    pub(crate) span: Range<usize>,
}

/// One of Heine's built-in block directive roles.
///
/// Keep this list deliberately small. A new role belongs here only when it
/// has a checked Markdown contract of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockDirectiveKind {
    Picture,
    Render,
    Component,
}

/// The shared lexical part of a built-in block-directive opening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockDirectiveOpening {
    pub(crate) kind: BlockDirectiveKind,
    pub(crate) fence_length: usize,
}

/// Recognizes a built-in block-directive opening at the beginning of `line`.
///
/// This is intentionally stricter than a name-prefix check. In particular,
/// `:::picture-book` remains literal Markdown rather than becoming a picture
/// directive. `render[` remains an opening even without its closing bracket so
/// the renderer module can report that malformed header at its authored span.
/// Individual directive modules own the rest of their header syntax after this
/// shared boundary.
pub(crate) fn block_directive_opening(line: &str) -> Option<BlockDirectiveOpening> {
    let fence_length = colon_run(line);
    (fence_length >= 3).then_some(())?;
    let header = &line[fence_length..];
    let kind = if header == "picture" {
        BlockDirectiveKind::Picture
    } else if header.starts_with("render[") {
        BlockDirectiveKind::Render
    } else if header.starts_with("component[") && header.ends_with(']') {
        BlockDirectiveKind::Component
    } else {
        return None;
    };
    Some(BlockDirectiveOpening { kind, fence_length })
}

/// Returns the colon run of an otherwise empty closing fence.
pub(crate) fn closing_fence(line: &str) -> Option<usize> {
    let length = colon_run(line);
    (length > 0 && line[length..].trim().is_empty()).then_some(length)
}

/// Finds the next unescaped inline role outside Markdown code spans.
///
/// `cursor` is a byte boundary in `line`. The result points at the initial
/// colon of `:name[...`. The caller resumes after the complete role that it
/// recognizes.
pub(crate) fn next_inline_role(
    line: &str,
    line_start: usize,
    mut cursor: usize,
    name: &str,
    raw_html: &[Range<usize>],
) -> Option<usize> {
    while cursor < line.len() {
        if let Some(range) = raw_html_range_at(raw_html, line_start + cursor) {
            cursor = range.end.saturating_sub(line_start).min(line.len());
            continue;
        }
        if line.as_bytes()[cursor] == b'`' {
            let run = line[cursor..]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            let after = cursor + run;
            let delimiter = "`".repeat(run);
            if let Some(close) = line[after..].find(&delimiter) {
                cursor = after + close + run;
                continue;
            }
            cursor = after;
            continue;
        }
        let after_colon = cursor + 1;
        if line.as_bytes()[cursor] == b':'
            && (cursor == 0 || line.as_bytes()[cursor - 1] != b':')
            && line[after_colon..].starts_with(name)
            && line.as_bytes().get(after_colon + name.len()) == Some(&b'[')
            && !is_escaped(line, cursor)
        {
            return Some(cursor);
        }
        let Some(character) = line[cursor..].chars().next() else {
            break;
        };
        cursor += character.len_utf8();
    }
    None
}

/// Returns whether a source position is escaped by an odd number of
/// backslashes.
pub(crate) fn is_escaped(line: &str, offset: usize) -> bool {
    line[..offset]
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'\\')
        .count()
        % 2
        == 1
}

/// Identifies an opening backtick or tilde code fence in an ordinary source
/// line. Up to three spaces are CommonMark's block indentation allowance.
pub(crate) fn opening_code_fence(line: &str) -> Option<(u8, usize)> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    let trimmed = (indent <= 3).then(|| &line[indent..])?;
    let marker = *trimmed.as_bytes().first()?;
    matches!(marker, b'`' | b'~').then_some(())?;
    let length = trimmed.bytes().take_while(|byte| *byte == marker).count();
    (length >= 3).then_some((marker, length))
}

/// Identifies the matching closing code fence for `marker` and `length`.
pub(crate) fn closing_code_fence(line: &str, marker: u8) -> Option<usize> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    let trimmed = (indent <= 3).then(|| &line[indent..])?;
    let length = trimmed.bytes().take_while(|byte| *byte == marker).count();
    (length > 0 && trimmed[length..].trim().is_empty()).then_some(length)
}

/// Removes a CR byte while preserving the caller's original byte offsets.
pub(crate) fn trim_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

/// Advances to the byte offset immediately after a line ending.
pub(crate) fn next_line(input: &str, end: usize) -> usize {
    if end < input.len() { end + 1 } else { end }
}

/// Identifies CommonMark's indented-code form, which always keeps directives
/// literal.
pub(crate) fn is_indented_code_line(line: &str) -> bool {
    line.starts_with('\t') || line.bytes().take_while(|byte| *byte == b' ').count() >= 4
}

/// Finds the first block directive outside an ordinary top-level paragraph.
///
/// List items, block quotes, and paragraph continuations are meaningful enough
/// that accepting directives there would make a later structural contract a
/// silent behavior change. Other non-block positions remain literal Markdown.
pub(crate) fn unsupported_block_placement(input: &str) -> Option<UnsupportedBlockPlacement> {
    if !input.contains(":::") {
        return None;
    }
    let starts = directive_starts(input);
    if starts.is_empty() {
        return None;
    }
    let mut starts = starts.iter().peekable();
    let mut lists = 0;
    let mut quotes = 0;
    let mut paragraph_start = None;
    let mut code_block_depth = 0;

    for (event, range) in Parser::new(input).into_offset_iter() {
        match event {
            Event::Start(Tag::List(_)) => lists += 1,
            Event::Start(Tag::BlockQuote(_)) => quotes += 1,
            Event::End(TagEnd::List(_)) => lists -= 1,
            Event::End(TagEnd::BlockQuote(_)) => quotes -= 1,
            Event::Start(Tag::Paragraph) => paragraph_start = Some(range.start),
            Event::End(TagEnd::Paragraph) => paragraph_start = None,
            Event::Start(Tag::CodeBlock(_)) => code_block_depth += 1,
            Event::End(TagEnd::CodeBlock) => code_block_depth -= 1,
            Event::Text(_)
                if code_block_depth == 0
                    && (paragraph_start.is_some() || lists > 0 || quotes > 0) =>
            {
                while starts.peek().is_some_and(|start| **start < range.start) {
                    starts.next();
                }
                while let Some(start) = starts.next_if(|start| range.contains(start)) {
                    let continuation = paragraph_start.is_some_and(|paragraph| paragraph != *start);
                    if lists > 0 || quotes > 0 || continuation {
                        return Some(UnsupportedBlockPlacement {
                            container: container_name(lists > 0, quotes > 0, continuation),
                            span: *start..line_end(input, *start),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Collects raw-HTML source ranges that keep directive-looking text literal.
///
/// Markdown owns the details of HTML block recognition. Reusing its offsets
/// here prevents each directive from carrying a partial and subtly different
/// HTML scanner.
pub(crate) fn raw_html_ranges(input: &str) -> Vec<Range<usize>> {
    Parser::new(input)
        .into_offset_iter()
        .filter_map(|(event, range)| {
            matches!(event, Event::Html(_) | Event::InlineHtml(_)).then_some(range)
        })
        .collect()
}

/// Returns whether `offset` belongs to a raw-HTML source range.
///
/// The parser yields ordered ranges, so a binary partition keeps the repeated
/// line checks logarithmic even in documents with many HTML fragments.
pub(crate) fn is_raw_html_at(ranges: &[Range<usize>], offset: usize) -> bool {
    raw_html_range_at(ranges, offset).is_some()
}

/// Returns the raw-HTML range containing `offset`, when one exists.
pub(crate) fn raw_html_range_at(ranges: &[Range<usize>], offset: usize) -> Option<&Range<usize>> {
    ranges
        .get(ranges.partition_point(|range| range.end <= offset))
        .filter(|range| range.start <= offset)
}

fn colon_run(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b':').count()
}

fn container_name(in_list: bool, in_quote: bool, in_paragraph_continuation: bool) -> &'static str {
    if in_list {
        if in_quote {
            "list item in a block quote"
        } else {
            "list item"
        }
    } else if in_quote {
        "block quote"
    } else {
        debug_assert!(in_paragraph_continuation);
        "paragraph continuation"
    }
}

fn directive_starts(input: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if let Some(start) = directive_offset_in_line(trimmed) {
            starts.push(offset + start);
        }
        offset += line.len();
    }
    starts
}

fn directive_offset_in_line(line: &str) -> Option<usize> {
    let mut offset = 0;
    loop {
        offset += line[offset..]
            .bytes()
            .take_while(|byte| matches!(*byte, b' ' | b'\t'))
            .count();
        let remaining = &line[offset..];
        if block_directive_opening(remaining).is_some() {
            return Some(offset);
        }
        if let Some(after_quote) = remaining.strip_prefix('>') {
            offset += 1;
            if after_quote.starts_with([' ', '\t']) {
                offset += 1;
            }
            continue;
        }
        let marker = list_marker_length(remaining)?;
        offset += marker;
    }
}

fn list_marker_length(line: &str) -> Option<usize> {
    let marker = match line.as_bytes().first() {
        Some(b'-' | b'+' | b'*') => 1,
        Some(b'0'..=b'9') => line
            .bytes()
            .position(|byte| !byte.is_ascii_digit())
            .and_then(|digits| {
                matches!(line.as_bytes().get(digits), Some(b'.' | b')')).then_some(digits + 1)
            })?,
        _ => return None,
    };
    line[marker..]
        .starts_with([' ', '\t'])
        .then_some(marker + 1)
}

fn line_end(input: &str, offset: usize) -> usize {
    input[offset..]
        .find('\n')
        .map_or(input.len(), |length| offset + length)
}

#[cfg(test)]
mod tests {
    use super::{
        BlockDirectiveKind, BlockDirectiveOpening, block_directive_opening, closing_fence,
        is_raw_html_at, next_inline_role, raw_html_ranges, unsupported_block_placement,
    };

    #[test]
    fn shares_opening_and_closing_fence_lengths() {
        assert_eq!(
            block_directive_opening("::::component[site.note]"),
            Some(BlockDirectiveOpening {
                kind: BlockDirectiveKind::Component,
                fence_length: 4,
            })
        );
        assert_eq!(
            block_directive_opening("::::render[diagram]"),
            Some(BlockDirectiveOpening {
                kind: BlockDirectiveKind::Render,
                fence_length: 4,
            })
        );
        assert_eq!(
            block_directive_opening("::::picture"),
            Some(BlockDirectiveOpening {
                kind: BlockDirectiveKind::Picture,
                fence_length: 4,
            })
        );
        assert!(block_directive_opening(":::picture-book").is_none());
        assert_eq!(closing_fence("::::"), Some(4));
        assert_eq!(closing_fence("::: text"), None);
    }

    #[test]
    fn inline_roles_skip_code_and_respect_escaping() {
        let source = "` :render[code]` \\:render[literal] :render[active]`body`";
        assert_eq!(next_inline_role(source, 0, 0, "render", &[]), Some(35));
    }

    #[test]
    fn inline_roles_skip_raw_html_ranges_and_colon_fences() {
        let source = "<span data-render=\":render[ignored]`x`\"></span> :render[active]`x`";
        let ranges = raw_html_ranges(source);
        assert_eq!(
            next_inline_role(source, 0, 0, "render", &ranges),
            Some(source.find(":render[active]").expect("active role"))
        );
        assert!(next_inline_role("\\:::render[ignored]`x`", 0, 0, "render", &[]).is_none());
    }

    #[test]
    fn raw_html_ranges_keep_directive_looking_lines_literal() {
        let source = "<div>\n:::component[site.note]\nBody\n:::\n</div>\n";
        let ranges = raw_html_ranges(source);
        let directive = source.find(":::component").expect("directive text");
        assert!(is_raw_html_at(&ranges, directive));
    }

    #[test]
    fn reserves_list_and_quote_positions_without_reading_literal_contexts() {
        let list = "- :::component[site.note]\n";
        assert_eq!(
            unsupported_block_placement(list),
            Some(super::UnsupportedBlockPlacement {
                container: "list item",
                span: 2..25,
            })
        );

        let quote = "> :::render[diagram]\n";
        assert_eq!(
            unsupported_block_placement(quote),
            Some(super::UnsupportedBlockPlacement {
                container: "block quote",
                span: 2..20,
            })
        );

        let continuation = "- Before\n  :::render[diagram]\n  source\n  :::\n";
        assert!(matches!(
            unsupported_block_placement(continuation),
            Some(super::UnsupportedBlockPlacement {
                container: "list item",
                ..
            })
        ));

        let nested = "> - :::picture\n";
        assert!(matches!(
            unsupported_block_placement(nested),
            Some(super::UnsupportedBlockPlacement {
                container: "list item in a block quote",
                ..
            })
        ));

        let literal = "```markdown\n- :::component[site.note]\n```\n";
        assert!(unsupported_block_placement(literal).is_none());
        assert!(unsupported_block_placement("| :::picture |\n| --- |\n").is_none());
        assert!(unsupported_block_placement("- :::picture-book\n").is_none());

        let continuation = "Before\n:::component[site.note]\nBody\n:::\n";
        assert!(matches!(
            unsupported_block_placement(continuation),
            Some(super::UnsupportedBlockPlacement {
                container: "paragraph continuation",
                ..
            })
        ));

        let adjacent = ":::component[site.note]\nBody\n:::\n:::render[diagram]\nBody\n:::\n";
        assert!(matches!(
            unsupported_block_placement(adjacent),
            Some(super::UnsupportedBlockPlacement {
                container: "paragraph continuation",
                ..
            })
        ));
    }
}
